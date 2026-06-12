import fs from 'node:fs';
import path from 'node:path';

import type { GraphNode, NodeGraph } from '../../shared/types.js';
import { EXECUTOR } from '../../shared/constants.js';
import type { NodeRegistry } from '../nodes/registry.js';
import {
  buildCommandArgs,
  buildCommandArgsFromJs,
  buildFormatConvertArgs,
  getFormatExtension,
} from './command-builder.js';
import { getExecutor } from './executorRegistry.js';
import {
  loadImageMeta,
  loadImageMean,
  loadImageChannelMean,
  loadMultipleChannelMeans,
  buildEmptyImageMeta,
  type ImageMeta,
} from './executor-compute.js';
import { resolveNodeParams } from './resolve-params.js';
import { spawnMagick } from './magick-spawn.js';
import { TEMP_DIR, shortHash } from './thumbnail-service.js';

export interface BatchContext {
  inputNodeId: string;
  outputNodeId: string;
  sorted: GraphNode[];
  outputContributorIds: Set<string>;
  registry: NodeRegistry;
  graph: NodeGraph;
  hasHeavyMetaNodes: boolean;
  hasImageMetaNodes: boolean;
  hasImageOutput: boolean;
  outputDir: string | null;
  overwrite: 'skip' | 'overwrite';
  isCancelled: () => boolean;
  // Per-spawn environment carrying MAGICK_THREAD_LIMIT for this run's concurrency.
  // Set by each pipeline after it computes its own concurrency, so overlapping
  // batches/previews never clobber a shared process.env.
  spawnEnv: NodeJS.ProcessEnv;
}

// Multi-stream execution for a single image with two speed optimisations:
//   1. Command fusion — consecutive standard nodes are chained into a single magick
//      invocation instead of one process per node (lazy-buffer approach).
//   2. Channel split — all 4 channels are extracted in one magick call via -write.
// Returns the final output path and extension, or null if the image should be suppressed.
export async function executeMultiStream(
  inputPath: string,
  imageIndex: number,
  ctx: BatchContext,
  extraSeeds?: Map<string, string>,
  verboseOut?: string[]
): Promise<{ resultPath: string; outputExt: string; cleanup: () => Promise<void> } | null> {
  const {
    inputNodeId,
    outputNodeId,
    sorted,
    outputContributorIds,
    registry,
    graph,
    hasHeavyMetaNodes,
    hasImageMetaNodes,
    spawnEnv,
  } = ctx;

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
  buffers.set(`${inputNodeId}:out-0`, inputPath);
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
    const matVerboseArgs = verboseOut ? ['-verbose'] : [];
    await spawnMagick([...matVerboseArgs, v.base, ...v.args, out], undefined, undefined, verboseOut, { env: spawnEnv });
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
      : `${inputNodeId}:out-0`;
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
    // light-meta nodes only — no ImageMagick spawn needed.
    meta = await buildEmptyImageMeta(inputPath);
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
    if (!def) continue;

    const { params, isImageNode } = resolveNodeParams(node, def, graph.edges, resolvedParams, meta);
    if (!isImageNode) continue;

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
          await spawnMagick(args, undefined, undefined, undefined, { env: spawnEnv });
          for (let k = 0; k < usedIdxs.length; k++) {
            buffers.set(`${node.id}:out-${usedIdxs[k]}`, outs[k]);
          }
        }
      }
    } else if (def.executor === EXECUTOR.CHANNEL_MERGE) {
      const refPath = await mat(`${inputNodeId}:out-0`);

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
      ], undefined, undefined, undefined, { env: spawnEnv });
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
        resolvedParams.set(node.id, { ...params, value });
      } catch (err) {
        console.warn(`[executor] loadImageMean failed for node ${node.id}:`, err);
      }
    } else if (def.executor === EXECUTOR.FORMAT_CONVERT) {
      // Format convert must materialise immediately (changes file type).
      const src = await getImg(node.id, 0);
      const fmt = String(params.format ?? 'PNG').toUpperCase();
      outputExt = getFormatExtension(fmt);
      const out = newTmp(outputExt);
      const fmtVerboseArgs = verboseOut ? ['-verbose'] : [];
      const fmtArgs = buildFormatConvertArgs(fmt, params);
      await spawnMagick([...fmtVerboseArgs, src, ...fmtArgs, `${fmt}:${out}`], undefined, undefined, verboseOut, {
        env: spawnEnv,
      });
      buffers.set(`${node.id}:out-0`, out);
    } else if (params._enabled !== false) {
      // Standard image op — fuse into a lazy chain when safe to do so.
      const imgInEdge = graph.edges.find((e) => e.target === node.id && e.targetHandle === 'in-0');
      const srcKey = imgInEdge ? `${imgInEdge.source}:${imgInEdge.sourceHandle ?? 'out-0'}` : `${inputNodeId}:out-0`;
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
      const srcKey = imgInEdge ? `${imgInEdge.source}:${imgInEdge.sourceHandle ?? 'out-0'}` : `${inputNodeId}:out-0`;
      buffers.set(`${node.id}:out-0`, buffers.get(srcKey) ?? inputPath);
    }
  }

  const outputEdge = graph.edges.find((e) => e.target === outputNodeId && e.targetHandle === 'in-0');
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
      const extVerboseArgs = verboseOut ? ['-verbose'] : [];
      await spawnMagick([...extVerboseArgs, finalVal, finalOut], undefined, undefined, verboseOut, { env: spawnEnv });
      return { resultPath: finalOut, outputExt, cleanup };
    }
    return { resultPath: finalVal, outputExt, cleanup };
  }

  // Final materialisation: use the correct output extension (not always .png).
  const finalOut = newTmp(outputExt);
  if (finalVal.args.length > 0) {
    const finalVerboseArgs = verboseOut ? ['-verbose'] : [];
    await spawnMagick(
      [...finalVerboseArgs, finalVal.base, ...finalVal.args, finalOut],
      undefined,
      undefined,
      verboseOut,
      { env: spawnEnv }
    );
  } else await fs.promises.copyFile(finalVal.base, finalOut);
  return { resultPath: finalOut, outputExt, cleanup };
}
