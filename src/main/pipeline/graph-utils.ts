import path from 'node:path';
import type { GraphEdge, GraphNode, NodeGraph } from '../../shared/types.js';

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
  for (const edge of edges) {
    if (edge.target !== node.id) continue;
    const th = edge.targetHandle ?? '';
    const sh = edge.sourceHandle ?? '';
    if (sh.startsWith('param-out-')) {
      const sourceParam = sh.slice('param-out-'.length);
      const srcResolved = resolvedParams.get(edge.source);
      if (srcResolved && sourceParam in srcResolved) {
        if (th.startsWith('param-in-')) {
          rawParams[th.slice('param-in-'.length)] = srcResolved[sourceParam];
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

  return sorted;
}

/**
 * BFS backwards from outputNodeId through image edges to find the connected inputNode.
 * Returns the inputNode's ID, or null if none found.
 */
export function traceInputNode(graph: NodeGraph, outputNodeId: string): string | null {
  const visited = new Set<string>();
  const queue = [outputNodeId];
  while (queue.length > 0) {
    const nodeId = queue.shift()!;
    if (visited.has(nodeId)) continue;
    visited.add(nodeId);
    const node = graph.nodes.find((n) => n.id === nodeId);
    if (node?.type === 'inputNode') return nodeId;
    for (const edge of graph.edges) {
      if (edge.target === nodeId && !edge.sourceHandle?.startsWith('param-')) {
        queue.push(edge.source);
      }
    }
  }
  return null;
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
