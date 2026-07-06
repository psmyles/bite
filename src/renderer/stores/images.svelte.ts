import type { ImageInfo } from '../../shared/types.js';
import { IPC, EMPTY_GRAPH } from '../../shared/constants.js';
import { graphStore } from './graph.svelte.js';

class ImageStore {
  // _nodes is a plain (non-reactive) Map mutated in-place.
  // Reactivity is driven by _tick: every mutation increments _tick, which is a
  // $state signal. All public getters read _tick first to establish the reactive
  // dependency, so consumers re-evaluate whenever any mutation occurs.
  // This avoids creating a new Map + spreading arrays for every streaming result
  // (which was O(N²) work for an import of N images).
  private _nodes = new Map<string, ImageInfo[]>();
  // Images of deleted input nodes are retained here (not discarded) so that undoing
  // the deletion can restore them — the undo history only snapshots nodes/edges.
  private _detached = new Map<string, ImageInfo[]>();
  private _tick = $state(0);

  activeInputNodeId = $state<string | null>(null);
  selectedIndex = $state<number>(-1);
  importProgress = $state<{ done: number; total: number } | null>(null);
  importDone = $state(false);
  lastImportMs = $state<number | null>(null);
  lastImportCount = $state<number>(0);

  private _importCancelled = false;
  private _importStartTime = 0;
  private _importToken = 0;

  /** Images for the currently active input node (shown in filmstrip). */
  get images(): ImageInfo[] {
    void this._tick; // reactive dependency
    return this.activeInputNodeId ? (this._nodes.get(this.activeInputNodeId) ?? []) : [];
  }

  getImages(nodeId: string): ImageInfo[] {
    void this._tick; // reactive dependency
    return this._nodes.get(nodeId) ?? [];
  }

  setActive(nodeId: string): void {
    this.activeInputNodeId = nodeId;
    this.selectedIndex = -1;
  }

  cancelImport(): void {
    this._importCancelled = true;
    window.ipcRenderer.invoke(IPC.LOAD_IMAGES_STREAMING_CANCEL).catch(() => {});
  }

  dismissImport(): void {
    this.importDone = false;
  }

  get selected(): ImageInfo | null {
    return this.selectedIndex >= 0 ? (this.images[this.selectedIndex] ?? null) : null;
  }

  async add(paths: string[], nodeId?: string): Promise<void> {
    const targetNodeId = nodeId ?? this.activeInputNodeId;
    if (!targetNodeId) return;

    const existing = new Set((this._nodes.get(targetNodeId) ?? []).map((img) => img.path));
    paths = paths.filter((p) => !existing.has(p));
    if (paths.length === 0) return;

    const autoSelect = this.activeInputNodeId === targetNodeId && this.selectedIndex === -1;
    // Unique per-import token: results are broadcast on a shared IPC channel, so a
    // concurrent import (e.g. filmstrip drop during a folder scan) would otherwise
    // fire every import's listener and duplicate/misplace images. Each result carries
    // the token of the import that produced it; ignore results for other imports.
    const myToken = ++this._importToken;
    this._importCancelled = false;
    this._importStartTime = performance.now();
    this.importDone = false;
    this.importProgress = { done: 0, total: paths.length };
    const allAdded: ImageInfo[] = [];
    let doneCount = 0;

    let listenerActive = true;
    const onResult = (_e: unknown, info: ImageInfo, token?: number) => {
      if (!listenerActive || token !== myToken) return;
      // Don't recreate an image list for a node deleted mid-import.
      if (!graphStore.nodes.some((n) => n.id === targetNodeId)) return;
      doneCount++;
      // Mutate in-place — no new Map, no array spread. O(1) per image.
      let nodeImages = this._nodes.get(targetNodeId);
      if (!nodeImages) {
        nodeImages = [];
        this._nodes.set(targetNodeId, nodeImages);
      }
      const idx = nodeImages.length;
      nodeImages.push(info);
      if (autoSelect && this.selectedIndex === -1) this.selectedIndex = idx;
      allAdded.push(info);
      this._tick++;
      this.importProgress = { done: doneCount, total: paths.length };
    };
    window.ipcRenderer.on(IPC.LOAD_IMAGES_STREAMING_RESULT, onResult);

    try {
      const node = graphStore.nodes.find((n) => n.id === targetNodeId);
      const inputParams = (node?.data as Record<string, unknown>)?.params as Record<string, unknown> | undefined;
      const thumbSize = Number(inputParams?.thumbnailSize ?? 256);
      const allResults: ImageInfo[] = await window.ipcRenderer.invoke(
        IPC.LOAD_IMAGES_STREAMING_START,
        paths,
        thumbSize,
        myToken
      );
      if (Array.isArray(allResults)) {
        const addedPaths = new Set(allAdded.map((img) => img.path));
        const extra: ImageInfo[] = [];
        for (const info of allResults) {
          if (!addedPaths.has(info.path)) {
            extra.push(info);
            allAdded.push(info);
          }
        }
        if (extra.length > 0 && graphStore.nodes.some((n) => n.id === targetNodeId)) {
          let nodeImages = this._nodes.get(targetNodeId);
          if (!nodeImages) {
            nodeImages = [];
            this._nodes.set(targetNodeId, nodeImages);
          }
          const startIdx = nodeImages.length;
          nodeImages.push(...extra);
          if (autoSelect && this.selectedIndex === -1) this.selectedIndex = startIdx;
          this._tick++;
        }
      }
    } catch (err) {
      console.error('[imageStore] Streaming import failed:', err);
    } finally {
      this.lastImportMs = performance.now() - this._importStartTime;
      this.lastImportCount = allAdded.length;
      this.importProgress = null;
      if (!this._importCancelled) this.importDone = true;
      listenerActive = false;
      window.ipcRenderer.off(IPC.LOAD_IMAGES_STREAMING_RESULT, onResult);
    }

    // Pre-warm the preview cache — run in parallel with a concurrency window.
    (async () => {
      const CONCURRENCY = 6;
      for (let i = 0; i < allAdded.length; i += CONCURRENCY) {
        await Promise.all(
          allAdded.slice(i, i + CONCURRENCY).map((info) =>
            window.ipcRenderer.invoke(IPC.EXECUTE_PREVIEW, EMPTY_GRAPH, info.path).catch((err: unknown) => {
              console.warn('[imageStore] Preview warm-up failed:', err);
            })
          )
        );
      }
    })();
  }

