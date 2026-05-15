import { ipcMain, dialog } from 'electron';
import type { BrowserWindow } from 'electron';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import type { GraphEdge, NodeGraph } from '../../shared/types.js';
import type { NodeRegistry } from '../nodes/registry.js';
import { topoSort } from '../pipeline/graph-utils.js';
import {
  computeNodeParams,
  loadImageMeta,
  loadImageMean,
  loadImageChannelMean,
  loadMultipleChannelMeans,
  getSeparator,
  buildEmptyImageMeta,
} from '../pipeline/executor-compute.js';
import { IPC } from '../../shared/constants.js';
import { writeOutputLog } from '../pipeline/output-log.js';

function valueToString(val: unknown): string {
  if (val === null || val === undefined) return '';
  if (typeof val === 'boolean') return val ? 'true' : 'false';
  if (typeof val === 'number') {
    return Number.isInteger(val) ? String(val) : val.toFixed(4).replace(/\.?0+$/, '');
  }
  if (Array.isArray(val)) {
    return (val as number[])
      .map((n) =>
        Number(n)
          .toFixed(4)
          .replace(/\.?0+$/, '')
      )
      .join(', ');
  }
  return String(val);
}

// prop_name and prop_path only need path.basename / the path itself — no magick spawn needed.
const PROP_PATH_ONLY = new Set(['prop_name', 'prop_path']);

interface ResolveContext {
  sorted: ReturnType<typeof topoSort>;
  needsMagickMeta: boolean;
  nodeMap: Map<string, ReturnType<typeof topoSort>[number]>;
  edgesByTarget: Map<string, GraphEdge[]>;
}

function buildResolveContext(graph: NodeGraph, registry: NodeRegistry): ResolveContext {
  const sorted = topoSort(graph.nodes, graph.edges);
  const needsMagickMeta = sorted.some((n) => {
    const exec = registry.get(n.data.definitionId)?.executor;
    return exec?.startsWith('prop_') && !PROP_PATH_ONLY.has(exec);
  });
  const nodeMap = new Map(graph.nodes.map((n) => [n.id, n]));
  const edgesByTarget = new Map<string, typeof graph.edges>();
  for (const edge of graph.edges) {
    const list = edgesByTarget.get(edge.target);
    if (list) list.push(edge);
    else edgesByTarget.set(edge.target, [edge]);
  }
  return { sorted, needsMagickMeta, nodeMap, edgesByTarget };
}

