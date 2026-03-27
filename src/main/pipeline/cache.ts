// Phase 4: node-level output caching for the preview pipeline

export class PreviewCache {
  private cache = new Map<string, string>()

  get(nodeId: string, inputHash: string): string | undefined {
    return this.cache.get(`${nodeId}:${inputHash}`)
  }

  set(nodeId: string, inputHash: string, outputPath: string): void {
    this.cache.set(`${nodeId}:${inputHash}`, outputPath)
  }

  invalidateFrom(nodeId: string): void {
    for (const key of this.cache.keys()) {
      // Match standard keys "nodeId:hash" and multi-output split keys "nodeId__N:hash"
      if (key.startsWith(nodeId + ':') || key.startsWith(nodeId + '__')) this.cache.delete(key)
    }
  }

  clear(): void {
    this.cache.clear()
  }
}
