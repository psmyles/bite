import type { NodeGraph, Progress } from '../../shared/types.js'
import type { NodeRegistry } from '../nodes/registry.js'
import './imageNodeExecutors.js'
import { PreviewCache } from './cache.js'
import { executePreview as executePreviewFn } from './preview-pipeline.js'
import { executeBatch as executeBatchFn } from './batch-pipeline.js'
import { cliScriptPS, cliScriptBash, cliScriptCmd } from './executor-cli.js'
import {
  loadImage as loadImageFn,
  loadImageWithThumbnail as loadImageWithThumbnailFn,
  loadImageWithThumbnailBatch as loadImageWithThumbnailBatchFn,
  generateThumbnail as generateThumbnailFn,
} from './thumbnail-service.js'

// ─── PipelineExecutor ─────────────────────────────────────────────────────────

export class PipelineExecutor {
  private previewCache    = new PreviewCache()
  private _batchCancelled = false

  cancelBatch(): void { this._batchCancelled = true }

  loadImage(imagePath: string)                                { return loadImageFn(imagePath) }
  loadImageWithThumbnail(imagePath: string, size: number)     { return loadImageWithThumbnailFn(imagePath, size) }
  loadImageWithThumbnailBatch(imagePaths: string[], size: number) { return loadImageWithThumbnailBatchFn(imagePaths, size) }
  generateThumbnail(imagePath: string, size: number)          { return generateThumbnailFn(imagePath, size) }

  // ── Preview pipeline ────────────────────────────────────────────────────────

  executePreview(graph: NodeGraph, imagePath: string, registry: NodeRegistry, fromNodeId?: string) {
    return executePreviewFn(this.previewCache, graph, imagePath, registry, fromNodeId)
  }

  // ── Batch pipeline ────────────────────────────────────────────────────────

  executeBatch(graph: NodeGraph, imagePaths: string[], outputDir: string | null, overwrite: 'skip' | 'overwrite', registry: NodeRegistry, onProgress: (p: Progress) => void) {
    this._batchCancelled = false
    return executeBatchFn(graph, imagePaths, outputDir, overwrite, registry, onProgress, () => this._batchCancelled)
  }


  // ── CLI script export ────────────────────────────────────────────────────────

  exportCLI(
    shellType: 'powershell' | 'bash' | 'cmd',
    workflowFileName: string,
  ): string {
    const date = new Date().toISOString().slice(0, 10)
    if (shellType === 'powershell') return cliScriptPS(workflowFileName, date)
    if (shellType === 'bash')       return cliScriptBash(workflowFileName, date)
    return cliScriptCmd(workflowFileName, date)
  }

  // ── Cache control ───────────────────────────────────────────────────────────

  clearPreviewCache(): void {
    this.previewCache.clear()
  }
}