/** Resolve param values for all non-image nodes in the graph for a single image. */
async function resolveParamsForImage(
  graph: NodeGraph,
  imagePath: string,
  registry: NodeRegistry,
  ctx?: ResolveContext
): Promise<Map<string, Record<string, unknown>>> {
  const { sorted, needsMagickMeta, nodeMap, edgesByTarget } = ctx ?? buildResolveContext(graph, registry);
  const resolvedParams = new Map<string, Record<string, unknown>>();
  // Track image output slots blocked by a gate whose condition is false: "nodeId:handleId"
  const blockedImageSlots = new Set<string>();

  // mean_value handles its own image read via loadImageChannelMean — it does NOT use meta.
  // Excluded from needsMagickMeta so images blocked by a gate skip loadImageMeta entirely.
  const meta = needsMagickMeta ? await loadImageMeta(imagePath) : buildEmptyImageMeta(imagePath);

  // Pre-scan all mean_value nodes and batch their channel reads into a single spawn.
  const meanValueChannelMap = new Map<string, number>(); // nodeId → channelIdx (-1 = whole-image mean)
  const channelIndicesNeeded: number[] = [];
  for (const node of sorted) {
    const def = registry.get(node.data.definitionId);
    if (!def || def.executor !== 'mean_value') continue;
    const inEdges = edgesByTarget.get(node.id) ?? [];
    const imgInEdge = inEdges.find((e) => e.targetHandle === 'in-0');
    if (imgInEdge) {
      const srcNode = nodeMap.get(imgInEdge.source);
      const srcDef = registry.get(srcNode?.data.definitionId ?? '');
      const channelIdx = parseInt((imgInEdge.sourceHandle ?? '').replace('out-', ''), 10);
      if (srcDef?.executor === 'channel_split' && !isNaN(channelIdx)) {
        meanValueChannelMap.set(node.id, channelIdx);
        if (!channelIndicesNeeded.includes(channelIdx)) channelIndicesNeeded.push(channelIdx);
        continue;
      }
    }
    meanValueChannelMap.set(node.id, -1);
  }
  const batchedChannelMeans = new Map<number, number>();
  if (channelIndicesNeeded.length > 0) {
    try {
      channelIndicesNeeded.sort((a, b) => a - b);
      const means = await loadMultipleChannelMeans(imagePath, channelIndicesNeeded);
      channelIndicesNeeded.forEach((idx, i) => batchedChannelMeans.set(idx, means[i]));
    } catch {
      /* fallback to per-channel calls in the loop below */
    }
  }

  for (const node of sorted) {
    const def = registry.get(node.data.definitionId);
    if (!def) continue;

    const inEdges = edgesByTarget.get(node.id) ?? [];
    const rawParams: Record<string, unknown> = { ...(node.data.params ?? {}) };
    for (const edge of inEdges) {
      const th = edge.targetHandle ?? '';
      const sh = edge.sourceHandle ?? '';
      if (th.startsWith('param-in-') && sh.startsWith('param-out-')) {
        const srcResolved = resolvedParams.get(edge.source);
        if (srcResolved) {
          const sourceParam = sh.slice('param-out-'.length);
          if (sourceParam in srcResolved) rawParams[th.slice('param-in-'.length)] = srcResolved[sourceParam];
        }
      }
    }

    // Check whether this node's image input arrives from a blocked gate output
    const imageInputBlocked = inEdges.some(
      (e) => !e.targetHandle?.startsWith('param-') && blockedImageSlots.has(`${e.source}:${e.sourceHandle ?? 'out-0'}`)
    );

    // Gate: rawParams.condition is already resolved from param-wire edges above,
    // so this handles both wired conditions (upstream value propagated) and
    // static defaults (node's own param value). False → block image output.
    if (def.executor === 'gate') {
      if (!rawParams.condition) blockedImageSlots.add(`${node.id}:out-0`);
      resolvedParams.set(node.id, rawParams);
      continue;
    }

    // If image input is blocked, propagate blocking to this node's image outputs and skip
    if (imageInputBlocked) {
      for (let i = 0; i < def.outputs.length; i++) {
        if (def.outputs[i].type === 'image' || def.outputs[i].type === 'mask') {
          blockedImageSlots.add(`${node.id}:out-${i}`);
        }
      }
      resolvedParams.set(node.id, rawParams);
      continue;
    }

    const isImageNode =
      def.inputs.some((p) => p.type === 'image' || p.type === 'mask') ||
      def.outputs.some((p) => p.type === 'image' || p.type === 'mask');

    if (def.executor === 'mean_value') {
      try {
        const channelIdx = meanValueChannelMap.get(node.id) ?? -1;
        let value: number;
        if (channelIdx >= 0) {
          value = batchedChannelMeans.get(channelIdx) ?? (await loadImageChannelMean(imagePath, channelIdx));
        } else {
          value = await loadImageMean(imagePath);
        }
        resolvedParams.set(node.id, { ...rawParams, value });
      } catch {
        resolvedParams.set(node.id, rawParams);
      }
      continue;
    }

    const params = computeNodeParams(isImageNode ? undefined : def.executor, rawParams, meta);
    resolvedParams.set(node.id, params);
  }

  return resolvedParams;
}

function extractTextPortConfig(txNode: NodeGraph['nodes'][number], graph: NodeGraph, nodeId: string) {
  const p = (txNode.data.params ?? {}) as Record<string, unknown>;
  const separatorType = (p.separatorType as string) ?? 'comma';
  const customSep = (p.customSeparator as string) ?? '';
  const portIds = (p.portIds as string[]) ?? [];

  const connectedPorts = portIds
    .slice(0, -1)
    .filter((pid) => graph.edges.some((e) => e.target === nodeId && e.targetHandle === pid));
  const portSources = connectedPorts.map((portId) => {
    const edge = graph.edges.find((e) => e.target === nodeId && e.targetHandle === portId);
    if (!edge || !edge.sourceHandle?.startsWith('param-out-')) return null;
    return { sourceNodeId: edge.source, sourceParamKey: edge.sourceHandle.slice('param-out-'.length) };
  });
  const conditionEdge = graph.edges.find(
    (e) => e.target === nodeId && e.targetHandle === 'txo-condition' && e.sourceHandle?.startsWith('param-out-')
  );
  const conditionSource = conditionEdge
    ? {
        sourceNodeId: conditionEdge.source,
        sourceParamKey: conditionEdge.sourceHandle?.slice('param-out-'.length) ?? '',
      }
    : null;
  const sep = getSeparator(separatorType, customSep);

  return { connectedPorts, portSources, conditionSource, sep };
}

