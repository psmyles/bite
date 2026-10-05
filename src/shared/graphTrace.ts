// Canonical graph-tracing helpers shared by the renderer (xyflow Node/Edge),
// the main process, and the CLI (GraphNode/GraphEdge). Uses minimal structural
// shapes so every caller's node/edge type satisfies it without conversion.

interface TraceNode {
  id: string;
  type?: string;
}
interface TraceEdge {
  source: string;
  target: string;
  sourceHandle?: string | null;
  targetHandle?: string | null;
}

/**
 * BFS backward from outputNodeId through image edges to find the first connected
 * inputNode. Returns that node's ID, or null if none found. Skips edges whose
 * source OR target handle is a `param-` wire (numeric/param connections).
 */
export function traceInputNodeId(nodes: TraceNode[], edges: TraceEdge[], outputNodeId: string): string | null {
  const visited = new Set<string>();
  const queue = [outputNodeId];
  while (queue.length > 0) {
    const id = queue.shift()!;
    if (visited.has(id)) continue;
    visited.add(id);
    if (nodes.find((n) => n.id === id)?.type === 'inputNode') return id;
    for (const e of edges) {
      if (e.target === id && !e.sourceHandle?.startsWith('param-') && !e.targetHandle?.startsWith('param-')) {
        queue.push(e.source);
      }
    }
  }
  return null;
}
