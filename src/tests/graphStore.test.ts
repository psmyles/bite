import { describe, it, expect, beforeEach } from 'vitest';
import { graphStore } from '../renderer/stores/graph.svelte.js';

beforeEach(() => {
  graphStore.resetToSeed();
});

describe('canDeleteNode', () => {
  it('returns false for the only inputNode', () => {
    const inputNode = graphStore.nodes.find((n) => n.type === 'inputNode')!;
    expect(graphStore.canDeleteNode(inputNode.id)).toBe(false);
  });

  it('returns false for the only imageOutputNode', () => {
    const outNode = graphStore.nodes.find((n) => n.type === 'imageOutputNode')!;
    expect(graphStore.canDeleteNode(outNode.id)).toBe(false);
  });

  it('returns true for a second inputNode', () => {
    const firstInput = graphStore.nodes.find((n) => n.type === 'inputNode')!;
    graphStore.nodes = [...graphStore.nodes, { ...firstInput, id: 'input-extra' }];
    expect(graphStore.canDeleteNode(firstInput.id)).toBe(true);
  });

  it('returns true for a second imageOutputNode', () => {
    const firstOut = graphStore.nodes.find((n) => n.type === 'imageOutputNode')!;
    graphStore.nodes = [...graphStore.nodes, { ...firstOut, id: 'out-extra' }];
    expect(graphStore.canDeleteNode(firstOut.id)).toBe(true);
  });

  it('returns true for a non-existent nodeId', () => {
    expect(graphStore.canDeleteNode('does-not-exist')).toBe(true);
  });

  it('returns true for a regular process node', () => {
    graphStore.nodes = [
      ...graphStore.nodes,
      {
        id: 'proc-1',
        type: 'default',
        position: { x: 0, y: 0 },
        data: { label: 'proc', definitionId: 'resize', params: {} },
      },
    ];
    expect(graphStore.canDeleteNode('proc-1')).toBe(true);
  });
});

describe('setParam', () => {
  it('updates the param on the target node', () => {
    const inputNode = graphStore.nodes.find((n) => n.type === 'inputNode')!;
    graphStore.setParam(inputNode.id, 'thumbnailSize', 512);
    const updated = graphStore.nodes.find((n) => n.id === inputNode.id)!;
    expect((updated.data as Record<string, unknown>).params).toMatchObject({ thumbnailSize: 512 });
  });

  it('is a no-op for a non-existent nodeId', () => {
    const nodesBefore = graphStore.nodes.length;
    graphStore.setParam('nonexistent-id', 'foo', 'bar');
    expect(graphStore.nodes.length).toBe(nodesBefore);
  });

  it('does not mutate other nodes', () => {
    const [n1, n2] = graphStore.nodes;
    graphStore.setParam(n1.id, 'cliName', 'new-name');
    expect(graphStore.nodes.find((n) => n.id === n2.id)).toBe(n2);
  });
});

describe('isDirty', () => {
  it('is false immediately after resetToSeed', () => {
    expect(graphStore.isDirty).toBe(false);
  });

  it('becomes true after setParam', () => {
    const inputNode = graphStore.nodes.find((n) => n.type === 'inputNode')!;
    graphStore.setParam(inputNode.id, 'cliName', 'changed');
    expect(graphStore.isDirty).toBe(true);
  });

  it('becomes false after markClean', () => {
    const inputNode = graphStore.nodes.find((n) => n.type === 'inputNode')!;
    graphStore.setParam(inputNode.id, 'cliName', 'changed');
    expect(graphStore.isDirty).toBe(true);
    graphStore.markClean(null);
    expect(graphStore.isDirty).toBe(false);
  });
});
