import { spawn } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import type { ImageInfo } from '../../shared/types.js';
import { getMagickBinary } from './magick-path.js';
import { readHeaderDimensions, fileToDataUrl } from './image-header.js';
import { spawnMagick } from './magick-spawn.js';
import { timings } from './timing.js';

export const TEMP_DIR = path.join(os.tmpdir(), 'imgplex-preview');
fs.mkdirSync(TEMP_DIR, { recursive: true });

export function shortHash(data: string): string {
  return crypto.createHash('md5').update(data).digest('hex').slice(0, 8);
}

const _metaCache = new Map<string, { width: number; height: number; format: string }>();

export function getMetaCached(imagePath: string): { width: number; height: number; format: string } | undefined {
  return _metaCache.get(imagePath);
}

export async function loadImage(imagePath: string): Promise<ImageInfo> {
  const output = await new Promise<string>((resolve, reject) => {
    const proc = spawn(getMagickBinary(), ['identify', '-format', '%w %h %m', imagePath]);
    const out: string[] = [];
    const err: string[] = [];
    proc.stdout.on('data', (c: Buffer) => out.push(c.toString()));
    proc.stderr.on('data', (c: Buffer) => err.push(c.toString()));
    proc.on('close', (code) => {
      if (code === 0) resolve(out.join('').trim());
      else reject(new Error(err.join('').trim()));
    });
    proc.on('error', reject);
  });

  // identify returns one line per frame; take the first
  const firstLine = output.split('\n')[0];
  const [widthStr, heightStr, format] = firstLine.split(' ');
  const stat = await fs.promises.stat(imagePath);

  return {
    path: imagePath,
    name: path.basename(imagePath),
    width: parseInt(widthStr ?? '0', 10) || 0,
    height: parseInt(heightStr ?? '0', 10) || 0,
    format: format ?? 'UNKNOWN',
    sizeBytes: stat.size,
  };
}

