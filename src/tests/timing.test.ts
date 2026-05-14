import { describe, it, expect, beforeEach } from 'vitest';
import { TimingCollector } from '../main/pipeline/timing.js';

let t: TimingCollector;

beforeEach(() => {
  t = new TimingCollector();
});

// ── enabled flag ──────────────────────────────────────────────────────────────

describe('TimingCollector.enabled', () => {
  it('starts disabled', () => {
    expect(t.enabled).toBe(false);
  });

  it('can be toggled', () => {
    t.enabled = true;
    expect(t.enabled).toBe(true);
    t.enabled = false;
    expect(t.enabled).toBe(false);
  });
});

// ── batch lifecycle ───────────────────────────────────────────────────────────

describe('batch recording', () => {
  it('endBatch with no startBatch is a no-op', () => {
    t.endBatch(null);
    expect(t.batchHistory).toHaveLength(0);
  });

  it('records a batch run in history', () => {
    t.startBatch(5);
    t.endBatch(null);
    expect(t.batchHistory).toHaveLength(1);
    expect(t.batchHistory[0].imageCount).toBe(5);
  });

  it('records setup and startup times', () => {
    t.startBatch(3);
    t.recordSetup(12);
    t.recordStartup(18);
    t.endBatch(null);
    const run = t.batchHistory[0];
    expect(run.setupMs).toBe(12);
    expect(run.startupMs).toBe(18);
  });

  it('records per-image entries via beginImage', () => {
    t.startBatch(2);
    const e1 = t.beginImage('a.png');
    e1.fileCheck(3);
    e1.magick(100);
    e1.copy(2);
    e1.done(105);
    const e2 = t.beginImage('b.png');
    e2.fileCheck(4);
    e2.magick(200);
    e2.copy(3);
    e2.done(207);
    t.endBatch(null);
    const run = t.batchHistory[0];
    expect(run.images).toHaveLength(2);
    expect(run.images[0].file).toBe('a.png');
    expect(run.images[0].magickMs).toBe(100);
    expect(run.images[1].magickMs).toBe(200);
  });

  it('computes avg/min/max from per-image totals', () => {
    t.startBatch(3);
    for (const ms of [100, 200, 300]) {
      const e = t.beginImage(`img_${ms}.png`);
      e.fileCheck(0);
      e.magick(ms);
      e.copy(0);
      e.done(ms);
    }
    t.endBatch(null);
    const run = t.batchHistory[0];
    expect(run.minMs).toBe(100);
    expect(run.maxMs).toBe(300);
    expect(run.avgMs).toBe(200);
  });

  it('p95 is correct for small arrays', () => {
    t.startBatch(4);
    for (const ms of [10, 20, 30, 400]) {
      const e = t.beginImage('f.png');
      e.fileCheck(0);
      e.magick(ms);
      e.copy(0);
      e.done(ms);
    }
    t.endBatch(null);
    const run = t.batchHistory[0];
    // 4 items sorted: [10, 20, 30, 400]; ceil(4*0.95)=4, index 3 → 400
    expect(run.p95Ms).toBe(400);
  });

  it('stats are zero when no images recorded', () => {
    t.startBatch(0);
    t.endBatch(null);
    const run = t.batchHistory[0];
    expect(run.avgMs).toBe(0);
    expect(run.minMs).toBe(0);
    expect(run.maxMs).toBe(0);
    expect(run.p95Ms).toBe(0);
  });

  it('keeps at most 10 runs in history', () => {
    for (let i = 0; i < 12; i++) {
      t.startBatch(1);
      t.endBatch(null);
    }
    expect(t.batchHistory).toHaveLength(10);
  });

  it('startBatch resets state so a second call discards the previous run', () => {
    t.startBatch(5);
    t.startBatch(3); // restart before endBatch
    t.endBatch(null);
    expect(t.batchHistory).toHaveLength(1);
    expect(t.batchHistory[0].imageCount).toBe(3);
  });

  it('recordSetup and recordStartup are no-ops when no batch active', () => {
    t.recordSetup(100);
    t.recordStartup(100);
    // no crash, history unchanged
    expect(t.batchHistory).toHaveLength(0);
  });

  it('magick accumulates across multiple calls', () => {
    t.startBatch(1);
    const e = t.beginImage('a.png');
    e.magick(50);
    e.magick(75);
    e.fileCheck(0);
    e.copy(0);
    e.done(125);
    t.endBatch(null);
    expect(t.batchHistory[0].images[0].magickMs).toBe(125);
  });
});

// ── import lifecycle ──────────────────────────────────────────────────────────

describe('import recording', () => {
  it('endImport with no startImport is a no-op', () => {
    t.endImport(500);
    expect(t.importHistory).toHaveLength(0);
  });

  it('records an import run in history', () => {
    t.startImport(12);
    t.endImport(800);
    expect(t.importHistory).toHaveLength(1);
    expect(t.importHistory[0].imageCount).toBe(12);
    expect(t.importHistory[0].totalMs).toBe(800);
  });

  it('records per-image import entries', () => {
    t.startImport(2);
    t.recordImportImage({ file: 'a.png', headerParseMs: 2, identifyMs: 0, thumbnailMs: 40, pathType: 'fast' });
    t.recordImportImage({ file: 'b.psd', headerParseMs: 3, identifyMs: 50, thumbnailMs: 60, pathType: 'slow' });
    t.endImport(200);
    const run = t.importHistory[0];
    expect(run.images).toHaveLength(2);
    expect(run.images[0].pathType).toBe('fast');
    expect(run.images[1].pathType).toBe('slow');
  });

  it('keeps at most 10 import runs in history', () => {
    for (let i = 0; i < 12; i++) {
      t.startImport(1);
      t.endImport(100);
    }
    expect(t.importHistory).toHaveLength(10);
  });

  it('recordImportImage is a no-op when no import active', () => {
    t.recordImportImage({ file: 'x.png', headerParseMs: 1, identifyMs: 0, thumbnailMs: 10, pathType: 'cached' });
    expect(t.importHistory).toHaveLength(0);
  });
});
