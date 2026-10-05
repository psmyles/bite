import { describe, it, expect } from 'vitest';
import { traceInputNodeId } from '../shared/graphTrace.js';

type N = { id: string; type?: string };
type E = { source: string; target: string; sourceHandle?: string | null; targetHandle?: string | null };

function edge(source: string, target: string, sh: string | null = 'out-0', th: string | null = 'in-0'): E {
  return { source, target, sourceHandle: sh, targetHandle: th };
}

describe('traceInputNodeId', () => {
  const nodes: N[] = [
    { id: 'in-1', type: 'inputNode' },
    { id: 'op-1', type: 'resize' },
    { id: 'out-1', type: 'imageOutputNode' },
  ];

  it('walks image edges back to the connected inputNode', () => {
    const edges = [edge('in-1', 'op-1'), edge('op-1', 'out-1')];
    expect(traceInputNodeId(nodes, edges, 'out-1')).toBe('in-1');
  });

  it('returns null when no inputNode is upstream', () => {
    const edges = [edge('op-1', 'out-1')];
    expect(traceInputNodeId(nodes, edges, 'out-1')).toBeNull();
  });

  it('skips edges whose source handle is a param wire', () => {
    // A numeric/param source feeding the output must not be mistaken for the image input.
    const ns: N[] = [
      { id: 'in-1', type: 'inputNode' },
      { id: 'math-1', type: 'math' },
      { id: 'out-1', type: 'imageOutputNode' },
    ];
    const edges = [edge('math-1', 'out-1', 'param-out-value', 'param-in-x')];
    expect(traceInputNodeId(ns, edges, 'out-1')).toBeNull();
  });

  it('skips edges whose target handle is a param wire', () => {
    const edges = [edge('in-1', 'out-1', 'out-0', 'param-in-x')];
    expect(traceInputNodeId(nodes, edges, 'out-1')).toBeNull();
  });

  it('does not hang on a cycle', () => {
    const edges = [edge('op-1', 'out-1'), edge('out-1', 'op-1')];
    expect(() => traceInputNodeId(nodes, edges, 'out-1')).not.toThrow();
  });
});