export async function loadImageWithThumbnail(
  imagePath: string,
  size: number
): Promise<ImageInfo & { thumbnailDataUrl: string }> {
  const hash = shortHash(imagePath);
  const thumbPath = path.join(TEMP_DIR, `thumb_${hash}_${size}.webp`);

  // Check thumb cache first; only fetch srcStat if thumb actually exists
  const thumbStat = await fs.promises.stat(thumbPath).catch(() => null);
  const cached = _metaCache.get(imagePath);

  let needsRegen = !thumbStat;
  if (thumbStat) {
    const srcStat = await fs.promises.stat(imagePath).catch(() => null);
    needsRegen = !!(srcStat && thumbStat.mtimeMs < srcStat.mtimeMs);
  }

  let width: number, height: number, format: string;

  if (!needsRegen && cached) {
    // Full cache hit — zero magick calls
    ({ width, height, format } = cached);
    const [fileStat, dataUrl] = await Promise.all([
      fs.promises.stat(imagePath).catch(() => null),
      fileToDataUrl(thumbPath),
    ]);
    return {
      path: imagePath,
      name: path.basename(imagePath),
      width,
      height,
      format,
      sizeBytes: fileStat?.size ?? 0,
      thumbnailDataUrl: dataUrl,
    };
  }

  // Cache miss — parse header natively first to enable JPEG DCT size hint
  await fs.promises.mkdir(TEMP_DIR, { recursive: true });

  const hdr = await readHeaderDimensions(imagePath);

  if (hdr) {
    // Fast path: dimensions known from header — spawn magick with optional JPEG size hint
    const magickArgs: string[] = [];
    if (hdr.isJpeg) {
      // Tell libjpeg to decode at 1/8 scale; picks smallest factor >= requested size,
      // giving up to ~64× less data to decompress for large images.
      magickArgs.push('-define', `jpeg:size=${size * 2}x${size * 2}`);
    }
    magickArgs.push(`${imagePath}[0]`, '-thumbnail', `${size}x${size}>`, '-quality', '85', thumbPath);
    await new Promise<void>((resolve, reject) => {
      const proc = spawn(getMagickBinary(), magickArgs, { env: { ...process.env, MAGICK_THREAD_LIMIT: '1' } });
      const stderr: string[] = [];
      proc.stderr.on('data', (d: Buffer) => stderr.push(d.toString()));
      proc.on('close', (code) => {
        if (code === 0) resolve();
        else reject(new Error(`magick exited ${code}: ${stderr.join('').trim()}`));
      });
      proc.on('error', (err) =>
        reject(new Error(`Failed to spawn magick: ${err.message}. Is ImageMagick v7 in PATH?`))
      );
    });
    width = hdr.width;
    height = hdr.height;
    format = hdr.format;
  } else {
    // Slow path (RAW, TIFF, PSD, GIF, …): combined -print + thumbnail spawn
    let stdout = '';
    await new Promise<void>((resolve, reject) => {
      const proc = spawn(
        getMagickBinary(),
        [`${imagePath}[0]`, '-print', '%w %h %m\n', '-thumbnail', `${size}x${size}>`, '-quality', '85', thumbPath],
        { env: { ...process.env, MAGICK_THREAD_LIMIT: '1' } }
      );
      proc.stdout.on('data', (d: Buffer) => {
        stdout += d.toString();
      });
      const stderr: string[] = [];
      proc.stderr.on('data', (d: Buffer) => stderr.push(d.toString()));
      proc.on('close', (code) => {
        if (code === 0) resolve();
        else reject(new Error(`magick exited ${code}: ${stderr.join('').trim()}`));
      });
      proc.on('error', (err) =>
        reject(new Error(`Failed to spawn magick: ${err.message}. Is ImageMagick v7 in PATH?`))
      );
    });
    const [ws, hs, fmt] = stdout.trim().split(' ');
    width = parseInt(ws, 10) || 0;
    height = parseInt(hs, 10) || 0;
    format = fmt ?? 'UNKNOWN';
  }

  _metaCache.set(imagePath, { width, height, format });

  const [fileStat, dataUrl] = await Promise.all([
    fs.promises.stat(imagePath).catch(() => null),
    fileToDataUrl(thumbPath),
  ]);

  return {
    path: imagePath,
    name: path.basename(imagePath),
    width,
    height,
    format,
    sizeBytes: fileStat?.size ?? 0,
    thumbnailDataUrl: dataUrl,
  };
}