  async openDialog(nodeId?: string): Promise<void> {
    const paths: string[] = await window.ipcRenderer.invoke(IPC.OPEN_IMAGES_DIALOG);
    await this.add(paths, nodeId);
  }

  async openFolderDialog(opts: { recursive: boolean; extensions: string[] }, nodeId?: string): Promise<void> {
    const paths: string[] = await window.ipcRenderer.invoke(IPC.SCAN_FOLDER_DIALOG, opts);
    await this.add(paths, nodeId);
  }

  select(index: number): void {
    this.selectedIndex = index;
  }

  remove(index: number, nodeId?: string): void {
    const targetNodeId = nodeId ?? this.activeInputNodeId;
    if (!targetNodeId) return;
    const imgs = this._nodes.get(targetNodeId) ?? [];
    this._nodes.set(
      targetNodeId,
      imgs.filter((_, i) => i !== index)
    );
    this._tick++;
    if (this.activeInputNodeId === targetNodeId) {
      const newLen = (this._nodes.get(targetNodeId) ?? []).length;
      if (this.selectedIndex >= newLen) this.selectedIndex = newLen - 1;
    }
  }

  clear(nodeId?: string): void {
    const targetNodeId = nodeId ?? this.activeInputNodeId;
    if (!targetNodeId) return;
    this._nodes.set(targetNodeId, []);
    this._tick++;
    if (this.activeInputNodeId === targetNodeId) this.selectedIndex = -1;
  }

  removeNode(nodeId: string): void {
    const imgs = this._nodes.get(nodeId);
    if (imgs && imgs.length > 0) this._detached.set(nodeId, imgs); // retain for undo
    this._nodes.delete(nodeId);
    this._tick++;
    if (this.activeInputNodeId === nodeId) {
      this.activeInputNodeId = null;
      this.selectedIndex = -1;
    }
  }

  /**
   * Reconcile image storage with the input nodes present after an undo/redo.
   * Restores detached images for nodes that reappeared, and detaches images for
   * nodes that were removed by the snapshot (so a subsequent redo/undo is symmetric).
   */
  reconcileNodes(presentNodeIds: string[]): void {
    const present = new Set(presentNodeIds);
    let changed = false;
    for (const id of [...this._nodes.keys()]) {
      if (!present.has(id)) {
        const imgs = this._nodes.get(id);
        if (imgs && imgs.length > 0) this._detached.set(id, imgs);
        this._nodes.delete(id);
        if (this.activeInputNodeId === id) {
          this.activeInputNodeId = null;
          this.selectedIndex = -1;
        }
        changed = true;
      }
    }
    for (const id of present) {
      if (!this._nodes.has(id) && this._detached.has(id)) {
        this._nodes.set(id, this._detached.get(id)!);
        this._detached.delete(id);
        changed = true;
      }
    }
    if (changed) this._tick++;
  }
}

export const imageStore = new ImageStore();