/** Compute the output lines for a Text Output node without writing anything. */
async function computeTextOutputLines(
  graph: NodeGraph,
  imagePaths: string[],
  nodeId: string,
  registry: NodeRegistry
): Promise<string[]> {
  const txNode = graph.nodes.find((n) => n.id === nodeId);
  if (!txNode) throw new Error('Text output node not found in graph.');

  if (imagePaths.length === 0) return [];

  const { connectedPorts, portSources, conditionSource, sep } = extractTextPortConfig(txNode, graph, nodeId);
  if (connectedPorts.length === 0) return [];

  const ctx = buildResolveContext(graph, registry);
  const allResolved = await Promise.all(
    imagePaths.map((imagePath) => resolveParamsForImage(graph, imagePath, registry, ctx))
  );

  const lines: string[] = [];
  for (const resolvedParams of allResolved) {
    if (conditionSource) {
      const condResolved = resolvedParams.get(conditionSource.sourceNodeId);
      const condVal = condResolved?.[conditionSource.sourceParamKey];
      if (!condVal) continue;
    }
    const values = portSources.map((ps) => {
      if (!ps) return '';
      const resolved = resolvedParams.get(ps.sourceNodeId);
      return valueToString(resolved?.[ps.sourceParamKey]);
    });
    lines.push(values.join(sep));
  }

  return lines;
}

export function registerTextOutputHandlers(registry: NodeRegistry, getWin: () => BrowserWindow | null): void {
  ipcMain.handle(IPC.TEXT_OUTPUT_BROWSE, async () => {
    const result = await dialog.showSaveDialog(getWin()!, {
      title: 'Choose Output File',
      filters: [
        { name: 'Text File', extensions: ['txt'] },
        { name: 'All Files', extensions: ['*'] },
      ],
      defaultPath: 'output.txt',
    });
    return result.canceled ? null : result.filePath;
  });

  ipcMain.handle(
    IPC.TEXT_OUTPUT_PREVIEW,
    async (_e, { graph, imagePaths, nodeId }: { graph: NodeGraph; imagePaths: string[]; nodeId: string }) => {
      return computeTextOutputLines(graph, imagePaths, nodeId, registry);
    }
  );

  let _writeCancelled = false;
  ipcMain.handle(IPC.TEXT_OUTPUT_WRITE_CANCEL, () => {
    _writeCancelled = true;
  });

  ipcMain.handle(
    IPC.TEXT_OUTPUT_WRITE,
    async (
      _e,
      {
        graph,
        imagePaths,
        nodeId,
        generateLog,
      }: { graph: NodeGraph; imagePaths: string[]; nodeId: string; generateLog?: boolean }
    ) => {
      const textT0 = Date.now();
      const txNode = graph.nodes.find((n) => n.id === nodeId);
      if (!txNode) throw new Error('Text output node not found in graph.');

      const p = (txNode.data.params ?? {}) as Record<string, unknown>;
      const outputPath = (p.outputPath as string) ?? '';

      if (!outputPath) throw new Error('No output path set on the Text Output node.');
      if (imagePaths.length === 0) throw new Error('No images are loaded.');

      const { connectedPorts, portSources, conditionSource, sep } = extractTextPortConfig(txNode, graph, nodeId);
      if (connectedPorts.length === 0) throw new Error('No input ports are connected.');

      _writeCancelled = false;
      const win = getWin();
      const ctx = buildResolveContext(graph, registry);
      const total = imagePaths.length;
      const concurrency = Math.min(os.cpus().length, total);
      const queue = [...imagePaths.keys()]; // shared index queue
      let done = 0;
      const collectedLines: { index: number; line: string }[] = [];

      const worker = async (): Promise<void> => {
        while (true) {
          if (_writeCancelled) return;
          const i = queue.shift();
          if (i === undefined) return;
          const resolvedParams = await resolveParamsForImage(graph, imagePaths[i], registry, ctx);
          done++;
          win?.webContents.send(IPC.TEXT_OUTPUT_WRITE_PROGRESS, { done, total });
          if (conditionSource) {
            const condVal = resolvedParams.get(conditionSource.sourceNodeId)?.[conditionSource.sourceParamKey];
            if (!condVal) continue;
          }
          const values = portSources.map((ps) => {
            if (!ps) return '';
            const resolved = resolvedParams.get(ps.sourceNodeId);
            return valueToString(resolved?.[ps.sourceParamKey]);
          });
          collectedLines.push({ index: i, line: values.join(sep) });
        }
      };

      await Promise.all(Array.from({ length: concurrency }, worker));
      const lines = collectedLines.sort((a, b) => a.index - b.index).map((r) => r.line);

      if (_writeCancelled) throw new Error('CANCELLED');

      if (lines.length === 0) throw new Error('No lines were written — all images were filtered out by the condition.');
      if (!lines.some((l) => l.trim() !== '')) throw new Error('All values resolved to empty — file not written.');

      let filePath = outputPath;
      if (!filePath.toLowerCase().endsWith('.txt')) filePath += '.txt';

      await fs.promises.writeFile(filePath, lines.join('\n') + '\n', 'utf-8');
      if (generateLog) {
        await writeOutputLog({
          outputFiles: [filePath],
          durationMs: Date.now() - textT0,
          outputDir: path.dirname(filePath),
        }).catch((e) => {
          console.error('[log] Failed to write output log:', e);
        });
      }
      return filePath;
    }
  );
}
