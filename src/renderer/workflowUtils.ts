import type { Node, Edge } from '@xyflow/svelte';

/**
 * BFS backwards from nodeId through image edges; returns true if a
 * setInputNode is anywhere in the upstream chain.
 * Skips param and string (suf-in-*) wires — only follows image-stream edges.
 */
export function hasSetInputInChain(nodes: Node[], edges: Edge[], nodeId: string): boolean {
  const visited = new Set<string>();
  const queue = [nodeId];
  while (queue.length > 0) {
    const cur = queue.shift()!;
    if (visited.has(cur)) continue;
    visited.add(cur);
    if (nodes.find((n) => n.id === cur)?.type === 'setInputNode') return true;
    for (const edge of edges) {
      if (
        edge.target === cur &&
        !edge.sourceHandle?.startsWith('param-') &&
        !edge.targetHandle?.startsWith('param-') &&
        !edge.targetHandle?.startsWith('suf-in-')
      ) {
        queue.push(edge.source);
      }
    }
  }
  return false;
}

/**
 * BFS backwards from outputNodeId through image edges to find the first
 * connected inputNode. Returns that node's ID, or null if none found.
 * Skips edges whose sourceHandle starts with 'param-' (numeric/param wires).
 */
export function traceInputNodeId(nodes: Node[], edges: Edge[], outputNodeId: string): string | null {
  const visited = new Set<string>();
  const queue = [outputNodeId];
  while (queue.length > 0) {
    const nodeId = queue.shift()!;
    if (visited.has(nodeId)) continue;
    visited.add(nodeId);
    const node = nodes.find((n) => n.id === nodeId);
    if (node?.type === 'inputNode') return nodeId;
    for (const edge of edges) {
      if (
        edge.target === nodeId &&
        !edge.sourceHandle?.startsWith('param-') &&
        !edge.targetHandle?.startsWith('param-')
      ) {
        queue.push(edge.source);
      }
    }
  }
  return null;
}
