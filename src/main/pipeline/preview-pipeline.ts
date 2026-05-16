import fs from 'node:fs';
import path from 'node:path';

import type { NodeGraph } from '../../shared/types.js';
import { log } from '../logger.js';
import { PREVIEW_MAX_EDGE_PX, EXECUTOR } from '../../shared/constants.js';
import { topoSort, applyParamWires } from './graph-utils.js';
import type { NodeRegistry } from '../nodes/registry.js';
import { buildCommandArgs, buildCommandArgsFromJs } from './command-builder.js';
import { getExecutor } from './executorRegistry.js';
import { PreviewCache } from './cache.js';
import { computeNodeParams, loadImageMeta, loadImageMean } from './executor-compute.js';
import { spawnMagick } from './magick-spawn.js';
import { fileToDataUrl } from './image-header.js';
import { TEMP_DIR, shortHash } from './thumbnail-service.js';

export async function executePreview(
  previewCache: PreviewCache,
  graph: NodeGraph,
  imagePath: string,
  registry: NodeRegistry,
  fromNodeId?: string
): Promise<{ dataUrl: string; propParams: Record<string, Record<string, unknown>> }> {
  await fs.promises.mkdir(TEMP_DIR, { recursive: true });
  log(
    'info',
    `[preview] start: ${path.basename(imagePath)} | ${graph.nodes.length} node(s)${fromNodeId ? ` (from ${fromNodeId})` : ''}`
  );

  // Always downscale proportionally first — this keeps the data URL small and
  // ensures the image has the correct aspect ratio (no square padding).
  const inputHash = shortHash(imagePath);
  const downscaledPath = path.join(TEMP_DIR, `preview_input_${inputHash}.png`);
  const downscaledExists = await fs.promises
    .access(downscaledPath)
    .then(() => true)
    .catch(() => false);

  if (!downscaledExists) {
    try {
      await spawnMagick([
        `${imagePath}[0]`,
        '-resize',
        `${PREVIEW_MAX_EDGE_PX}x${PREVIEW_MAX_EDGE_PX}>`,
        downscaledPath,
      ]);
    } catch (err) {
      // Concurrent preview request may have written the file first; retry access
      const nowExists = await fs.promises
        .access(downscaledPath)
        .then(() => true)
        .catch(() => false);
      if (!nowExists) throw err;
    }
  }

  // No nodes — return the proportionally-downscaled source directly.
  if (graph.nodes.length === 0) return { dataUrl: await fileToDataUrl(downscaledPath), propParams: {} };

  const sorted = topoSort(graph.nodes, graph.edges);

  // Invalidate stale cache entries when a specific node changed
  if (fromNodeId) {
    const changeIdx = sorted.findIndex((n) => n.id === fromNodeId);
    if (changeIdx >= 0) {
      for (let i = changeIdx; i < sorted.length; i++) {
        previewCache.invalidateFrom(sorted[i].id);
      }
    }
  }

  // Multi-stream image buffer: keyed "nodeId:out-N" → temp file path.
  // This allows fan-out (channel_split) and fan-in (channel_merge) topologies.
  const imageBuffers = new Map<string, string>();
  imageBuffers.set('workflow-input:out-0', downscaledPath);

  // In set mode, seed companion images for each suffix port of the setInputNode.
  // Infer middle name from the selected imagePath so we can locate sibling files.
  const previewSetNode = sorted.find((n) => n.data.definitionId === EXECUTOR.PROCESS_AS_SET);
  if (previewSetNode) {
    const setPrefix = String(previewSetNode.data.params?.prefix ?? '');
    const setSuffixes = Array.isArray(previewSetNode.data.params?.suffixes)
      ? (previewSetNode.data.params!.suffixes as string[]).filter(Boolean)
      : [];
    const imgExt = path.extname(imagePath);
    const imgBase = path.basename(imagePath, imgExt);
    const imgDir = path.dirname(imagePath);
    const imgRest = imgBase.startsWith(setPrefix) ? imgBase.slice(setPrefix.length) : imgBase;
    let middleName: string | null = null;
    for (const s of setSuffixes) {
      if (imgRest.endsWith(s)) {
        middleName = imgRest.slice(0, imgRest.length - s.length);
        break;
      }
    }
    if (middleName !== null) {
      for (let i = 0; i < setSuffixes.length; i++) {
        const companionPath = path.join(imgDir, setPrefix + middleName + setSuffixes[i] + imgExt);
        const companionExists = await fs.promises
          .access(companionPath)
          .then(() => true)
          .catch(() => false);
        if (!companionExists) continue;
        const companionHash = shortHash(companionPath);
        const companionDownscaled = path.join(TEMP_DIR, `preview_input_${companionHash}.png`);
        if (
          !(await fs.promises
            .access(companionDownscaled)
            .then(() => true)
            .catch(() => false))
        ) {
          try {
            await spawnMagick([
              `${companionPath}[0]`,
              '-resize',
              `${PREVIEW_MAX_EDGE_PX}x${PREVIEW_MAX_EDGE_PX}>`,
              companionDownscaled,
            ]);
          } catch {
            /* companion downscale failure is non-fatal */
          }
        }
        if (
          await fs.promises
            .access(companionDownscaled)
            .then(() => true)
            .catch(() => false)
        ) {
          imageBuffers.set(`${previewSetNode.id}:out-${i}`, companionDownscaled);
        }
      }
    }
  }

  // Resolve the file path feeding into a node's Nth image input by following edges.
  const getImgBuf = (nodeId: string, inputIdx: number): string => {
    const edge = graph.edges.find((e) => e.target === nodeId && e.targetHandle === `in-${inputIdx}`);
    if (!edge) return downscaledPath;
    return imageBuffers.get(`${edge.source}:${edge.sourceHandle ?? 'out-0'}`) ?? downscaledPath;
  };

  const fileExists = (p: string) =>
    fs.promises
      .access(p)
      .then(() => true)
      .catch(() => false);

  // Tracks resolved params per node so downstream param-wire consumers can read them.
  const resolvedParams = new Map<string, Record<string, unknown>>();

  // Load image metadata lazily — only when at least one Properties node is present.
  const needsMeta = sorted.some((n) => {
    const def = registry.get(n.data.definitionId);
    return def?.needs_image_meta === true || def?.executor === EXECUTOR.MEAN_VALUE;
  });
  const meta = needsMeta ? await loadImageMeta(imagePath) : undefined;

  for (const node of sorted) {
    // process_as_set is a source node — buffers are pre-seeded above; skip processing.
    if (node.data.definitionId === EXECUTOR.PROCESS_AS_SET) continue;
    const def = registry.get(node.data.definitionId);
    if (!def) {
      // Silently skip framework-internal nodes (workflow-input/output, groups)
      // which have no registry entry. Only warn for genuinely unknown IDs.
      if (node.data.definitionId) console.warn(`[executor] Unknown node definition: ${node.data.definitionId}`);
      continue;
    }

    // Inspector values, overridden by any incoming param-wire connections
    const rawParams = applyParamWires(node, graph.edges, resolvedParams);

    // Run pure math/logic/value computation; image nodes return params unchanged
    const isImageNode =
      def.inputs.some((p) => p.type === 'image' || p.type === 'mask') ||
      def.outputs.some((p) => p.type === 'image' || p.type === 'mask');
    // Inject compute_js body for pure-value nodes that use the inline JS path
    const computeInput = !isImageNode && def.compute_js ? { ...rawParams, __compute_js__: def.compute_js } : rawParams;
    const params = computeNodeParams(isImageNode ? undefined : def.executor, computeInput, meta);
    resolvedParams.set(node.id, params);

    // Pure value/math/logic nodes don't touch the image pipeline
    if (!isImageNode) continue;

    // ── Channel Split ──────────────────────────────────────────────────────
    if (def.executor === EXECUTOR.CHANNEL_SPLIT) {
      const inputPath = getImgBuf(node.id, 0);
      const nodeHash = shortHash(inputPath + JSON.stringify(params));
      const CHANNELS = [
        { suffix: '__r', magickChannel: 'Red' },
        { suffix: '__g', magickChannel: 'Green' },
        { suffix: '__b', magickChannel: 'Blue' },
        { suffix: '__a', magickChannel: 'Alpha' },
      ] as const;
      for (let i = 0; i < CHANNELS.length; i++) {
        const { suffix, magickChannel } = CHANNELS[i];
        const cacheKey = node.id + suffix;
        const cached = previewCache.get(cacheKey, nodeHash);
        let outPath: string;
        if (cached && (await fileExists(cached))) {
          outPath = cached;
        } else {
          outPath = path.join(TEMP_DIR, `preview_${node.id}${suffix}_${nodeHash}.png`);
          await spawnMagick([inputPath, '-channel', magickChannel, '-separate', outPath]);
          previewCache.set(cacheKey, nodeHash, outPath);
        }
        imageBuffers.set(`${node.id}:out-${i}`, outPath);
      }
      continue;
    }

    // ── Channel Merge ──────────────────────────────────────────────────────
    if (def.executor === EXECUTOR.CHANNEL_MERGE) {
      const refPath = imageBuffers.get('workflow-input:out-0') ?? downscaledPath;

      // Returns the image for a channel port, or a solid-black placeholder when unconnected.
      const resolveChannel = async (inputIdx: number): Promise<string> => {
        const imgEdge = graph.edges.find((e) => e.target === node.id && e.targetHandle === `in-${inputIdx}`);
        if (imgEdge) {
          // Float value wire (param-out-*) — convert to a solid gray image at 0–1 brightness
          if (imgEdge.sourceHandle?.startsWith('param-out-')) {
            const paramKey = imgEdge.sourceHandle.slice('param-out-'.length);
            const srcParams = resolvedParams.get(imgEdge.source);
            const fillVal = Math.max(0, Math.min(1, Number(srcParams?.[paramKey] ?? 0)));
            const pct = Math.round(fillVal * 100);
            const floatKey = `${node.id}__float_${inputIdx}`;
            const floatHash = shortHash(refPath + String(pct));
            const cachedFloat = previewCache.get(floatKey, floatHash);
            if (cachedFloat && (await fileExists(cachedFloat))) return cachedFloat;
            const floatPath = path.join(TEMP_DIR, `preview_${floatKey}_${floatHash}.png`);
            await spawnMagick([refPath, '-evaluate', 'set', `${pct}%`, '-colorspace', 'Gray', floatPath]);
            previewCache.set(floatKey, floatHash, floatPath);
            return floatPath;
          }
          return imageBuffers.get(`${imgEdge.source}:${imgEdge.sourceHandle ?? 'out-0'}`) ?? refPath;
        }
        // Unconnected — solid black placeholder sized to match the workflow input
        const solidKey = `${node.id}__solid_${inputIdx}`;
        const solidHash = shortHash(refPath);
        const cachedSolid = previewCache.get(solidKey, solidHash);
        if (cachedSolid && (await fileExists(cachedSolid))) return cachedSolid;
        const solidPath = path.join(TEMP_DIR, `preview_${solidKey}_${solidHash}.png`);
        await spawnMagick([refPath, '-evaluate', 'set', '0%', '-colorspace', 'Gray', solidPath]);
        previewCache.set(solidKey, solidHash, solidPath);
        return solidPath;
      };

      const r = await resolveChannel(0);
      const g = await resolveChannel(1);
      const b = await resolveChannel(2);
      const channelCount = Number(params.channels ?? 3);
      const aImgEdge = graph.edges.find((e) => e.target === node.id && e.targetHandle === 'in-3');
      // Alpha only when the node is in 4-channel mode and the A port is connected
      const hasAlpha = channelCount >= 4 && !!aImgEdge;
      const a = hasAlpha ? await resolveChannel(3) : null;

      const nodeHash = shortHash(r + g + b + (a ?? '') + JSON.stringify(params));
      const cached = previewCache.get(node.id, nodeHash);
      let outputPath: string;
      if (cached && (await fileExists(cached))) {
        outputPath = cached;
      } else {
        outputPath = path.join(TEMP_DIR, `preview_${node.id}_${nodeHash}.png`);
        const mergeArgs = hasAlpha
          ? [r, g, b, a!, '-set', 'colorspace', 'sRGB', '-combine', '-alpha', 'on', outputPath]
          : [r, g, b, '-set', 'colorspace', 'sRGB', '-combine', outputPath];
        await spawnMagick(mergeArgs);
        previewCache.set(node.id, nodeHash, outputPath);
      }
      imageBuffers.set(`${node.id}:out-0`, outputPath);
      continue;
    }

    // ── Mean Value — reads channel mean, no image output ──────────────────
    if (def.executor === EXECUTOR.MEAN_VALUE) {
      const inputPath = getImgBuf(node.id, 0);
      try {
        const value = await loadImageMean(inputPath);
        resolvedParams.set(node.id, { ...rawParams, value });
      } catch (err) {
        console.warn(`[executor] loadImageMean failed for node ${node.id}:`, err);
      }
      continue;
    }

    // ── Standard single-in / single-out node ──────────────────────────────
    const inputPath = getImgBuf(node.id, 0);
    // Hash the *actual input* (previous node's output) so that any change to the
    // upstream chain automatically invalidates this node's cached result.
    const nodeHash = shortHash(inputPath + JSON.stringify(params));
    const cached = previewCache.get(node.id, nodeHash);

    if (cached && (await fileExists(cached))) {
      imageBuffers.set(`${node.id}:out-0`, cached);
      continue;
    }

    const outputPath = path.join(TEMP_DIR, `preview_${node.id}_${nodeHash}.png`);

    if (params._enabled === false) {
      // Bypassed: pass image through unchanged
      await fs.promises.copyFile(inputPath, outputPath);
    } else if (def.executor === EXECUTOR.FORMAT_CONVERT) {
      // Preview output is always a .png temp file, so just re-encode at the requested quality.
      // The format label is ignored here — the file extension (.png) is authoritative for magick.
      const quality = Number(params.quality ?? 90);
      await spawnMagick([inputPath, '-quality', String(quality), outputPath]);
    } else {
      const registeredFn = def.executor ? getExecutor(def.executor) : undefined;
      const opArgs = registeredFn
        ? registeredFn(def, params)
        : def.command_js
          ? buildCommandArgsFromJs(def, params)
          : buildCommandArgs(def, params);
      if (opArgs.length > 0) {
        await spawnMagick([inputPath, ...opArgs, outputPath]);
      } else {
        // executor-type node or empty template — pass through unchanged
        await fs.promises.copyFile(inputPath, outputPath);
      }
    }

    previewCache.set(node.id, nodeHash, outputPath);
    imageBuffers.set(`${node.id}:out-0`, outputPath);
  }

  // Resolve the final output from the edge feeding into workflow-output's image input.
  // When the graph is filtered to a preview sub-graph, workflow-output is excluded, so
  // fall back to the last image-producing node in topological order.
  const outputEdge = graph.edges.find((e) => e.target === 'workflow-output' && e.targetHandle === 'in-0');
  let finalPath: string;
  if (outputEdge) {
    finalPath = imageBuffers.get(`${outputEdge.source}:${outputEdge.sourceHandle ?? 'out-0'}`) ?? downscaledPath;
  } else {
    finalPath = downscaledPath;
    for (let i = sorted.length - 1; i >= 0; i--) {
      const p = imageBuffers.get(`${sorted[i].id}:out-0`);
      if (p && p !== downscaledPath) {
        finalPath = p;
        break;
      }
    }
  }

  // Collect computed output values for nodes that produce no image outputs
  // (pure value/logic/property nodes, plus mean_value which consumes an image
  // but emits only a scalar). These are displayed live on canvas nodes.
  const propParams: Record<string, Record<string, unknown>> = {};
  for (const node of sorted) {
    const def = registry.get(node.data.definitionId);
    if (!def) continue;
    const hasImageOutput = def.outputs.some((p) => p.type === 'image' || p.type === 'mask');
    if (!hasImageOutput) {
      const resolved = resolvedParams.get(node.id);
      if (resolved) propParams[node.id] = resolved;
    }
  }

  log('info', `[preview] done: ${path.basename(imagePath)}`);
  return { dataUrl: await fileToDataUrl(finalPath), propParams };
}
