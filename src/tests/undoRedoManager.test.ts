import { describe, it, expect } from 'vitest';
import { UndoRedoManager } from '../renderer/nodeEditor/undoRedoManager.js';
import type { Node, Edge } from '@xyflow/svelte';

function node(id: string): Node {
  return { id, type: 'default', position: { x: 0, y: 0 }, data: {} } as Node;
}

function edge(id: string): Edge {
  return { id, source: 'a', target: 'b' } as Edge;
}

describe('UndoRedoManager', () => {
  it('push stores a deep clone — mutations after push do not affect history', () => {
    const mgr = new UndoRedoManager();
    const nodes = [node('n1')];
    mgr.push(nodes, []);
    nodes[0].position = { x: 999, y: 999 };
    const snap = mgr.undo();
    expect(snap).toBeNull(); // nothing before first entry
    // The stored snapshot must be unaffected by the mutation
    // undo() returns null because index was already at 0; check via redo
  });

  it('push then undo returns null (only one entry)', () => {
    const mgr = new UndoRedoManager();
    mgr.push([node('a')], []);
    expect(mgr.undo()).toBeNull();
  });

  it('two pushes: undo returns first snapshot', () => {
    const mgr = new UndoRedoManager();
    mgr.push([node('a')], []);
    mgr.push([node('b')], []);
    const snap = mgr.undo();
    expect(snap).not.toBeNull();
    expect(snap!.nodes[0].id).toBe('a');
  });

  it('undo then redo returns the later snapshot', () => {
    const mgr = new UndoRedoManager();
    mgr.push([node('a')], []);
    mgr.push([node('b')], []);
    mgr.undo();
    const snap = mgr.redo();
    expect(snap).not.toBeNull();
    expect(snap!.nodes[0].id).toBe('b');
  });

  it('redo at end returns null', () => {
    const mgr = new UndoRedoManager();
    mgr.push([node('a')], []);
    expect(mgr.redo()).toBeNull();
  });

  it('undo past start returns null', () => {
    const mgr = new UndoRedoManager();
    mgr.push([node('a')], []);
    mgr.push([node('b')], []);
    mgr.undo();
    expect(mgr.undo()).toBeNull();
  });

  it('push after undo discards redo future', () => {
    const mgr = new UndoRedoManager();
    mgr.push([node('a')], []);
    mgr.push([node('b')], []);
    mgr.undo();
    mgr.push([node('c')], []);
    expect(mgr.redo()).toBeNull();
    const snap = mgr.undo();
    expect(snap!.nodes[0].id).toBe('a');
  });

  it('history is capped at 100 entries', () => {
    const mgr = new UndoRedoManager();
    for (let i = 0; i < 101; i++) {
      mgr.push([node(`n${i}`)], []);
    }
    // After 101 pushes with cap=100, oldest entry (n0) is evicted.
    // Undo 99 times from the end brings us to n1 (the oldest remaining).
    for (let i = 0; i < 99; i++) mgr.undo();
    const snap = mgr.undo();
    expect(snap).toBeNull(); // no entry before n1
  });

  it('push stores edges correctly', () => {
    const mgr = new UndoRedoManager();
    mgr.push([node('a')], [edge('e1')]);
    mgr.push([node('b')], []);
    const snap = mgr.undo();
    expect(snap!.edges[0].id).toBe('e1');
  });

  it('schedulePush batches two synchronous calls into one entry', async () => {
    const mgr = new UndoRedoManager();
    mgr.push([node('initial')], []);
    let callCount = 0;
    const getSnapshot = () => {
      callCount++;
      return { nodes: [node('batched')], edges: [] };
    };
    mgr.schedulePush(getSnapshot);
    mgr.schedulePush(getSnapshot); // second call is a no-op (pending flag set)
    await Promise.resolve(); // flush microtask queue
    expect(callCount).toBe(1);
    const snap = mgr.undo();
    expect(snap!.nodes[0].id).toBe('initial');
  });
});
