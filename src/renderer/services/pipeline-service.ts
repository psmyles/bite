// Electron implementation of PipelineService — calls main process via IPC
import type { ImageInfo, NodeGraph, Progress, PipelineService } from '../../shared/types.js'
import { IPC } from '../../shared/constants.js'

export class ElectronPipelineService implements PipelineService {
  async loadImages(paths: string[]): Promise<ImageInfo[]> {
    return window.ipcRenderer.invoke(IPC.LOAD_IMAGES, paths)
  }

  async generateThumbnail(imagePath: string, size: number): Promise<string> {
    return window.ipcRenderer.invoke(IPC.GENERATE_THUMBNAIL, imagePath, size)
  }

  async executePreview(graph: NodeGraph, imagePath: string, fromNodeId?: string): Promise<string> {
    return window.ipcRenderer.invoke(IPC.EXECUTE_PREVIEW, graph, imagePath, fromNodeId)
  }

  async executeBatch(
    graph: NodeGraph,
    imagePaths: string[],
    outputDir: string,
    onProgressCb: (p: Progress) => void
  ): Promise<void> {
    // Progress updates come via a separate IPC event channel
    const onProgress = (_e: unknown, p: Progress) => onProgressCb(p)
    window.ipcRenderer.on(`${IPC.EXECUTE_BATCH}:progress`, onProgress)
    try {
      await window.ipcRenderer.invoke(IPC.EXECUTE_BATCH, graph, imagePaths, outputDir)
    } finally {
      window.ipcRenderer.off(`${IPC.EXECUTE_BATCH}:progress`, onProgress)
    }
  }

  exportCLI(graph: NodeGraph): string {
    // Synchronous — run via invoke in practice; stub returns empty for now
    void graph
    return ''
  }
}
