import { describe, it, expect } from 'vitest';
import type { GraphEdge, GraphNode } from '../shared/types.js';
import { findOutputContributors, applyParamWires } from '../main/pipeline/graph-utils.js';

function edge(source: string, target: string, sh = 'out-0', th = 'in-0'): GraphEdge {
  return { id: `${source}-${target}`, source, target, sourceHandle: sh, targetHandle: th };
}

function node(id: string, params: Record<string, unknown> = {}): GraphNode {
  return { id, type: 'test', position: { x: 0, y: 0 }, data: { label: id, definitionId: id, params } };
}

// ── findOutputContributors ────────────────────────────────────────────────────

describe('findOutputContributors', () => {
  it('includes direct predecessor of start node', () => {
    const edges = [edge('a', 'b'), edge('b', 'workflow-output')];
    const result = findOutputContributors(edges, ['workflow-output']);
    expect(result.has('workflow-output')).toBe(true);
    expect(result.has('b')).toBe(true);
    expect(result.has('a')).toBe(true);
  });

  it('excludes disconnected nodes', () => {
    const edges = [edge('a', 'workflow-output')];
    const result = findOutputContributors(edges, ['workflow-output']);
    expect(result.has('orphan')).toBe(false);
  });

  it('handles multiple start nodes (text output + workflow-output)', () => {
    const edges = [edge('src-1', 'workflow-output'), edge('src-2', 'text-out')];
    const result = findOutputContributors(edges, ['workflow-output', 'text-out']);
    expect(result.has('src-1')).toBe(true);
    expect(result.has('src-2')).toBe(true);
  });

  it('handles cycles gracefully (no infinite loop)', () => {
    // Toposort prevents cycles in practice, but the BFS should not hang
    const edges = [edge('a', 'b'), edge('b', 'a'), edge('a', 'workflow-output')];
    expect(() => findOutputContributors(edges, ['workflow-output'])).not.toThrow();
  });
});

// ── applyParamWires ───────────────────────────────────────────────────────────

describe('applyParamWires', () => {
  it('starts with node params as base', () => {
    const n = node('x', { brightness: 0.5 });
    const result = applyParamWires(n, [], new Map());
    expect(result.brightness).toBe(0.5);
  });

  it('applies param-in override from a wired source', () => {
    const n = node('x', { strength: 1 });
    const upstream = new Map([['math-1', { output: 0.8 }]]);
    const e = edge('math-1', 'x', 'param-out-output', 'param-in-strength');
    const result = applyParamWires(n, [e], upstream);
    expect(result.strength).toBe(0.8);
  });

  it('applies txo- slot from a param-out wire', () => {
    const n = node('txo', { value: 0 });
    const upstream = new Map([['prop-1', { name: 'hello' }]]);
    const e = edge('prop-1', 'txo', 'param-out-name', 'txo-label');
    const result = applyParamWires(n, [e], upstream);
    expect(result['_txo_label']).toBe('hello');
  });

  it('ignores edges that are not param-out', () => {
    const n = node('x', { width: 100 });
    const upstream = new Map([['img-1', { anything: 999 }]]);
    const e = edge('img-1', 'x', 'out-0', 'in-0'); // image wire, not param
    const result = applyParamWires(n, [e], upstream);
    expect(result.width).toBe(100);
    expect(result.anything).toBeUndefined();
  });

  it('ignores edges targeting other nodes', () => {
    const n = node('x', { alpha: 1 });
    const upstream = new Map([['src', { val: 99 }]]);
    const e = edge('src', 'other-node', 'param-out-val', 'param-in-alpha');
    const result = applyParamWires(n, [e], upstream);
    expect(result.alpha).toBe(1); // unchanged
  });
});