export async function loadImageWithThumbnailBatch(
  imagePaths: string[],
  size: number
): Promise<Array<ImageInfo & { thumbnailDataUrl: string }>> {
  if (imagePaths.length === 0) return [];

  await fs.promises.mkdir(TEMP_DIR, { recursive: true });

  const hashes = imagePaths.map((p) => shortHash(p));
  const thumbPaths = hashes.map((h) => path.join(TEMP_DIR, `thumb_${h}_${size}.webp`));

  // ── Phase 1: parallel cache + header checks ──────────────────────────────
  const headerStartMs = Date.now();
  const [thumbStats, hdrs] = await Promise.all([
    Promise.all(thumbPaths.map((tp) => fs.promises.stat(tp).catch(() => null))),
    Promise.all(imagePaths.map((p) => readHeaderDimensions(p))),
  ]);
  const headerTotalMs = Date.now() - headerStartMs;

  // Only stat source file when thumb exists (avoids extra stat on cache miss)
  const needsRegen: boolean[] = await Promise.all(
    imagePaths.map(async (p, i) => {
      if (!thumbStats[i]) return true;
      const srcStat = await fs.promises.stat(p).catch(() => null);
      return !!(srcStat && thumbStats[i]!.mtimeMs < srcStat.mtimeMs);
    })
  );

  // ── Phase 2: classify each image ─────────────────────────────────────────
  const fastMisses: number[] = []; // thumb regen needed, header parsed → batch spawn
  const slowMisses: number[] = []; // thumb regen needed, no header     → individual spawn
  const metaOnlyMisses: number[] = []; // thumb cached but _metaCache empty → identify only

  for (let i = 0; i < imagePaths.length; i++) {
    if (needsRegen[i]) {
      if (hdrs[i]) fastMisses.push(i);
      else slowMisses.push(i);
    } else if (!_metaCache.has(imagePaths[i])) {
      // Thumb exists but app restarted — _metaCache is empty; recover metadata.
      if (hdrs[i]) {
        // Fast path: header already read in Phase 1, no spawn needed.
        _metaCache.set(imagePaths[i], { width: hdrs[i]!.width, height: hdrs[i]!.height, format: hdrs[i]!.format });
      } else {
        // Slow path (PSD, TIFF, …): need a meta-only identify call.
        metaOnlyMisses.push(i);
      }
    }
  }

  // ── Phase 2b: meta-only identify for slow-path cached images ─────────────
  for (const i of metaOnlyMisses) {
    try {
      const output = await new Promise<string>((resolve, reject) => {
        const proc = spawn(getMagickBinary(), ['identify', '-format', '%w %h %m\n', `${imagePaths[i]}[0]`]);
        const out: string[] = [];
        proc.stdout.on('data', (d: Buffer) => out.push(d.toString()));
        proc.on('close', (code) => (code === 0 ? resolve(out.join('')) : reject(new Error(`identify exited ${code}`))));
        proc.on('error', (err) => reject(err));
      });
      const [ws, hs, fmt] = output.split('\n')[0].split(' ');
      _metaCache.set(imagePaths[i], {
        width: parseInt(ws, 10) || 0,
        height: parseInt(hs, 10) || 0,
        format: fmt ?? 'UNKNOWN',
      });
    } catch (err) {
      console.error('[import] Failed to get meta for cached image:', imagePaths[i], err);
    }
  }

  // ── Phase 3: single batch spawn for all fast-path misses ─────────────────
  let fastThumbMs = 0;
  if (fastMisses.length > 0) {
    // Build one magick command: each image reads, thumbnails, writes, then +delete
    // (last image writes directly to its thumbPath as the final output)
    const batchArgs: string[] = [];
    for (let j = 0; j < fastMisses.length; j++) {
      const i = fastMisses[j];
      batchArgs.push(`${imagePaths[i]}[0]`);
      batchArgs.push('-thumbnail', `${size}x${size}>`, '-quality', '85');
      if (j < fastMisses.length - 1) batchArgs.push('-write', thumbPaths[i], '+delete');
      else batchArgs.push(thumbPaths[i]);
    }

    let batchOk = true;
    const batchSpawnT0 = Date.now();
    try {
      await new Promise<void>((resolve, reject) => {
        const proc = spawn(getMagickBinary(), batchArgs, { env: { ...process.env, MAGICK_THREAD_LIMIT: '1' } });
        const stderr: string[] = [];
        proc.stderr.on('data', (d: Buffer) => stderr.push(d.toString()));
        proc.on('close', (code) =>
          code === 0 ? resolve() : reject(new Error(`magick exited ${code}: ${stderr.join('').trim()}`))
        );
        proc.on('error', (err) =>
          reject(new Error(`Failed to spawn magick: ${err.message}. Is ImageMagick v7 in PATH?`))
        );
      });
    } catch (err) {
      batchOk = false;
      console.warn('[import] Batch spawn failed, falling back to individual processing:', (err as Error).message);
      for (const i of fastMisses) slowMisses.push(i);
    }

    if (batchOk) {
      fastThumbMs = Date.now() - batchSpawnT0;
      for (const i of fastMisses) {
        _metaCache.set(imagePaths[i], { width: hdrs[i]!.width, height: hdrs[i]!.height, format: hdrs[i]!.format });
      }
    }
  }

  // ── Phase 4: individual spawns for slow-path misses ──────────────────────
  // These are rare (PSD, TIFF, …) so sequential processing is fine.
  const slowThumbMs = new Map<number, number>();
  for (const i of slowMisses) {
    try {
      let stdout = '';
      const slowT0 = Date.now();
      await new Promise<void>((resolve, reject) => {
        const proc = spawn(
          getMagickBinary(),
          [
            `${imagePaths[i]}[0]`,
            '-print',
            '%w %h %m\n',
            '-thumbnail',
            `${size}x${size}>`,
            '-quality',
            '85',
            thumbPaths[i],
          ],
          { env: { ...process.env, MAGICK_THREAD_LIMIT: '1' } }
        );
        proc.stdout.on('data', (d: Buffer) => {
          stdout += d.toString();
        });
        const stderr: string[] = [];
        proc.stderr.on('data', (d: Buffer) => stderr.push(d.toString()));
        proc.on('close', (code) =>
          code === 0 ? resolve() : reject(new Error(`magick exited ${code}: ${stderr.join('').trim()}`))
        );
        proc.on('error', (err) =>
          reject(new Error(`Failed to spawn magick: ${err.message}. Is ImageMagick v7 in PATH?`))
        );
      });
      slowThumbMs.set(i, Date.now() - slowT0);
      const [ws, hs, fmt] = stdout.trim().split(' ');
      _metaCache.set(imagePaths[i], {
        width: parseInt(ws, 10) || 0,
        height: parseInt(hs, 10) || 0,
        format: fmt ?? 'UNKNOWN',
      });
    } catch (err) {
      console.error('[import] Failed to process:', imagePaths[i], err);
    }
  }

  // ── Phase 5: parallel stat + thumbnail read for all images ───────────────
  const fastMissSet = new Set(fastMisses);
  const slowMissSet = new Set(slowMisses);
  const avgHeaderPerImage = imagePaths.length > 0 ? Math.round(headerTotalMs / imagePaths.length) : 0;
  const avgFastThumb = fastMisses.length > 0 ? Math.round(fastThumbMs / fastMisses.length) : 0;

  return Promise.all(
    imagePaths.map(async (imagePath, i) => {
      const meta = _metaCache.get(imagePath) ?? { width: 0, height: 0, format: 'UNKNOWN' };
      const [fileStat, thumbnailDataUrl] = await Promise.all([
        fs.promises.stat(imagePath).catch(() => null),
        fileToDataUrl(thumbPaths[i]),
      ]);
      if (timings.enabled) {
        const pathType: 'fast' | 'slow' | 'cached' = fastMissSet.has(i)
          ? 'fast'
          : slowMissSet.has(i)
            ? 'slow'
            : 'cached';
        timings.recordImportImage({
          file: imagePath,
          headerParseMs: avgHeaderPerImage,
          identifyMs: slowMissSet.has(i) ? (slowThumbMs.get(i) ?? 0) : 0,
          thumbnailMs: fastMissSet.has(i) ? avgFastThumb : (slowThumbMs.get(i) ?? 0),
          pathType,
        });
      }
      return {
        path: imagePath,
        name: path.basename(imagePath),
        ...meta,
        sizeBytes: fileStat?.size ?? 0,
        thumbnailDataUrl,
      };
    })
  );
}

export async function generateThumbnail(imagePath: string, size: number): Promise<string> {
  const hash = shortHash(imagePath);
  const thumbPath = path.join(TEMP_DIR, `thumb_${hash}_${size}.webp`);

  // Only regenerate if missing or source is newer
  const [srcStat, thumbStat] = await Promise.all([
    fs.promises.stat(imagePath).catch(() => null),
    fs.promises.stat(thumbPath).catch(() => null),
  ]);

  if (!thumbStat || (srcStat && thumbStat.mtimeMs < srcStat.mtimeMs)) {
    await fs.promises.mkdir(TEMP_DIR, { recursive: true });
    // [0] selects only the first frame — without it, multi-frame images (e.g.
    // JPEGs with embedded EXIF thumbnails) cause magick to write numbered
    // output files instead of the expected path.
    await spawnMagick([`${imagePath}[0]`, '-thumbnail', `${size}x${size}>`, '-quality', '85', thumbPath]);
  }

  return fileToDataUrl(thumbPath);
}
