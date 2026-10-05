import type { Node, Edge } from '@xyflow/svelte';

export interface Snapshot {
  nodes: Node[];
  edges: Edge[];
}

const MAX_HISTORY = 100;

export class UndoRedoManager {
  private history: Snapshot[] = [];
  private index = -1;
  private _pendingPush = false;

  push(nodes: Node[], edges: Edge[]): void {
    this.history.splice(this.index + 1); // discard any redo future
    this.history.push({
      nodes: JSON.parse(JSON.stringify(nodes)) as Node[],
      edges: JSON.parse(JSON.stringify(edges)) as Edge[],
    });
    if (this.history.length > MAX_HISTORY) this.history.shift();
    this.index = this.history.length - 1;
  }

  // Deferred variant: batches rapid back-to-back calls (e.g. node+edge deleted
  // in the same tick) into a single history entry.
  schedulePush(getSnapshot: () => { nodes: Node[]; edges: Edge[] }): void {
    if (this._pendingPush) return;
    this._pendingPush = true;
    Promise.resolve().then(() => {
      this._pendingPush = false;
      const { nodes, edges } = getSnapshot();
      this.push(nodes, edges);
    });
  }

  undo(): Snapshot | null {
    if (this.index <= 0) return null;
    this.index--;
    return this.history[this.index];
  }

  redo(): Snapshot | null {
    if (this.index >= this.history.length - 1) return null;
    this.index++;
    return this.history[this.index];
  }
}
