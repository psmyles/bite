import { describe, it, expect } from 'vitest';
import type { Node, Edge } from '@xyflow/svelte';
import { hasSetInputInChain } from '../renderer/workflowUtils.js';

function n(id: string, type: string): Node {
  return { id, type, position: { x: 0, y: 0 }, data: {} } as Node;
}
function e(source: string, target: string, sh = 'out-0', th = 'in-0'): Edge {
  return { id: `${source}-${target}`, source, target, sourceHandle: sh, targetHandle: th } as Edge;
}

describe('hasSetInputInChain', () => {
  it('detects a setInputNode upstream through image edges', () => {
    const nodes = [n('set-1', 'setInputNode'), n('op-1', 'resize'), n('out-1', 'imageOutputNode')];
    const edges = [e('set-1', 'op-1'), e('op-1', 'out-1')];
    expect(hasSetInputInChain(nodes, edges, 'out-1')).toBe(true);
  });

  it('returns false when only a plain inputNode is upstream', () => {
    const nodes = [n('in-1', 'inputNode'), n('out-1', 'imageOutputNode')];
    const edges = [e('in-1', 'out-1')];
    expect(hasSetInputInChain(nodes, edges, 'out-1')).toBe(false);
  });

  it('ignores param-, suf-in-, and prefix-in wires', () => {
    const nodes = [n('set-1', 'setInputNode'), n('out-1', 'imageOutputNode')];
    // Connected only via a suffix/param wire — not a real image-stream link.
    const edges = [e('set-1', 'out-1', 'out-0', 'suf-in-0')];
    expect(hasSetInputInChain(nodes, edges, 'out-1')).toBe(false);
  });

  it('does not hang on a cycle', () => {
    const nodes = [n('a', 'resize'), n('b', 'resize')];
    const edges = [e('a', 'b'), e('b', 'a')];
    expect(() => hasSetInputInChain(nodes, edges, 'a')).not.toThrow();
  });
});
