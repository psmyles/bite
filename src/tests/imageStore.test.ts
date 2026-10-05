import { describe, it, expect, beforeEach, vi } from 'vitest';

// Stub window.ipcRenderer before importing the store (the store uses it in add/openDialog)
vi.stubGlobal('window', {
  ipcRenderer: {
    invoke: vi.fn().mockResolvedValue([]),
    on: vi.fn(),
    off: vi.fn(),
  },
});

import { imageStore } from '../renderer/stores/images.svelte.js';
import type { ImageInfo } from '../shared/types.js';

function makeImage(path: string): ImageInfo {
  return { path, name: path.split('/').pop()!, width: 100, height: 100, format: 'PNG', sizeBytes: 1024 };
}

// Direct access to the private backing Map for seeding test data without IPC
function seed(nodeId: string, images: ImageInfo[]) {
  (imageStore as unknown as { _nodes: Map<string, ImageInfo[]> })._nodes.set(nodeId, [...images]);
  // Increment _tick so derived reads see the change
  (imageStore as unknown as { _tick: number })._tick++;
}

beforeEach(() => {
  // Clear all nodes and reset state
  (imageStore as unknown as { _nodes: Map<string, ImageInfo[]> })._nodes.clear();
  (imageStore as unknown as { _tick: number })._tick++;
  imageStore.activeInputNodeId = null;
  imageStore.selectedIndex = -1;
});

describe('getImages', () => {
  it('returns [] for an unknown nodeId', () => {
    expect(imageStore.getImages('no-such-node')).toEqual([]);
  });

  it('returns the images for a seeded node', () => {
    seed('node-1', [makeImage('/a.png'), makeImage('/b.png')]);
    expect(imageStore.getImages('node-1')).toHaveLength(2);
  });
});

describe('setActive', () => {
  it('updates activeInputNodeId', () => {
    imageStore.setActive('node-abc');
    expect(imageStore.activeInputNodeId).toBe('node-abc');
  });

  it('resets selectedIndex to -1', () => {
    imageStore.selectedIndex = 3;
    imageStore.setActive('node-abc');
    expect(imageStore.selectedIndex).toBe(-1);
  });
});

describe('removeNode', () => {
  it('makes getImages return [] for the removed node', () => {
    seed('node-1', [makeImage('/a.png')]);
    imageStore.removeNode('node-1');
    expect(imageStore.getImages('node-1')).toEqual([]);
  });

  it('clears activeInputNodeId when removing the active node', () => {
    seed('node-1', [makeImage('/a.png')]);
    imageStore.setActive('node-1');
    imageStore.removeNode('node-1');
    expect(imageStore.activeInputNodeId).toBeNull();
  });

  it('does not affect other nodes', () => {
    seed('node-1', [makeImage('/a.png')]);
    seed('node-2', [makeImage('/b.png')]);
    imageStore.removeNode('node-1');
    expect(imageStore.getImages('node-2')).toHaveLength(1);
  });

  it('is a no-op for non-existent nodeId', () => {
    seed('node-1', [makeImage('/a.png')]);
    imageStore.removeNode('nonexistent');
    expect(imageStore.getImages('node-1')).toHaveLength(1);
  });
});

describe('clear', () => {
  it('clears images for the specified node', () => {
    seed('node-1', [makeImage('/a.png'), makeImage('/b.png')]);
    imageStore.clear('node-1');
    expect(imageStore.getImages('node-1')).toEqual([]);
  });

  it('does not affect other nodes', () => {
    seed('node-1', [makeImage('/a.png')]);
    seed('node-2', [makeImage('/b.png'), makeImage('/c.png')]);
    imageStore.clear('node-1');
    expect(imageStore.getImages('node-2')).toHaveLength(2);
  });

  it('resets selectedIndex when clearing the active node', () => {
    seed('node-1', [makeImage('/a.png')]);
    imageStore.setActive('node-1');
    imageStore.selectedIndex = 0;
    imageStore.clear('node-1');
    expect(imageStore.selectedIndex).toBe(-1);
  });
});
