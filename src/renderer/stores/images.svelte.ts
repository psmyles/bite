import type { ImageInfo } from '../../shared/types.js'
import { IPC, THUMBNAIL_SIZE_PX, EMPTY_GRAPH } from '../../shared/constants.js'

class ImageStore {
  images          = $state<ImageInfo[]>([])
  selectedIndex   = $state<number>(-1)
  importProgress  = $state<{ done: number; total: number } | null>(null)
  importDone      = $state(false)
  lastImportMs    = $state<number | null>(null)
  lastImportCount = $state<number>(0)

  private _importCancelled = false
  private _importStartTime = 0

  cancelImport(): void {
    this._importCancelled = true
    window.ipcRenderer.invoke(IPC.LOAD_IMAGES_STREAMING_CANCEL).catch(() => {})
  }

  dismissImport(): void {
    this.importDone = false
  }

  get selected(): ImageInfo | null {
    return this.selectedIndex >= 0 ? (this.images[this.selectedIndex] ?? null) : null
  }

  async add(paths: string[]): Promise<void> {
    const existing = new Set(this.images.map(img => img.path))
    paths = paths.filter(p => !existing.has(p))
    if (paths.length === 0) return

    const autoSelect = this.selectedIndex === -1
    this._importCancelled = false
    this._importStartTime = performance.now()
    this.importDone    = false
    this.importProgress = { done: 0, total: paths.length }
    const allAdded: ImageInfo[] = []
    let doneCount = 0

    // Guard against late-arriving IPC messages after the stream is done.
    // (The preload wraps listeners, so off() may not remove all in-flight messages.)
    let listenerActive = true
    const onResult = (_e: unknown, info: ImageInfo) => {
      if (!listenerActive) return
      doneCount++
      const idx = this.images.length
      this.images.push(info)
      if (autoSelect && this.selectedIndex === -1) this.selectedIndex = idx
      allAdded.push(info)
      this.importProgress = { done: doneCount, total: paths.length }
    }
    window.ipcRenderer.on(IPC.LOAD_IMAGES_STREAMING_RESULT, onResult)

    try {
      await window.ipcRenderer.invoke(IPC.LOAD_IMAGES_STREAMING_START, paths, THUMBNAIL_SIZE_PX)
    } catch (err) {
      console.error('[imageStore] Streaming import failed:', err)
    } finally {
      this.lastImportMs    = performance.now() - this._importStartTime
      this.lastImportCount = allAdded.length
      this.importProgress  = null
      if (!this._importCancelled) this.importDone = true
      listenerActive = false
      window.ipcRenderer.off(IPC.LOAD_IMAGES_STREAMING_RESULT, onResult)
    }

    // Pre-warm the preview cache sequentially in the background.
    // One at a time avoids flooding the system with magick processes.
    ;(async () => {
      for (const info of allAdded) {
        await window.ipcRenderer
          .invoke(IPC.EXECUTE_PREVIEW, EMPTY_GRAPH, info.path)
          .catch((err: unknown) => { console.warn('[imageStore] Preview warm-up failed:', err) })
      }
    })()
  }

  async openDialog(): Promise<void> {
    const paths: string[] = await window.ipcRenderer.invoke(IPC.OPEN_IMAGES_DIALOG)
    await this.add(paths)
  }

  async openFolderDialog(opts: { recursive: boolean; extensions: string[] }): Promise<void> {
    const paths: string[] = await window.ipcRenderer.invoke(IPC.SCAN_FOLDER_DIALOG, opts)
    await this.add(paths)
  }

  select(index: number): void {
    this.selectedIndex = index
  }

  remove(index: number): void {
    this.images = this.images.filter((_, i) => i !== index)
    if (this.selectedIndex >= this.images.length) {
      this.selectedIndex = this.images.length - 1
    }
  }

  clear(): void {
    this.images = []
    this.selectedIndex = -1
  }
}

export const imageStore = new ImageStore()
