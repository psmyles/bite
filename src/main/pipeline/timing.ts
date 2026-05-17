import fs from 'node:fs';
import path from 'node:path';

export interface ImageBatchTiming {
  file: string;
  totalMs: number;
  fileCheckMs: number;
  magickMs: number;
  copyMs: number;
}

export interface BatchRun {
  startedAt: string;
  imageCount: number;
  totalMs: number;
  setupMs: number;
  startupMs: number;
  images: ImageBatchTiming[];
  avgMs: number;
  minMs: number;
  maxMs: number;
  p95Ms: number;
}

export interface ImageImportTiming {
  file: string;
  headerParseMs: number;
  identifyMs: number;
  thumbnailMs: number;
  pathType: 'fast' | 'slow' | 'cached';
}

export interface ImportRun {
  startedAt: string;
  imageCount: number;
  totalMs: number;
  images: ImageImportTiming[];
}

export class TimingCollector {
  enabled = false;

  batchHistory: BatchRun[] = [];
  importHistory: ImportRun[] = [];

  private _currentBatch: {
    startedAt: string;
    imageCount: number;
    t0: number;
    setupMs: number;
    startupMs: number;
    images: ImageBatchTiming[];
  } | null = null;

  private _currentImport: {
    startedAt: string;
    imageCount: number;
    images: ImageImportTiming[];
  } | null = null;

  startBatch(imageCount: number): void {
    this._currentBatch = {
      startedAt: new Date().toISOString(),
      imageCount,
      t0: Date.now(),
      setupMs: 0,
      startupMs: 0,
      images: [],
    };
  }

  recordSetup(ms: number): void {
    if (this._currentBatch) this._currentBatch.setupMs = ms;
  }

  recordStartup(ms: number): void {
    if (this._currentBatch) this._currentBatch.startupMs = ms;
  }

  beginImage(file: string): {
    magick(ms: number): void;
    fileCheck(ms: number): void;
    copy(ms: number): void;
    done(totalMs: number): void;
  } {
    const entry: ImageBatchTiming = { file, totalMs: 0, fileCheckMs: 0, magickMs: 0, copyMs: 0 };
    return {
      magick: (ms) => {
        entry.magickMs += ms;
      },
      fileCheck: (ms) => {
        entry.fileCheckMs = ms;
      },
      copy: (ms) => {
        entry.copyMs = ms;
      },
      done: (ms) => {
        entry.totalMs = ms;
        this._currentBatch?.images.push(entry);
      },
    };
  }

  endBatch(outputDir: string | null, verboseEntries?: string[]): void {
    const b = this._currentBatch;
    if (!b) return;
    this._currentBatch = null;

    const totalMs = Date.now() - b.t0;
    const imgs = b.images;

    let avgMs = 0,
      minMs = 0,
      maxMs = 0,
      p95Ms = 0;
    if (imgs.length > 0) {
      const sorted = [...imgs.map((i) => i.totalMs)].sort((a, z) => a - z);
      avgMs = Math.round(sorted.reduce((s, v) => s + v, 0) / sorted.length);
      minMs = sorted[0];
      maxMs = sorted[sorted.length - 1];
      p95Ms = sorted[Math.min(Math.ceil(sorted.length * 0.95), sorted.length) - 1];
    }

    const avgMagick = imgs.length ? Math.round(imgs.reduce((s, i) => s + i.magickMs, 0) / imgs.length) : 0;
    const avgFileCheck = imgs.length ? Math.round(imgs.reduce((s, i) => s + i.fileCheckMs, 0) / imgs.length) : 0;
    const avgCopy = imgs.length ? Math.round(imgs.reduce((s, i) => s + i.copyMs, 0) / imgs.length) : 0;

    const slowest = imgs.length ? imgs.reduce((a, b) => (b.totalMs > a.totalMs ? b : a)) : null;

    const run: BatchRun = {
      startedAt: b.startedAt,
      imageCount: b.imageCount,
      totalMs,
      setupMs: b.setupMs,
      startupMs: b.startupMs,
      images: imgs,
      avgMs,
      minMs,
      maxMs,
      p95Ms,
    };

    this.batchHistory.push(run);
    if (this.batchHistory.length > 10) this.batchHistory.shift();

    const block = [
      `[timings] Batch run — ${b.startedAt} — ${b.imageCount} images`,
      `  Total:    ${totalMs}ms`,
      `  Setup:    ${b.setupMs}ms  |  Startup (IPC -> first image): ${b.startupMs}ms`,
      imgs.length > 0
        ? `  Per-image avg: ${avgMs}ms  min: ${minMs}ms  max: ${maxMs}ms  p95: ${p95Ms}ms`
        : `  Per-image: no images recorded`,
      `  Breakdown avg: magick ${avgMagick}ms  fileCheck ${avgFileCheck}ms  copy ${avgCopy}ms`,
      slowest ? `  Slowest: ${path.basename(slowest.file)} (${slowest.totalMs}ms)` : '',
    ]
      .filter(Boolean)
      .join('\n');

    console.log(block);

    if (outputDir) {
      const extra = verboseEntries && verboseEntries.length > 0 ? verboseEntries.join('\n') + '\n' : '';
      fs.promises.appendFile(path.join(outputDir, 'perf.log'), block + '\n\n' + extra).catch(() => {});
    }
  }

  startImport(imageCount: number): void {
    this._currentImport = {
      startedAt: new Date().toISOString(),
      imageCount,
      images: [],
    };
  }

  recordImportImage(timing: ImageImportTiming): void {
    this._currentImport?.images.push(timing);
  }

  endImport(totalMs: number): void {
    const b = this._currentImport;
    if (!b) return;
    this._currentImport = null;

    const imgs = b.images;
    const fastImgs = imgs.filter((i) => i.pathType === 'fast');
    const slowImgs = imgs.filter((i) => i.pathType === 'slow');
    const cachedImgs = imgs.filter((i) => i.pathType === 'cached');

    const avgHeader = imgs.length ? Math.round(imgs.reduce((s, i) => s + i.headerParseMs, 0) / imgs.length) : 0;
    const avgIdentify = slowImgs.length
      ? Math.round(slowImgs.reduce((s, i) => s + i.identifyMs, 0) / slowImgs.length)
      : 0;
    const thumbImgs = [...fastImgs, ...slowImgs];
    const avgThumb = thumbImgs.length
      ? Math.round(thumbImgs.reduce((s, i) => s + i.thumbnailMs, 0) / thumbImgs.length)
      : 0;
    const avgPerImage = imgs.length ? Math.round(totalMs / imgs.length) : 0;

    const run: ImportRun = { startedAt: b.startedAt, imageCount: b.imageCount, totalMs, images: imgs };
    this.importHistory.push(run);
    if (this.importHistory.length > 10) this.importHistory.shift();

    const block = [
      `[timings] Import run — ${b.startedAt} — ${b.imageCount} images`,
      `  Total:    ${totalMs}ms`,
      `  Per-image avg: ${avgPerImage}ms  (header: ${avgHeader}ms  identify: ${avgIdentify}ms  thumb: ${avgThumb}ms)`,
      `  Fast-path: ${fastImgs.length}  Slow-path: ${slowImgs.length}  Cached: ${cachedImgs.length}`,
    ].join('\n');

    console.log(block);
  }
}

export const timings = new TimingCollector();
