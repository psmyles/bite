import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import type { NodeGraph, Progress } from '../../shared/types.js';
import { EXECUTOR } from '../../shared/constants.js';
import { topoSort, groupBySetPattern, findOutputContributors, applyParamWires } from './graph-utils.js';
import type { NodeRegistry } from '../nodes/registry.js';
import { buildCommandArgs, buildCommandArgsFromJs } from './command-builder.js';
import { getExecutor } from './executorRegistry.js';
import {
  computeNodeParams,
  loadImageMeta,
  loadImageMean,
  loadImageChannelMean,
  loadMultipleChannelMeans,
  getSeparator,
  buildEmptyImageMeta,
  type ImageMeta,
} from './executor-compute.js';
import { spawnMagick } from './magick-spawn.js';
import { computeNewName, type RenameParams } from '../../shared/renameUtils.js';
import { timings } from './timing.js';
import { TEMP_DIR, shortHash } from './thumbnail-service.js';

export async function executeBatch(
  graph: NodeGraph,
  imagePaths: string[],
  outputDir: string | null, // null = same directory as each source image
  overwrite: 'skip' | 'overwrite',
  registry: NodeRegistry,
  onProgress: (p: Progress) => void,
  isCancelled: () => boolean
): Promise<{ processed: number; skipped: number; failed: number; errors: string[]; outputFiles: string[] }> {
  const batchT0 = Date.now();
  const outputFiles: string[] = [];
  const sorted = topoSort(graph.nodes, graph.edges);

  // Text Output nodes — treated as "output sinks" so upstream nodes (mean_value, etc.)
  // that only feed text outputs are recognised as output contributors.
  // Also includes workflow-output when it is in text mode.
  const outputNode = sorted.find((n) => n.id === 'workflow-output');
  const outputNodeTextMode = (outputNode?.data.params as Record<string, unknown> | undefined)?.outputMode === 'text';
  const textOutputNodes = sorted.filter(
    (n) =>
      registry.get(n.data.definitionId)?.executor === EXECUTOR.TEXT_OUTPUT ||
      (n.id === 'workflow-output' && outputNodeTextMode)
  );
  const hasTextOutputNodes = textOutputNodes.length > 0;
  // Whether the workflow produces an image output (edge to workflow-output in image mode).
  const hasImageOutput = !outputNodeTextMode && graph.edges.some((e) => e.target === 'workflow-output');

  // Nodes that actually contribute to the final output — backward BFS from
  // workflow-output following ALL edges (image AND param-wire).
  // This ensures channel_split that feeds mean_value → gate (via param-wires)
  // is correctly recognised as a contributor and uses the multi-stream path.
  // Nodes that are purely decorative (no path to workflow-output of any kind)
  // are excluded so they don't force the slow path unnecessarily.
  // Start BFS from workflow-output AND any text_output nodes so upstream nodes
  // (channel_split, mean_value, etc.) that only feed text outputs are correctly
  // recognised as contributors and trigger the right execution path.
  const outputContributorIds = findOutputContributors(graph.edges, [
    'workflow-output',
    ...textOutputNodes.map((n) => n.id),
  ]);

  // prop_ nodes depend on per-image file metadata (dimensions, name, EXIF, etc.)
  // mean_value depends on per-image pixel data but NOT on loadImageMeta.
  // Either kind requires per-image plan evaluation (no shared opArgs).
  // Only nodes that contribute to output are relevant — disconnected nodes must not
  // influence the shared-plan decision or trigger unnecessary metadata calls.
  const hasImageMetaNodes = sorted.some((n) => {
    if (!outputContributorIds.has(n.id)) return false;
    const def = registry.get(n.data.definitionId);
    return def?.needs_image_meta === true;
  });
  // prop_name / prop_path only need path.basename — no ImageMagick identify call required.
  // All other needs_image_meta nodes (dimensions, bitdepth, EXIF, …) need the full identify.
  const NAME_PATH_ONLY_EXECUTORS = new Set<string>([EXECUTOR.PROP_NAME, EXECUTOR.PROP_PATH]);
  const hasHeavyMetaNodes = sorted.some((n) => {
    if (!outputContributorIds.has(n.id)) return false;
    const def = registry.get(n.data.definitionId);
    return def?.needs_image_meta === true && !NAME_PATH_ONLY_EXECUTORS.has(def.executor ?? '');
  });
  const hasPropNodes =
    hasImageMetaNodes ||
    sorted.some((n) => {
      if (!outputContributorIds.has(n.id)) return false;
      const def = registry.get(n.data.definitionId);
      return def?.executor === EXECUTOR.MEAN_VALUE;
    });

  // Executors that require executeMultiStream (cannot be handled by the fast-path
  // buildOpArgsForImage): channel_split/merge produce multiple image buffers;
  // mean_value reads pixel data per-image and feeds param-wires (gate conditions etc.)
  // — buildOpArgsForImage skips it, leaving downstream gate conditions unset.
  const MULTI_STREAM_EXECUTORS = new Set<string>([EXECUTOR.CHANNEL_SPLIT, EXECUTOR.CHANNEL_MERGE, EXECUTOR.MEAN_VALUE]);
  const hasMultiStreamNodes = sorted.some((n) => {
    if (!outputContributorIds.has(n.id)) return false;
    const def = registry.get(n.data.definitionId);
    return def?.executor && MULTI_STREAM_EXECUTORS.has(def.executor);
  });

  const FORMAT_EXT: Record<string, string> = {
    PNG: '.png',
    JPEG: '.jpg',
    WEBP: '.webp',
    TIFF: '.tif',
    AVIF: '.avif',
    BMP: '.bmp',
    TGA: '.tga',
  };

  interface BatchPlan {
    opArgs: string[];
    outputFormat: string | null; // e.g. 'PNG' — non-null only when format_convert is active
    textLines: string[]; // values collected from text_output nodes (condition=true)
  }

  // Lines collected across all images; written to disk after the batch completes.
  const collectedTextLines: Array<{ index: number; value: string }> = [];

  // Returns null when a Gate node suppresses the image (don't write output).
  async function buildOpArgsForImage(imagePath: string): Promise<BatchPlan | null> {
    let meta: ImageMeta | undefined;
    if (hasHeavyMetaNodes) {
      try {
        meta = await loadImageMeta(imagePath);
      } catch (err) {
        console.warn(`[executor] loadImageMeta failed for ${imagePath} (non-fatal, prop nodes use defaults):`, err);
      }
    } else if (hasImageMetaNodes && imagePath !== '') {
      // prop_name / prop_path only — no ImageMagick spawn needed.
      meta = buildEmptyImageMeta(imagePath);
    }
    const resolvedParams = new Map<string, Record<string, unknown>>();
    const opArgs: string[] = [];
    const textLines: string[] = [];
    let outputFormat: string | null = null;
    for (const node of sorted) {
      if (!outputContributorIds.has(node.id)) continue;
      if (node.data.definitionId === EXECUTOR.PROCESS_AS_SET) continue;
      const def = registry.get(node.data.definitionId);
      if (!def) {
        // workflow-output in text mode acts as a text output sink
        if (node.id === 'workflow-output' && outputNodeTextMode) {
          const rawParams: Record<string, unknown> = { ...(node.data.params ?? {}) };
          for (const edge of graph.edges) {
            if (edge.target !== node.id) continue;
            const th = edge.targetHandle ?? '',
              sh = edge.sourceHandle ?? '';
            if (sh.startsWith('param-out-') && th.startsWith('txo-')) {
              const src = resolvedParams.get(edge.source);
              if (src) rawParams[`_txo_${th.slice('txo-'.length)}`] = src[sh.slice('param-out-'.length)];
            }
          }
          if (Boolean(rawParams._txo_condition ?? true)) {
            const portIds = (rawParams.portIds ?? []) as string[];
            const sep = getSeparator(String(rawParams.separatorType ?? ''), String(rawParams.customSeparator ?? ''));
            const line = portIds
              .map((pid) => rawParams[`_txo_${pid.slice('txo-'.length)}`])
              .filter((v) => v !== undefined && v !== null && v !== '')
              .map((v) => String(v))
              .join(sep);
            if (line) textLines.push(line);
          }
        }
        continue;
      }
      const rawParams = applyParamWires(node, graph.edges, resolvedParams);
      const isImageNode =
        def.inputs.some((p) => p.type === 'image' || p.type === 'mask') ||
        def.outputs.some((p) => p.type === 'image' || p.type === 'mask');
      const computeInput =
        !isImageNode && def.compute_js ? { ...rawParams, __compute_js__: def.compute_js } : rawParams;
      const params = computeNodeParams(isImageNode ? undefined : def.executor, computeInput, meta);
      resolvedParams.set(node.id, params);
      if (!isImageNode) {
        // Collect text_output values — written to disk after the full batch completes.
        if (
          def.executor === EXECUTOR.TEXT_OUTPUT &&
          params._enabled !== false &&
          Boolean(params._txo_condition ?? params.condition)
        ) {
          const portIds = (params.portIds ?? []) as string[];
          const sep = getSeparator(String(params.separatorType ?? ''), String(params.customSeparator ?? ''));
          const values = portIds
            .map((pid) => params[`_txo_${pid.slice('txo-'.length)}`])
            .filter((v) => v !== undefined && v !== null && v !== '')
            .map((v) => String(v));
          const line = values.join(sep);
          if (line) textLines.push(line);
        }
        continue;
      }
      // Gate node: when active and condition is false, suppress this image entirely
      if (def.executor === EXECUTOR.GATE && params._enabled !== false && !params.condition) return null;
      // Mean Value — analysis-only, no image output, no opArgs contribution
      if (def.executor === EXECUTOR.MEAN_VALUE) continue;
      if (params._enabled !== false) {
        if (def.executor === EXECUTOR.FORMAT_CONVERT) {
          // Record the target format so processOne can set the output extension.
          // Add quality arg unconditionally — ImageMagick ignores it for lossless formats.
          outputFormat = String(params.format ?? 'PNG').toUpperCase();
          opArgs.push('-quality', String(params.quality ?? 90));
        } else {
          const registeredFn = def.executor ? getExecutor(def.executor) : undefined;
          opArgs.push(
            ...(registeredFn
              ? registeredFn(def, params)
              : def.command_js
                ? buildCommandArgsFromJs(def, params)
                : buildCommandArgs(def, params))
          );
        }
      }
    }
    return { opArgs, outputFormat, textLines };
  }

  // Fast path: no Properties nodes — evaluate once, reuse for all images.
  // undefined = not pre-computed (will be built per-image); null = gate suppressed for all images.
  const sharedPlan: BatchPlan | null | undefined =
    hasPropNodes || hasMultiStreamNodes ? undefined : await buildOpArgsForImage('');

  // Multi-stream execution for a single image with two speed optimisations:
  //   1. Command fusion — consecutive standard nodes are chained into a single magick
  //      invocation instead of one process per node (lazy-buffer approach).
  //   2. Channel split — all 4 channels are extracted in one magick call via -write.
  // Returns the final output path and extension, or null if the image should be suppressed.
  const executeMultiStream = async (
    inputPath: string,
    imageIndex: number,
    extraSeeds?: Map<string, string>
  ): Promise<{ resultPath: string; outputExt: string; cleanup: () => Promise<void> } | null> => {
    const tmpId = shortHash(inputPath + String(imageIndex));
    let _seq = 0;
    const tmpFiles: string[] = [];
    // MIFF (ImageMagick native) is uncompressed and skips PNG encode/decode overhead
    // for every intermediate step. Only the final output uses the real output extension.
    const newTmp = (ext = '.miff') => {
      const p = path.join(TEMP_DIR, `batch_ms_${tmpId}_${_seq++}${ext}`);
      tmpFiles.push(p);
      return p;
    };
    const cleanupAll = () => Promise.allSettled(tmpFiles.map((p) => fs.promises.unlink(p).catch(() => {})));

    // Each buffer slot is either a concrete file path (string) or a lazy chain that
    // accumulates magick args to be applied to a base file on demand.
    type Lazy = { base: string; args: string[] };
    const buffers = new Map<string, string | Lazy>();
    buffers.set('workflow-input:out-0', inputPath);
    if (extraSeeds) for (const [k, v] of extraSeeds) buffers.set(k, v);

    // Materialise a buffer slot: flush its lazy args into a temp file if needed.
    const mat = async (key: string): Promise<string> => {
      const v = buffers.get(key);
      if (v === undefined) return inputPath;
      if (typeof v === 'string') return v;
      if (v.args.length === 0) {
        buffers.set(key, v.base);
        return v.base;
      }
      const out = newTmp();
      await spawnMagick([v.base, ...v.args, out]);
      buffers.set(key, out); // upgrade to concrete path so re-materialisation is free
      return out;
    };

    // Return the materialised image path for node's Nth image input.
    const getImg = async (nodeId: string, inputIdx: number): Promise<string> => {
      const edge = graph.edges.find((e) => e.target === nodeId && e.targetHandle === `in-${inputIdx}`);
      if (!edge) return inputPath;
      return mat(`${edge.source}:${edge.sourceHandle ?? 'out-0'}`);
    };

    // Count image-edge consumers per output key so we know when lazy chaining is safe.
    // (A lazy chain can only be extended when we are the sole consumer of its source.)
    const imgConsumers = new Map<string, number>();
    for (const e of graph.edges) {
      if ((e.targetHandle ?? '').startsWith('in-')) {
        const k = `${e.source}:${e.sourceHandle ?? 'out-0'}`;
        imgConsumers.set(k, (imgConsumers.get(k) ?? 0) + 1);
      }
    }

    // Pre-detect channel_split nodes whose outputs are consumed ONLY by mean_value.
    // For these we skip writing the 4 channel PNG files and instead compute the
    // per-channel mean directly from the source image (no temp-file I/O at all).
    // Map: "splitNodeId:out-N" → { srcKey of split's image input, channelIdx }
    const channelMeanSources = new Map<string, { srcKey: string; channelIdx: number }>();
    const analysisOnlySplitNodes = new Set<string>();
    for (const n of sorted) {
      const d = registry.get(n.data.definitionId);
      if (d?.executor !== EXECUTOR.CHANNEL_SPLIT) continue;
      const outEdges = graph.edges.filter((e) => e.source === n.id && (e.sourceHandle ?? '').startsWith('out-'));
      if (outEdges.length === 0) continue;
      const allMeanValue = outEdges.every((e) => {
        const consumer = sorted.find((c) => c.id === e.target);
        return registry.get(consumer?.data.definitionId ?? '')?.executor === EXECUTOR.MEAN_VALUE;
      });
      if (!allMeanValue) continue;
      analysisOnlySplitNodes.add(n.id);
      const splitInEdge = graph.edges.find((e) => e.target === n.id && e.targetHandle === 'in-0');
      const splitSrcKey = splitInEdge
        ? `${splitInEdge.source}:${splitInEdge.sourceHandle ?? 'out-0'}`
        : 'workflow-input:out-0';
      for (const e of outEdges) {
        const chIdx = parseInt((e.sourceHandle ?? 'out-0').replace('out-', ''));
        channelMeanSources.set(`${n.id}:${e.sourceHandle ?? 'out-0'}`, { srcKey: splitSrcKey, channelIdx: chIdx });
      }
    }

    let meta: ImageMeta | undefined;
    if (hasHeavyMetaNodes) {
      try {
        meta = await loadImageMeta(inputPath);
      } catch (err) {
        console.warn(`[executor] loadImageMeta failed for ${inputPath} (non-fatal):`, err);
      }
    } else if (hasImageMetaNodes) {
      // prop_name / prop_path only — no ImageMagick spawn needed.
      meta = buildEmptyImageMeta(inputPath);
    }

    // Pre-compute all channel means for analysis-only split nodes in a single magick
    // spawn per source image, instead of one spawn per channel per mean_value node.
    // Key: srcKey (e.g. "workflow-input:out-0"), Value: array indexed by channelIdx.
    const precomputedMeans = new Map<string, number[]>();
    if (channelMeanSources.size > 0) {
      const bySourceKey = new Map<string, number[]>();
      for (const { srcKey, channelIdx } of channelMeanSources.values()) {
        if (!bySourceKey.has(srcKey)) bySourceKey.set(srcKey, []);
        const arr = bySourceKey.get(srcKey)!;
        if (!arr.includes(channelIdx)) arr.push(channelIdx);
      }
      for (const [srcKey, indices] of bySourceKey) {
        try {
          indices.sort((a, b) => a - b);
          const resolvedSrc = await mat(srcKey);
          const means = await loadMultipleChannelMeans(resolvedSrc, indices);
          const byIdx: number[] = [];
          for (let i = 0; i < indices.length; i++) byIdx[indices[i]] = means[i];
          precomputedMeans.set(srcKey, byIdx);
        } catch (err) {
          console.warn(`[executor] loadMultipleChannelMeans failed for ${srcKey}:`, err);
        }
      }
    }

    // Track the effective output extension (updated by format_convert nodes).
    let outputExt = path.extname(inputPath);

    const resolvedParams = new Map<string, Record<string, unknown>>();

    for (const node of sorted) {
      if (!outputContributorIds.has(node.id)) continue;
      // process_as_set is a source node — buffers are pre-seeded externally; skip processing.
      if (node.data.definitionId === EXECUTOR.PROCESS_AS_SET) continue;
      const def = registry.get(node.data.definitionId);
      if (!def) {
        // workflow-output in text mode acts as a text output sink
        if (node.id === 'workflow-output' && outputNodeTextMode) {
          const rawParams: Record<string, unknown> = { ...(node.data.params ?? {}) };
          for (const edge of graph.edges) {
            if (edge.target !== node.id) continue;
            const th = edge.targetHandle ?? '',
              sh = edge.sourceHandle ?? '';
            if (sh.startsWith('param-out-') && th.startsWith('txo-')) {
              const src = resolvedParams.get(edge.source);
              if (src) rawParams[`_txo_${th.slice('txo-'.length)}`] = src[sh.slice('param-out-'.length)];
            }
          }
          if (Boolean(rawParams._txo_condition ?? true)) {
            const portIds = (rawParams.portIds ?? []) as string[];
            const sep = getSeparator(String(rawParams.separatorType ?? ''), String(rawParams.customSeparator ?? ''));
            const line = portIds
              .map((pid) => rawParams[`_txo_${pid.slice('txo-'.length)}`])
              .filter((v) => v !== undefined && v !== null && v !== '')
              .map((v) => String(v))
              .join(sep);
            if (line) collectedTextLines.push({ index: imageIndex, value: line });
          }
        }
        continue;
      }

      const rawParams = applyParamWires(node, graph.edges, resolvedParams);

      const isImageNode =
        def.inputs.some((p) => p.type === 'image' || p.type === 'mask') ||
        def.outputs.some((p) => p.type === 'image' || p.type === 'mask');
      const computeInput =
        !isImageNode && def.compute_js ? { ...rawParams, __compute_js__: def.compute_js } : rawParams;
      const params = computeNodeParams(isImageNode ? undefined : def.executor, computeInput, meta);
      resolvedParams.set(node.id, params);
      if (!isImageNode) {
        // Collect text_output values — written to disk after the full batch completes.
        if (
          def.executor === EXECUTOR.TEXT_OUTPUT &&
          params._enabled !== false &&
          Boolean(params._txo_condition ?? params.condition)
        ) {
          const portIds = (params.portIds ?? []) as string[];
          const sep = getSeparator(String(params.separatorType ?? ''), String(params.customSeparator ?? ''));
          const values = portIds
            .map((pid) => params[`_txo_${pid.slice('txo-'.length)}`])
            .filter((v) => v !== undefined && v !== null && v !== '')
            .map((v) => String(v));
          const line = values.join(sep);
          if (line) collectedTextLines.push({ index: imageIndex, value: line });
        }
        continue;
      }

      if (def.executor === EXECUTOR.GATE && params._enabled !== false && !params.condition) {
        await cleanupAll();
        return null;
      }

      if (def.executor === EXECUTOR.CHANNEL_SPLIT) {
        if (analysisOnlySplitNodes.has(node.id)) {
          // All consumers are mean_value — skip writing channel files entirely.
          // channelMeanSources already maps each out-N to the source image key;
          // mean_value will call loadImageChannelMean directly.
        } else {
          const CHAN = ['Red', 'Green', 'Blue', 'Alpha'];
          const usedIdxs = [0, 1, 2, 3].filter((i) => (imgConsumers.get(`${node.id}:out-${i}`) ?? 0) > 0);

          if (usedIdxs.length === 1) {
            // Single channel consumed — defer as a lazy chain so the downstream op
            // (e.g. negate) can fuse onto it, avoiding an intermediate temp file.
            const src = await getImg(node.id, 0);
            const i = usedIdxs[0];
            buffers.set(`${node.id}:out-${i}`, { base: src, args: ['-channel', CHAN[i], '-separate'] });
          } else if (usedIdxs.length > 1) {
            // Multiple channels — extract only those actually used in one spawn.
            const src = await getImg(node.id, 0);
            const outs = usedIdxs.map(() => newTmp());
            const lastK = usedIdxs.length - 1;
            const args: string[] = [src];
            for (let k = 0; k < lastK; k++) {
              args.push('(', '+clone', '-channel', CHAN[usedIdxs[k]], '-separate', '-write', outs[k], '+delete', ')');
            }
            args.push('-channel', CHAN[usedIdxs[lastK]], '-separate', outs[lastK]);
            await spawnMagick(args);
            for (let k = 0; k < usedIdxs.length; k++) {
              buffers.set(`${node.id}:out-${usedIdxs[k]}`, outs[k]);
            }
          }
        }
      } else if (def.executor === EXECUTOR.CHANNEL_MERGE) {
        const refPath = await mat('workflow-input:out-0');

        // Returns either a concrete path or inline magick args for a constant fill.
        // Constant channels (param-wired or unconnected) skip the intermediate temp-file
        // spawn and are inlined as parenthesised sub-expressions in the combine command.
        type ChanSrc = { kind: 'path'; path: string } | { kind: 'inline'; args: string[] };

        const resolveChannel = async (inputIdx: number): Promise<ChanSrc> => {
          const imgEdge = graph.edges.find((e) => e.target === node.id && e.targetHandle === `in-${inputIdx}`);
          if (imgEdge) {
            if (imgEdge.sourceHandle?.startsWith('param-out-')) {
              const paramKey = imgEdge.sourceHandle.slice('param-out-'.length);
              const srcParams = resolvedParams.get(imgEdge.source);
              const fillVal = Math.max(0, Math.min(1, Number(srcParams?.[paramKey] ?? 0)));
              const pct = Math.round(fillVal * 100);
              return { kind: 'inline', args: [refPath, '-evaluate', 'set', `${pct}%`, '-colorspace', 'Gray'] };
            }
            return { kind: 'path', path: await mat(`${imgEdge.source}:${imgEdge.sourceHandle ?? 'out-0'}`) };
          }
          return { kind: 'inline', args: [refPath, '-evaluate', 'set', '0%', '-colorspace', 'Gray'] };
        };

        const expand = (ch: ChanSrc): string[] => (ch.kind === 'path' ? [ch.path] : ['(', ...ch.args, ')']);

        const channelCount = Number(params.channels ?? 3);
        const aImgEdge = graph.edges.find((e) => e.target === node.id && e.targetHandle === 'in-3');
        const hasAlpha = channelCount >= 4 && !!aImgEdge;
        // Resolve all channels concurrently — their materialisations are independent.
        const idxs = hasAlpha ? [0, 1, 2, 3] : [0, 1, 2];
        const resolved = await Promise.all(idxs.map((i) => resolveChannel(i)));
        const [r, g, b] = resolved;
        const a = hasAlpha ? resolved[3] : null;
        const out = newTmp();
        await spawnMagick([
          ...expand(r),
          ...expand(g),
          ...expand(b),
          ...(hasAlpha && a ? expand(a) : []),
          '-set',
          'colorspace',
          'sRGB',
          '-combine',
          ...(hasAlpha ? ['-alpha', 'on'] : []),
          out,
        ]);
        buffers.set(`${node.id}:out-0`, out);
      } else if (def.executor === EXECUTOR.MEAN_VALUE) {
        try {
          const imgInEdge = graph.edges.find((e) => e.target === node.id && e.targetHandle === 'in-0');
          const srcSlot = imgInEdge ? `${imgInEdge.source}:${imgInEdge.sourceHandle ?? 'out-0'}` : undefined;
          const chanInfo = srcSlot ? channelMeanSources.get(srcSlot) : undefined;
          const value = chanInfo
            ? (precomputedMeans.get(chanInfo.srcKey)?.[chanInfo.channelIdx] ??
              (await loadImageChannelMean(await mat(chanInfo.srcKey), chanInfo.channelIdx)))
            : await loadImageMean(await getImg(node.id, 0));
          resolvedParams.set(node.id, { ...rawParams, value });
        } catch (err) {
          console.warn(`[executor] loadImageMean failed for node ${node.id}:`, err);
        }
      } else if (def.executor === EXECUTOR.FORMAT_CONVERT) {
        // Format convert must materialise immediately (changes file type).
        const src = await getImg(node.id, 0);
        const fmt = String(params.format ?? 'PNG').toUpperCase();
        const fmtExts: Record<string, string> = {
          PNG: '.png',
          JPEG: '.jpg',
          WEBP: '.webp',
          TIFF: '.tif',
          AVIF: '.avif',
          BMP: '.bmp',
          TGA: '.tga',
        };
        outputExt = fmtExts[fmt] ?? outputExt;
        const out = newTmp(outputExt);
        await spawnMagick([src, '-quality', String(params.quality ?? 90), `${fmt}:${out}`]);
        buffers.set(`${node.id}:out-0`, out);
      } else if (params._enabled !== false) {
        // Standard image op — fuse into a lazy chain when safe to do so.
        const imgInEdge = graph.edges.find((e) => e.target === node.id && e.targetHandle === 'in-0');
        const srcKey = imgInEdge ? `${imgInEdge.source}:${imgInEdge.sourceHandle ?? 'out-0'}` : 'workflow-input:out-0';
        const outKey = `${node.id}:out-0`;
        const registeredFn = def.executor ? getExecutor(def.executor) : undefined;
        const opArgs = registeredFn
          ? registeredFn(def, params)
          : def.command_js
            ? buildCommandArgsFromJs(def, params)
            : buildCommandArgs(def, params);

        if (opArgs.length === 0) {
          // No-op — inherit source slot unchanged.
          buffers.set(outKey, buffers.get(srcKey) ?? inputPath);
        } else if ((imgConsumers.get(srcKey) ?? 0) <= 1) {
          // Sole consumer of source — extend (or start) the lazy chain.
          const srcVal = buffers.get(srcKey) ?? inputPath;
          if (typeof srcVal === 'string') {
            buffers.set(outKey, { base: srcVal, args: opArgs });
          } else {
            buffers.set(outKey, { base: srcVal.base, args: [...srcVal.args, ...opArgs] });
          }
        } else {
          // Multiple consumers — materialise first so we don't double-apply ops.
          const src = await mat(srcKey);
          buffers.set(outKey, { base: src, args: opArgs });
        }
      } else {
        // Bypassed — pass source slot through unchanged (preserves any lazy chain).
        const imgInEdge = graph.edges.find((e) => e.target === node.id && e.targetHandle === 'in-0');
        const srcKey = imgInEdge ? `${imgInEdge.source}:${imgInEdge.sourceHandle ?? 'out-0'}` : 'workflow-input:out-0';
        buffers.set(`${node.id}:out-0`, buffers.get(srcKey) ?? inputPath);
      }
    }

    const outputEdge = graph.edges.find((e) => e.target === 'workflow-output' && e.targetHandle === 'in-0');
    // When there's no output edge or no buffer for it, return the input unchanged (no temps to clean up from output).
    const cleanup = async () => {
      await cleanupAll();
    };
    if (!outputEdge) return { resultPath: inputPath, outputExt: path.extname(inputPath), cleanup };

    const finalKey = `${outputEdge.source}:${outputEdge.sourceHandle ?? 'out-0'}`;
    const finalVal = buffers.get(finalKey);
    if (!finalVal) return { resultPath: inputPath, outputExt: path.extname(inputPath), cleanup };
    if (typeof finalVal === 'string') {
      // If the concrete path is an intermediate format (e.g. .miff) but the output
      // needs a different format (e.g. .PNG), do a single conversion spawn.
      if (path.extname(finalVal).toLowerCase() !== outputExt.toLowerCase()) {
        const finalOut = newTmp(outputExt);
        await spawnMagick([finalVal, finalOut]);
        return { resultPath: finalOut, outputExt, cleanup };
      }
      return { resultPath: finalVal, outputExt, cleanup };
    }

    // Final materialisation: use the correct output extension (not always .png).
    const finalOut = newTmp(outputExt);
    if (finalVal.args.length > 0) await spawnMagick([finalVal.base, ...finalVal.args, finalOut]);
    else await fs.promises.copyFile(finalVal.base, finalOut);
    return { resultPath: finalOut, outputExt, cleanup };
  };

  // ── Preview substitution — swap full-size paths for cached thumbnails ────────
  if (outputNodeTextMode && (outputNode?.data.params as Record<string, unknown> | undefined)?.usePreviewForProcessing) {
    const inputNode = graph.nodes.find((n) => n.id === 'workflow-input');
    const inputParams = (inputNode?.data as Record<string, unknown>)?.params as Record<string, unknown> | undefined;
    const thumbSizePx = Number(inputParams?.thumbnailSize ?? 128);
    const thumbPath = (p: string) => path.join(TEMP_DIR, `thumb_${shortHash(p)}_${thumbSizePx}.webp`);
    imagePaths = await Promise.all(
      imagePaths.map(async (p) => {
        const thumb = thumbPath(p);
        return await fs.promises
          .access(thumb)
          .then(() => thumb)
          .catch(() => p);
      })
    );
  }

  // ── Set batch mode ─────────────────────────────────────────────────────────
  // When a setInputNode is present, group images by naming convention and
  // execute one run per set instead of one run per image.
  const setInputNode = sorted.find((n) => n.data.definitionId === EXECUTOR.PROCESS_AS_SET);
  if (setInputNode) {
    const setPrefix = String(setInputNode.data.params?.prefix ?? '');
    const setSuffixes = Array.isArray(setInputNode.data.params?.suffixes)
      ? (setInputNode.data.params!.suffixes as string[]).filter(Boolean)
      : [];
    const outParams = outputNode?.data.params as Record<string, unknown> | undefined;
    const setOutputPrefix = String(outParams?.setOutputPrefix ?? '');
    const setOutputSuffix = String(outParams?.setOutputSuffix ?? '');

    const setGroups = groupBySetPattern(imagePaths, setPrefix, setSuffixes);
    const setEntries = [...setGroups.entries()];
    const totalSets = setEntries.length;

    if (timings.enabled) {
      timings.startBatch(totalSets);
      timings.recordSetup(Date.now() - batchT0);
    }

    let setQueueIdx = 0;
    let setCompleted = 0;
    let setFailures = 0;
    let setSkipped = 0;
    const setErrors: string[] = [];
    let _setStartupRecorded = false;
    const activeImages = new Set<string>();

    onProgress({ completed: 0, total: totalSets, currentFile: '', active: [] });

    const setConcurrency = Math.min(Math.max(1, os.cpus().length), Math.max(1, totalSets));

    async function processOneSet(): Promise<void> {
      while (setQueueIdx < setEntries.length) {
        if (isCancelled()) return;
        const setIndex = setQueueIdx;
        const [middleName, suffixMap] = setEntries[setQueueIdx++];
        activeImages.add(middleName);
        onProgress({ completed: setCompleted, total: totalSets, currentFile: middleName, active: [...activeImages] });
        const imgT0 = timings.enabled ? Date.now() : 0;
        if (timings.enabled && !_setStartupRecorded) {
          _setStartupRecorded = true;
          timings.recordStartup(Date.now() - batchT0);
        }

        // First available path serves as the inputPath fallback for unconnected ports.
        const firstPath = setSuffixes.map((s) => suffixMap[s]).find(Boolean) ?? '';

        // Pre-seed one buffer per suffix port; missing suffixes are left unseeded
        // so their downstream nodes receive the firstPath fallback.
        const seeds = new Map<string, string>();
        const setNodeId = setInputNode!.id;
        for (let i = 0; i < setSuffixes.length; i++) {
          const p = suffixMap[setSuffixes[i]];
          if (p) seeds.set(`${setNodeId}:out-${i}`, p);
        }

        try {
          const targetDir = outputDir ?? (firstPath ? path.dirname(firstPath) : process.cwd());
          const checkT0 = timings.enabled ? Date.now() : 0;
          await fs.promises.mkdir(targetDir, { recursive: true });
          let fileCheckMs = timings.enabled ? Date.now() - checkT0 : 0;

          if (hasImageOutput) {
            const msT0 = timings.enabled ? Date.now() : 0;
            const msResult = await executeMultiStream(firstPath, setIndex, seeds);
            const msElapsed = timings.enabled ? Date.now() - msT0 : 0;
            if (msResult === null) {
              setSkipped++;
              activeImages.delete(middleName);
              onProgress({
                completed: ++setCompleted,
                total: totalSets,
                currentFile: middleName,
                active: [...activeImages],
              });
              continue;
            }
            const { resultPath, outputExt: msExt, cleanup } = msResult;
            const outBase = setOutputPrefix + middleName + setOutputSuffix;
            const outExt = msExt || path.extname(firstPath);
            const outPath = path.join(targetDir, outBase + outExt);

            if (overwrite === 'skip') {
              const accessT0 = timings.enabled ? Date.now() : 0;
              const exists = await fs.promises
                .access(outPath)
                .then(() => true)
                .catch(() => false);
              if (timings.enabled) fileCheckMs += Date.now() - accessT0;
              if (exists) {
                setSkipped++;
                activeImages.delete(middleName);
                onProgress({
                  completed: ++setCompleted,
                  total: totalSets,
                  currentFile: middleName,
                  active: [...activeImages],
                });
                void cleanup();
                continue;
              }
            }
            const copyT0 = timings.enabled ? Date.now() : 0;
            await fs.promises.copyFile(resultPath, outPath);
            void cleanup();
            if (timings.enabled) {
              const imgEntry = timings.beginImage(firstPath || middleName);
              imgEntry.fileCheck(fileCheckMs);
              imgEntry.magick(msElapsed);
              imgEntry.copy(Date.now() - copyT0);
              imgEntry.done(Date.now() - imgT0);
            }
            outputFiles.push(outPath);
          }
        } catch (err) {
          setFailures++;
          const msg = err instanceof Error ? err.message : String(err);
          setErrors.push(`${middleName}: ${msg}`);
          console.error(`[executor] Failed to process set "${middleName}":`, err);
        }
        activeImages.delete(middleName);
        onProgress({ completed: ++setCompleted, total: totalSets, currentFile: middleName, active: [...activeImages] });
      }
    }

    const threadsPerSetProcess = Math.max(1, Math.floor(os.cpus().length / setConcurrency));
    const prevThreadLimitSet = process.env.MAGICK_THREAD_LIMIT;
    process.env.MAGICK_THREAD_LIMIT = String(threadsPerSetProcess);
    try {
      await Promise.all(Array.from({ length: setConcurrency }, processOneSet));
    } finally {
      if (prevThreadLimitSet !== undefined) process.env.MAGICK_THREAD_LIMIT = prevThreadLimitSet;
      else delete process.env.MAGICK_THREAD_LIMIT;
    }
    if (timings.enabled) {
      const resolvedOutputDir = outputDir ?? (outputFiles.length > 0 ? path.dirname(outputFiles[0]) : null);
      timings.endBatch(resolvedOutputDir);
    }
    return {
      processed: setCompleted - setFailures - setSkipped,
      skipped: setSkipped,
      failed: setFailures,
      errors: setErrors,
      outputFiles,
    };
  }

  // Run all images concurrently, capped at 128 for very large batches.
  // With MAGICK_THREAD_LIMIT=1, each process uses exactly one thread so small
  // oversubscription (e.g. 26 images on 24 cores) costs less than a full extra round.
  // JS single-threaded event loop guarantees queueIdx++ is race-free.
  const concurrency = Math.min(128, imagePaths.length);
  let queueIdx = 0;
  let completed = 0;
  let failures = 0;
  let skipped = 0;
  const errors: string[] = [];

  if (timings.enabled) {
    timings.startBatch(imagePaths.length);
    timings.recordSetup(Date.now() - batchT0);
  }

  const activeImages = new Set<string>();
  onProgress({ completed: 0, total: imagePaths.length, currentFile: '', active: [] });

  // Resolve rename node params once (shared across all images — index varies per image)
  const renameNode = sorted.find((n) => registry.get(n.data.definitionId)?.executor === EXECUTOR.RENAME);
  const renameParams = renameNode ? (renameNode.data.params as RenameParams) : undefined;

  let _startupRecorded = false;

  async function processOne(): Promise<void> {
    while (queueIdx < imagePaths.length) {
      if (isCancelled()) return;
      const imageIndex = queueIdx; // 0-based index for rename numbering
      const inputPath = imagePaths[queueIdx++];
      const fileName = path.basename(inputPath);
      activeImages.add(fileName);
      onProgress({ completed, total: imagePaths.length, currentFile: fileName, active: [...activeImages] });
      const imgT0 = timings.enabled ? Date.now() : 0;
      if (timings.enabled && !_startupRecorded) {
        _startupRecorded = true;
        timings.recordStartup(Date.now() - batchT0);
      }
      const magickBucket = timings.enabled
        ? {
            ms: 0,
            add(n: number) {
              this.ms += n;
            },
          }
        : null;
      // Apply rename transform to determine the output filename stem
      const renamedFileName = renameParams ? computeNewName(fileName, renameParams, imageIndex) : fileName;
      try {
        const targetDir = outputDir ?? path.dirname(inputPath);
        const checkT0 = timings.enabled ? Date.now() : 0;
        await fs.promises.mkdir(targetDir, { recursive: true });
        let fileCheckMs = timings.enabled ? Date.now() - checkT0 : 0;

        if (hasMultiStreamNodes) {
          // Multi-stream path — runs concurrently; unique tmpId per image prevents collisions
          const msT0 = timings.enabled ? Date.now() : 0;
          const msResult = await executeMultiStream(inputPath, imageIndex);
          const msElapsed = timings.enabled ? Date.now() - msT0 : 0;
          if (msResult === null) {
            skipped++;
            activeImages.delete(fileName);
            onProgress({
              completed: ++completed,
              total: imagePaths.length,
              currentFile: fileName,
              active: [...activeImages],
            });
            continue;
          }
          if (hasImageOutput) {
            const { resultPath, outputExt: msExt, cleanup } = msResult;
            const outExt = msExt || path.extname(renamedFileName);
            const outBase = path.basename(renamedFileName, path.extname(renamedFileName));
            const outPath = path.join(targetDir, outBase + outExt);

            if (overwrite === 'skip') {
              const accessT0 = timings.enabled ? Date.now() : 0;
              const exists = await fs.promises
                .access(outPath)
                .then(() => true)
                .catch(() => false);
              if (timings.enabled) fileCheckMs += Date.now() - accessT0;
              if (exists) {
                skipped++;
                activeImages.delete(fileName);
                onProgress({
                  completed: ++completed,
                  total: imagePaths.length,
                  currentFile: fileName,
                  active: [...activeImages],
                });
                void cleanup();
                continue;
              }
            }
            const copyT0 = timings.enabled ? Date.now() : 0;
            await fs.promises.copyFile(resultPath, outPath);
            void cleanup();
            if (timings.enabled) {
              const imgEntry = timings.beginImage(inputPath);
              imgEntry.fileCheck(fileCheckMs);
              imgEntry.magick(msElapsed);
              imgEntry.copy(Date.now() - copyT0);
              imgEntry.done(Date.now() - imgT0);
            }
            outputFiles.push(outPath);
          } else {
            void msResult.cleanup();
          }
        } else {
          // Single-command fast path
          const plan = sharedPlan !== undefined ? sharedPlan : await buildOpArgsForImage(inputPath);
          if (plan === null) {
            skipped++;
            activeImages.delete(fileName);
            onProgress({
              completed: ++completed,
              total: imagePaths.length,
              currentFile: fileName,
              active: [...activeImages],
            });
            continue;
          }
          // Collect text output values from this image's plan.
          for (const line of plan.textLines) {
            collectedTextLines.push({ index: imageIndex, value: line });
          }
          if (hasImageOutput) {
            const { opArgs, outputFormat } = plan;
            const outExt = outputFormat
              ? (FORMAT_EXT[outputFormat] ?? path.extname(renamedFileName))
              : path.extname(renamedFileName);
            const outBase = path.basename(renamedFileName, path.extname(renamedFileName));
            const outPath = path.join(targetDir, outBase + outExt);

            if (overwrite === 'skip') {
              const accessT0 = timings.enabled ? Date.now() : 0;
              const exists = await fs.promises
                .access(outPath)
                .then(() => true)
                .catch(() => false);
              if (timings.enabled) fileCheckMs += Date.now() - accessT0;
              if (exists) {
                skipped++;
                activeImages.delete(fileName);
                onProgress({
                  completed: ++completed,
                  total: imagePaths.length,
                  currentFile: fileName,
                  active: [...activeImages],
                });
                continue;
              }
            }

            if (opArgs.length > 0 || outputFormat) {
              const fmtOut = outputFormat ? `${outputFormat}:${outPath}` : outPath;
              await spawnMagick([inputPath, ...opArgs, fmtOut], magickBucket ?? undefined);
              if (timings.enabled && magickBucket) {
                const imgEntry = timings.beginImage(inputPath);
                imgEntry.fileCheck(fileCheckMs);
                imgEntry.magick(magickBucket.ms);
                imgEntry.copy(0);
                imgEntry.done(Date.now() - imgT0);
              }
            } else {
              const copyT0 = timings.enabled ? Date.now() : 0;
              await fs.promises.copyFile(inputPath, outPath);
              if (timings.enabled && magickBucket) {
                const imgEntry = timings.beginImage(inputPath);
                imgEntry.fileCheck(fileCheckMs);
                imgEntry.magick(0);
                imgEntry.copy(Date.now() - copyT0);
                imgEntry.done(Date.now() - imgT0);
              }
            }
            outputFiles.push(outPath);
          }
        }
      } catch (err) {
        failures++;
        const msg = err instanceof Error ? err.message : String(err);
        errors.push(`${fileName}: ${msg}`);
        console.error(`[executor] Failed to process ${fileName}:`, err);
      }
      activeImages.delete(fileName);
      onProgress({
        completed: ++completed,
        total: imagePaths.length,
        currentFile: fileName,
        active: [...activeImages],
      });
    }
  }

  // Prevent magick's internal OpenMP thread pool from oversubscribing the CPU.
  // With N concurrent pipelines each trying to use all cores, we'd get N×cores threads
  // competing for cores threads — massive context-switching overhead.  Giving each
  // process an equal share of the hardware threads keeps total thread count at os.cpus().
  const threadsPerProcess = Math.max(1, Math.floor(os.cpus().length / concurrency));
  const prevThreadLimit = process.env.MAGICK_THREAD_LIMIT;
  process.env.MAGICK_THREAD_LIMIT = String(threadsPerProcess);
  try {
    await Promise.all(Array.from({ length: concurrency }, processOne));
  } finally {
    if (prevThreadLimit !== undefined) process.env.MAGICK_THREAD_LIMIT = prevThreadLimit;
    else delete process.env.MAGICK_THREAD_LIMIT;
  }
  // Write collected text output lines to disk (preserving input order).
  if (hasTextOutputNodes && collectedTextLines.length > 0) {
    collectedTextLines.sort((a, b) => a.index - b.index);
    const toNode = textOutputNodes[0];
    const toParams = toNode.data.params as Record<string, unknown>;
    // outputPath is the user-configured path in the node inspector;
    // fall back to file_path (old param name) then a safe default.
    const filePath = String(toParams.outputPath ?? toParams.file_path ?? 'output.txt');
    const appendMode = Boolean(toParams.append ?? false);
    const content = collectedTextLines.map((l) => l.value).join('\n') + '\n';
    try {
      const dir = path.dirname(path.resolve(filePath));
      await fs.promises.mkdir(dir, { recursive: true });
      await fs.promises.writeFile(filePath, content, { flag: appendMode ? 'a' : 'w' });
      outputFiles.push(filePath);
    } catch (err) {
      console.error('[executor] Failed to write text output file:', err);
    }
  }

  if (timings.enabled) {
    const resolvedOutputDir = outputDir ?? (outputFiles.length > 0 ? path.dirname(outputFiles[0]) : null);
    timings.endBatch(resolvedOutputDir);
  }
  return { processed: completed - failures - skipped, skipped, failed: failures, errors, outputFiles };
}
