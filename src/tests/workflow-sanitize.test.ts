import { describe, it, expect } from 'vitest';
import { sanitizeWorkflowGraph } from '../main/ipc/workflow-sanitize.js';

function graph(nodes: unknown[], edges: unknown[] = []) {
  return { nodes, edges, viewport: { x: 0, y: 0, zoom: 1 } };
}

describe('sanitizeWorkflowGraph', () => {
  it('strips __compute_js__ (and any __-prefixed key) from node params', () => {
    const g = graph([
      { id: 'n1', type: 'math', data: { params: { value: 1, __compute_js__: 'return globalThis.evil()' } } },
    ]);
    const out = sanitizeWorkflowGraph(g) as typeof g;
    const params = (out.nodes[0] as { data: { params: Record<string, unknown> } }).data.params;
    expect(params.value).toBe(1);
    expect('__compute_js__' in params).toBe(false);
  });

  it('keeps legitimate params untouched', () => {
    const g = graph([{ id: 'n1', type: 'resize', data: { params: { width: 100, keepAspect: true } } }]);
    const out = sanitizeWorkflowGraph(g) as typeof g;
    expect((out.nodes[0] as { data: { params: Record<string, unknown> } }).data.params).toEqual({
      width: 100,
      keepAspect: true,
    });
  });

  it('throws when graph is not an object', () => {
    expect(() => sanitizeWorkflowGraph(null)).toThrow();
    expect(() => sanitizeWorkflowGraph('nope')).toThrow();
  });

  it('throws when nodes/edges are not arrays', () => {
    expect(() => sanitizeWorkflowGraph({ nodes: {}, edges: [] })).toThrow();
    expect(() => sanitizeWorkflowGraph({ nodes: [], edges: 5 })).toThrow();
  });

  it('throws on a node with a non-string id', () => {
    expect(() => sanitizeWorkflowGraph(graph([{ id: 42, data: { params: {} } }]))).toThrow();
  });

  it('throws on an edge missing source/target', () => {
    expect(() => sanitizeWorkflowGraph(graph([], [{ id: 'e1', source: 'a' }]))).toThrow();
  });

  it('accepts a valid minimal graph', () => {
    const g = graph([{ id: 'n1', data: { params: {} } }], [{ id: 'e1', source: 'n1', target: 'n2' }]);
    expect(() => sanitizeWorkflowGraph(g)).not.toThrow();
  });
});
