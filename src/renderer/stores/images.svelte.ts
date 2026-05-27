import type { ImageInfo } from '../../shared/types.js';
import { IPC, EMPTY_GRAPH } from '../../shared/constants.js';
import { graphStore } from './graph.svelte.js';

class ImageStore {
  private _nodes = $state<Map<string, ImageInfo[]>>(new Map());
  activeInputNodeId = $state<string | null>(null);
  selectedIndex = $state<number>(-1);
  importProgress = $state<{ done: number; total: number } | null>(null);
  importDone = $state(false);
  lastImportMs = $state<number | null>(null);
  lastImportCount = $state<number>(0);

  private _importCancelled = false;
  private _importStartTime = 0;

  /** Images for the currently active input node (shown in filmstrip). */
  get images(): ImageInfo[] {
    return this.activeInputNodeId ? (this._nodes.get(this.activeInputNodeId) ?? []) : [];
  }

  getImages(nodeId: string): ImageInfo[] {
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

    const currentImages = this._nodes.get(targetNodeId) ?? [];
    const autoSelect = this.activeInputNodeId === targetNodeId && this.selectedIndex === -1;
    this._importCancelled = false;
    this._importStartTime = performance.now();
    this.importDone = false;
    this.importProgress = { done: 0, total: paths.length };
    const allAdded: ImageInfo[] = [];
    let doneCount = 0;

    let listenerActive = true;
    const onResult = (_e: unknown, info: ImageInfo) => {
      if (!listenerActive) return;
      doneCount++;
      const nodeImages = this._nodes.get(targetNodeId) ?? [];
      const idx = nodeImages.length;
      // Trigger reactivity by creating a new Map
      const newMap = new Map(this._nodes);
      newMap.set(targetNodeId, [...nodeImages, info]);
      this._nodes = newMap;
      if (autoSelect && this.selectedIndex === -1) this.selectedIndex = idx;
      allAdded.push(info);
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
        thumbSize
      );
      if (Array.isArray(allResults)) {
        const addedPaths = new Set(allAdded.map((img) => img.path));
        const nodeImages = this._nodes.get(targetNodeId) ?? [];
        const extra: ImageInfo[] = [];
        for (const info of allResults) {
          if (!addedPaths.has(info.path)) {
            const idx = nodeImages.length + extra.length;
            extra.push(info);
            if (autoSelect && this.selectedIndex === -1) this.selectedIndex = idx;
            allAdded.push(info);
          }
        }
        if (extra.length > 0) {
          const newMap = new Map(this._nodes);
          newMap.set(targetNodeId, [...nodeImages, ...extra]);
          this._nodes = newMap;
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

    // Pre-warm the preview cache.
    ;(async () => {
      for (const info of allAdded) {
        await window.ipcRenderer.invoke(IPC.EXECUTE_PREVIEW, EMPTY_GRAPH, info.path).catch((err: unknown) => {
          console.warn('[imageStore] Preview warm-up failed:', err);
        });
      }
    })();
    void currentImages; // suppress unused warning
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
    const newMap = new Map(this._nodes);
    newMap.set(targetNodeId, imgs.filter((_, i) => i !== index));
    this._nodes = newMap;
    if (this.activeInputNodeId === targetNodeId) {
      const newLen = (this._nodes.get(targetNodeId) ?? []).length;
      if (this.selectedIndex >= newLen) this.selectedIndex = newLen - 1;
    }
  }

  clear(nodeId?: string): void {
    const targetNodeId = nodeId ?? this.activeInputNodeId;
    if (!targetNodeId) return;
    const newMap = new Map(this._nodes);
    newMap.set(targetNodeId, []);
    this._nodes = newMap;
    if (this.activeInputNodeId === targetNodeId) this.selectedIndex = -1;
  }

  removeNode(nodeId: string): void {
    const newMap = new Map(this._nodes);
    newMap.delete(nodeId);
    this._nodes = newMap;
    if (this.activeInputNodeId === nodeId) {
      this.activeInputNodeId = null;
      this.selectedIndex = -1;
    }
  }
}

export const imageStore = new ImageStore();
