import path from 'node:path';
import type { GraphEdge, GraphNode } from '../../shared/types.js';

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
