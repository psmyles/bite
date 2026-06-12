import type { Node, Edge } from '@xyflow/svelte';

// Canonical implementation lives in src/shared so the CLI and main process share it.
export { traceInputNodeId } from '../shared/graphTrace.js';

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
        !edge.targetHandle?.startsWith('suf-in-') &&
        edge.targetHandle !== 'prefix-in'
      ) {
        queue.push(edge.source);
      }
    }
  }
  return false;
}
