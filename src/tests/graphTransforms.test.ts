import { describe, it, expect } from 'vitest';
import type { Node, Edge } from '@xyflow/svelte';
import {
  computeDeleteSelected,
  computeDuplicateNodes,
  makeWorkflowCliName,
} from '../renderer/nodeEditor/graphTransforms.js';

function n(id: string, over: Partial<Node> = {}): Node {
  return { id, type: 'process', position: { x: 0, y: 0 }, data: {}, ...over } as Node;
}
function e(id: string, source: string, target: string, over: Partial<Edge> = {}): Edge {
  return { id, source, target, ...over } as Edge;
}
const allow = () => true;

describe('computeDeleteSelected', () => {
  it('no-ops when nothing relevant is selected', () => {
    const nodes = [n('a'), n('b')];
    const edges = [e('e1', 'a', 'b')];
    const r = computeDeleteSelected(nodes, edges, allow);
    expect(r.changed).toBe(false);
    expect(r.nodes).toBe(nodes); // unchanged reference
  });

  it('reports deleted inputNode ids (the keyboard-delete leak fix)', () => {
    const nodes = [n('in1', { type: 'inputNode', selected: true }), n('out', { type: 'imageOutputNode' })];
    const r = computeDeleteSelected(nodes, [], allow);
    expect(r.changed).toBe(true);
    expect(r.deletedInputIds).toEqual(['in1']);
    expect(r.nodes.map((x) => x.id)).toEqual(['out']);
  });

  it('respects the deletion guard', () => {
    const nodes = [n('in1', { type: 'inputNode', selected: true })];
    const r = computeDeleteSelected(nodes, [], () => false);
    expect(r.changed).toBe(false);
    expect(r.deletedInputIds).toEqual([]);
  });

  it('drops edges touching a deleted node', () => {
    const nodes = [n('a', { selected: true }), n('b')];
    const edges = [e('e1', 'a', 'b'), e('e2', 'b', 'c')];
    const r = computeDeleteSelected(nodes, edges, allow);
    expect(r.edges.map((x) => x.id)).toEqual(['e2']);
  });

  it('deletes selected standalone edges without touching nodes', () => {
    const nodes = [n('a'), n('b')];
    const edges = [e('e1', 'a', 'b', { selected: true })];
    const r = computeDeleteSelected(nodes, edges, allow);
    expect(r.changed).toBe(true);
    expect(r.edges).toHaveLength(0);
    expect(r.nodes.map((x) => x.id)).toEqual(['a', 'b']);
  });

  it('ungroupes children of a deleted group into absolute coordinates', () => {
    const nodes = [
      n('g', { type: 'group', selected: true, position: { x: 100, y: 50 } }),
      n('child', { parentId: 'g', position: { x: 10, y: 5 } }),
    ];
    const r = computeDeleteSelected(nodes, [], allow);
    const child = r.nodes.find((x) => x.id === 'child')!;
    expect(r.nodes.map((x) => x.id)).toEqual(['child']);
    expect(child.position).toEqual({ x: 110, y: 55 });
    expect(child.parentId).toBeUndefined();
  });
});

describe('computeDuplicateNodes', () => {
  it('no-ops on empty targets', () => {
    const nodes = [n('a')];
    const r = computeDuplicateNodes(nodes, [], 123);
    expect(r.changed).toBe(false);
    expect(r.nodes).toBe(nodes);
  });

  it('appends offset, selected copies and deselects originals', () => {
    const nodes = [n('a', { position: { x: 0, y: 0 } })];
    const r = computeDuplicateNodes(nodes, [nodes[0]], 99);
    expect(r.nodes).toHaveLength(2);
    expect(r.nodes[0].selected).toBe(false);
    const dup = r.nodes[1];
    expect(dup.id).toBe('a-dup990');
    expect(dup.position).toEqual({ x: 20, y: 20 });
    expect(dup.selected).toBe(true);
  });

  it('pulls in group children and re-parents them to the duplicated group', () => {
    const group = n('g', { type: 'group' });
    const child = n('c', { parentId: 'g' });
    const nodes = [group, child];
    const r = computeDuplicateNodes(nodes, [group], 7);
    const dupChild = r.nodes.find((x) => x.id === 'c-dup71');
    const dupGroup = r.nodes.find((x) => x.id === 'g-dup70');
    expect(dupGroup).toBeDefined();
    expect(dupChild?.parentId).toBe('g-dup70');
    // group precedes its child in the array
    expect(r.nodes.indexOf(dupGroup!)).toBeLessThan(r.nodes.indexOf(dupChild!));
  });
});

describe('makeWorkflowCliName', () => {
  it('picks the first free prefix-N', () => {
    const nodes = [
      n('a', { type: 'inputNode', data: { params: { cliName: 'input-1' } } }),
      n('b', { type: 'inputNode', data: { params: { cliName: 'input-3' } } }),
    ];
    expect(makeWorkflowCliName('inputNode', nodes)).toBe('input-2');
  });

  it('starts at 1 when none are used', () => {
    expect(makeWorkflowCliName('imageOutputNode', [])).toBe('output-image-1');
  });

  it('ignores blank/whitespace cliNames', () => {
    const nodes = [n('a', { type: 'inputNode', data: { params: { cliName: '  ' } } })];
    expect(makeWorkflowCliName('inputNode', nodes)).toBe('input-1');
  });
});
