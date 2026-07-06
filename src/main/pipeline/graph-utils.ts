import path from 'node:path';
import type { GraphEdge, GraphNode } from '../../shared/types.js';
import { log } from '../logger.js';

/** Forward BFS from startNodeIds; returns those nodes plus all of their descendants. */
export function findDescendants(edges: GraphEdge[], startNodeIds: string[]): Set<string> {
  const descendants = new Set<string>();
  const queue = [...startNodeIds];
  while (queue.length > 0) {
    const id = queue.shift()!;
    if (descendants.has(id)) continue;
    descendants.add(id);
    for (const e of edges) {
      if (e.source === id) queue.push(e.target);
    }
  }
  return descendants;
}

/** Backward BFS from startNodeIds; returns all nodes that contribute to those outputs. */
export function findOutputContributors(edges: GraphEdge[], startNodeIds: string[]): Set<string> {
  const contributors = new Set<string>();
  const queue = [...startNodeIds];
  while (queue.length > 0) {
    const id = queue.shift()!;
    if (contributors.has(id)) continue;
    contributors.add(id);
    for (const e of edges) {
      if (e.target === id) queue.push(e.source);
    }
  }
  return contributors;
}

/**
 * Merge param-wire connections into a node's raw params.
 * Handles both param-in-* (inspector overrides) and txo-* (text output port slots).
 */
export function applyParamWires(
  node: GraphNode,
  edges: GraphEdge[],
  resolvedParams: Map<string, Record<string, unknown>>
): Record<string, unknown> {
  const rawParams: Record<string, unknown> = { ...node.data.params };
  // Security: never let `__`-prefixed keys survive from workflow data. The only
  // legitimate one (`__compute_js__`) is injected at runtime from the trusted node
  // definition (resolve-params.ts). A malicious .imgplex could otherwise set it on
  // any node's params and achieve arbitrary code execution in the main process.
  for (const key of Object.keys(rawParams)) {
    if (key.startsWith('__')) delete rawParams[key];
  }
  for (const edge of edges) {
    if (edge.target !== node.id) continue;
    const th = edge.targetHandle ?? '';
    const sh = edge.sourceHandle ?? '';
    if (sh.startsWith('param-out-')) {
      const sourceParam = sh.slice('param-out-'.length);
      const srcResolved = resolvedParams.get(edge.source);
      if (srcResolved && sourceParam in srcResolved) {
        if (th.startsWith('param-in-')) {
          const destKey = th.slice('param-in-'.length);
          // Security: a wire target handle is attacker-controlled data from the
          // .imgplex file. Never let it write a `__`-prefixed key (e.g.
          // `param-in-__compute_js__`), which would inject executable JS back in
          // after the strip above.
          if (destKey.startsWith('__')) continue;
          rawParams[destKey] = srcResolved[sourceParam];
        } else if (th.startsWith('txo-')) {
          rawParams[`_txo_${th.slice('txo-'.length)}`] = srcResolved[sourceParam];
        }
      }
    }
  }
  return rawParams;
}

export function topoSort(nodes: GraphNode[], edges: GraphEdge[]): GraphNode[] {
  const inDegree = new Map<string, number>();
  const adj = new Map<string, string[]>();

  for (const node of nodes) {
    inDegree.set(node.id, 0);
    adj.set(node.id, []);
  }
  for (const edge of edges) {
    adj.get(edge.source)?.push(edge.target);
    inDegree.set(edge.target, (inDegree.get(edge.target) ?? 0) + 1);
  }

  const queue = [...inDegree.entries()].filter(([, d]) => d === 0).map(([id]) => id);
  const nodeMap = new Map(nodes.map((n) => [n.id, n]));
  const sorted: GraphNode[] = [];

  while (queue.length > 0) {
    const id = queue.shift()!;
    const node = nodeMap.get(id);
    if (node) sorted.push(node);
    for (const neighbor of adj.get(id) ?? []) {
      const deg = (inDegree.get(neighbor) ?? 0) - 1;
      inDegree.set(neighbor, deg);
      if (deg === 0) queue.push(neighbor);
    }
  }

  // Kahn's algorithm leaves cycle members out of `sorted`. A cycle shouldn't be
  // reachable via the UI, but a hand-edited/corrupt .imgplex can introduce one —
  // warn rather than silently dropping nodes from execution.
  if (sorted.length !== nodes.length) {
    log('warn', `[topoSort] graph contains a cycle: ${nodes.length - sorted.length} node(s) dropped from execution`);
  }

  return sorted;
}

export function groupBySetPattern(
  imagePaths: string[],
  prefix: string,
  suffixes: string[]
): Map<string, Record<string, string>> {
  const map = new Map<string, Record<string, string>>();
  for (const imgPath of imagePaths) {
    const ext = path.extname(imgPath);
    const base = path.basename(imgPath, ext);
    if (prefix && !base.startsWith(prefix)) continue;
    const rest = base.slice(prefix.length);
    for (const s of suffixes) {
      if (!s) continue;
      if (rest.endsWith(s)) {
        const mid = rest.slice(0, rest.length - s.length);
        if (!map.has(mid)) map.set(mid, {});
        map.get(mid)![s] = imgPath;
        break;
      }
    }
  }
  return map;
}
