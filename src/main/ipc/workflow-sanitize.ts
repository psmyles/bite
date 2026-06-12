/**
 * Light structural validation + sanitisation of a loaded workflow graph.
 * Throws on malformed shapes (so we surface a clear error instead of confusing
 * downstream runtime failures) and strips any `__`-prefixed params — see
 * applyParamWires; a malicious .imgplex must not be able to inject `__compute_js__`.
 */
export function sanitizeWorkflowGraph(graph: unknown): unknown {
  if (!graph || typeof graph !== 'object') {
    throw new Error('Invalid workflow file: graph is not an object');
  }
  const g = graph as Record<string, unknown>;
  if (!Array.isArray(g.nodes) || !Array.isArray(g.edges)) {
    throw new Error('Invalid workflow file: graph.nodes and graph.edges must be arrays');
  }
  for (const node of g.nodes) {
    if (!node || typeof node !== 'object') throw new Error('Invalid workflow file: malformed node');
    const n = node as Record<string, unknown>;
    if (typeof n.id !== 'string') throw new Error('Invalid workflow file: node id must be a string');
    const data = n.data as Record<string, unknown> | undefined;
    const params = data?.params;
    if (params && typeof params === 'object') {
      for (const key of Object.keys(params as Record<string, unknown>)) {
        if (key.startsWith('__')) delete (params as Record<string, unknown>)[key];
      }
    }
  }
  for (const edge of g.edges) {
    if (!edge || typeof edge !== 'object') throw new Error('Invalid workflow file: malformed edge');
    const e = edge as Record<string, unknown>;
    if (typeof e.id !== 'string' || typeof e.source !== 'string' || typeof e.target !== 'string') {
      throw new Error('Invalid workflow file: edge id/source/target must be strings');
    }
  }
  return graph;
}
