import { describe, it, expect } from 'vitest';
import type { Node, Edge, Connection } from '@xyflow/svelte';
import { handleToWireType, isValidConnection } from '../renderer/nodeEditor/connectionValidation.js';

function imgNode(id: string): Node {
  return {
    id,
    type: 'default',
    position: { x: 0, y: 0 },
    data: { inputs: ['image'], outputs: ['image'] },
  } as unknown as Node;
}

function paramNode(id: string, paramDefs: { name: string; type: string }[]): Node {
  return {
    id,
    type: 'default',
    position: { x: 0, y: 0 },
    data: { inputs: ['image'], outputs: ['image'], paramDefs },
  } as unknown as Node;
}

function conn(
  source: string,
  target: string,
  sourceHandle: string | null = 'out-0',
  targetHandle: string | null = 'in-0'
): Connection {
  return { source, target, sourceHandle, targetHandle };
}

function edge(source: string, target: string, sourceHandle = 'out-0', targetHandle = 'in-0'): Edge {
  return { id: `${source}-${target}`, source, target, sourceHandle, targetHandle } as unknown as Edge;
}

// ── handleToWireType ──────────────────────────────────────────────────────────

describe('handleToWireType', () => {
  it('folder-in → path', () => {
    expect(handleToWireType('n1', 'folder-in', 'target', [])).toBe('path');
  });

  it('prefix-in → string', () => {
    expect(handleToWireType('n1', 'prefix-in', 'target', [])).toBe('string');
  });

  it('suf-in-* → string', () => {
    expect(handleToWireType('n1', 'suf-in-0', 'target', [])).toBe('string');
  });

  it('txo-condition → boolean', () => {
    expect(handleToWireType('n1', 'txo-condition', 'target', [])).toBe('boolean');
  });

  it('txo-anything else → any', () => {
    expect(handleToWireType('n1', 'txo-somevalue', 'target', [])).toBe('any');
  });

  it('param-in-* with float paramDef → number', () => {
    const nodes = [paramNode('n1', [{ name: 'brightness', type: 'float' }])];
    expect(handleToWireType('n1', 'param-in-brightness', 'target', nodes)).toBe('number');
  });

  it('param-in-* with bool paramDef → boolean', () => {
    const nodes = [paramNode('n1', [{ name: 'enabled', type: 'bool' }])];
    expect(handleToWireType('n1', 'param-in-enabled', 'target', nodes)).toBe('boolean');
  });

  it('param-in-_enabled → boolean (built-in)', () => {
    expect(handleToWireType('n1', 'param-in-_enabled', 'target', [])).toBe('boolean');
  });

  it('image node source handle → image', () => {
    const nodes = [imgNode('n1')];
    expect(handleToWireType('n1', 'out-0', 'source', nodes)).toBe('image');
  });

  it('image node target handle → image', () => {
    const nodes = [imgNode('n1')];
    expect(handleToWireType('n1', 'in-0', 'target', nodes)).toBe('image');
  });
});

// ── isValidConnection ─────────────────────────────────────────────────────────

describe('isValidConnection', () => {
  it('self-loop is rejected', () => {
    const nodes = [imgNode('a')];
    expect(isValidConnection(conn('a', 'a'), nodes, [])).toBe(false);
  });

  it('cycle detection: A→B exists, B→A is rejected', () => {
    const nodes = [imgNode('a'), imgNode('b')];
    const edges = [edge('a', 'b')];
    expect(isValidConnection(conn('b', 'a'), nodes, edges)).toBe(false);
  });

  it('compatible types (image→image) → true', () => {
    const nodes = [imgNode('a'), imgNode('b')];
    expect(isValidConnection(conn('a', 'b'), nodes, [])).toBe(true);
  });

  it('incompatible types (number param out → image in) → false', () => {
    const nodes = [paramNode('a', [{ name: 'val', type: 'float' }]), imgNode('b')];
    expect(isValidConnection(conn('a', 'b', 'param-out-val', 'in-0'), nodes, [])).toBe(false);
  });

  it('channel_merge scalar→image in-N special case → true', () => {
    const mergeNode: Node = {
      id: 'merge',
      type: 'default',
      position: { x: 0, y: 0 },
      data: { inputs: ['image'], outputs: ['image'], definitionId: 'channel_merge' },
    } as unknown as Node;
    const srcNode = paramNode('src', [{ name: 'val', type: 'float' }]);
    const nodes = [srcNode, mergeNode];
    expect(isValidConnection(conn('src', 'merge', 'param-out-val', 'in-0'), nodes, [])).toBe(true);
  });

  it('any port constrained by incompatible wired sibling → false', () => {
    // Target node has two any-typed param inputs: alpha and beta.
    // alpha is already wired from a number source.
    // Connecting an image source to beta should be rejected.
    const srcImage = imgNode('img');
    const srcNumber = paramNode('num', [{ name: 'out', type: 'float' }]);
    const target = paramNode('tgt', [
      { name: 'alpha', type: 'any' },
      { name: 'beta', type: 'any' },
    ]);
    const nodes = [srcImage, srcNumber, target];
    const existingEdge = edge('num', 'tgt', 'param-out-out', 'param-in-alpha');
    expect(isValidConnection(conn('img', 'tgt', 'out-0', 'param-in-beta'), nodes, [existingEdge])).toBe(false);
  });

  it('any port with no constrained siblings → true', () => {
    const src = imgNode('src');
    const target = paramNode('tgt', [{ name: 'input', type: 'any' }]);
    const nodes = [src, target];
    expect(isValidConnection(conn('src', 'tgt', 'out-0', 'param-in-input'), nodes, [])).toBe(true);
  });

  it('compatible number connections → true', () => {
    const a = paramNode('a', [{ name: 'val', type: 'float' }]);
    const b = paramNode('b', [{ name: 'inp', type: 'int' }]);
    const nodes = [a, b];
    expect(isValidConnection(conn('a', 'b', 'param-out-val', 'param-in-inp'), nodes, [])).toBe(true);
  });
});
