import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { readHeaderDimensions } from '../main/pipeline/image-header.js';

let tmpDir: string;

beforeEach(() => {
  tmpDir = mkdtempSync(join(tmpdir(), 'imgplex-header-test-'));
});

afterEach(() => {
  rmSync(tmpDir, { recursive: true, force: true });
});

function writeFile(name: string, buf: Buffer): string {
  const p = join(tmpDir, name);
  writeFileSync(p, buf);
  return p;
}

function pngBuf(width: number, height: number): Buffer {
  const b = Buffer.alloc(24);
  // PNG magic
  b[0] = 0x89;
  b[1] = 0x50;
  b[2] = 0x4e;
  b[3] = 0x47;
  b[4] = 0x0d;
  b[5] = 0x0a;
  b[6] = 0x1a;
  b[7] = 0x0a;
  // IHDR chunk length + type (not parsed)
  b[8] = 0x00;
  b[9] = 0x00;
  b[10] = 0x00;
  b[11] = 0x0d;
  b[12] = 0x49;
  b[13] = 0x48;
  b[14] = 0x44;
  b[15] = 0x52;
  // width/height as uint32 BE
  b.writeUInt32BE(width, 16);
  b.writeUInt32BE(height, 20);
  return b;
}

function bmpBuf(width: number, height: number): Buffer {
  const b = Buffer.alloc(26);
  b[0] = 0x42;
  b[1] = 0x4d; // "BM"
  b.writeInt32LE(width, 18);
  b.writeInt32LE(height, 22);
  return b;
}

function webpVp8Buf(width: number, height: number): Buffer {
  const b = Buffer.alloc(30);
  // RIFF header
  b[0] = 0x52;
  b[1] = 0x49;
  b[2] = 0x46;
  b[3] = 0x46;
  // file size (ignored)
  // WEBP tag
  b[8] = 0x57;
  b[9] = 0x45;
  b[10] = 0x42;
  b[11] = 0x50;
  // VP8  chunk type (with trailing space)
  b[12] = 0x56;
  b[13] = 0x50;
  b[14] = 0x38;
  b[15] = 0x20;
  // chunk size (ignored)
  // VP8 start code at offset 23
  b[23] = 0x9d;
  b[24] = 0x01;
  b[25] = 0x2a;
  // width and height (14-bit values, LE)
  b.writeUInt16LE(width & 0x3fff, 26);
  b.writeUInt16LE(height & 0x3fff, 28);
  return b;
}

function webpVp8LBuf(width: number, height: number): Buffer {
  const b = Buffer.alloc(30);
  b[0] = 0x52;
  b[1] = 0x49;
  b[2] = 0x46;
  b[3] = 0x46;
  b[8] = 0x57;
  b[9] = 0x45;
  b[10] = 0x42;
  b[11] = 0x50;
  // VP8L chunk type
  b[12] = 0x56;
  b[13] = 0x50;
  b[14] = 0x38;
  b[15] = 0x4c;
  // signature at offset 20
  b[20] = 0x2f;
  // packed bits at offset 21 (uint32 LE): bits 0-13 = width-1, bits 14-27 = height-1
  const val = ((height - 1) << 14) | (width - 1);
  b.writeUInt32LE(val, 21);
  return b;
}

function webpVp8XBuf(width: number, height: number): Buffer {
  const b = Buffer.alloc(30);
  b[0] = 0x52;
  b[1] = 0x49;
  b[2] = 0x46;
  b[3] = 0x46;
  b[8] = 0x57;
  b[9] = 0x45;
  b[10] = 0x42;
  b[11] = 0x50;
  // VP8X chunk type
  b[12] = 0x56;
  b[13] = 0x50;
  b[14] = 0x38;
  b[15] = 0x58;
  // width-1 as 24-bit LE at offset 24
  const w1 = width - 1;
  const h1 = height - 1;
  b[24] = w1 & 0xff;
  b[25] = (w1 >> 8) & 0xff;
  b[26] = (w1 >> 16) & 0xff;
  b[27] = h1 & 0xff;
  b[28] = (h1 >> 8) & 0xff;
  b[29] = (h1 >> 16) & 0xff;
  return b;
}

function jpegBuf(width: number, height: number): Buffer {
  // SOI + SOF0 marker immediately after
  const b = Buffer.alloc(16);
  b[0] = 0xff;
  b[1] = 0xd8; // SOI
  b[2] = 0xff;
  b[3] = 0xc0; // SOF0
  b.writeUInt16BE(0x000b, 4); // length = 11 (>= 2)
  b[6] = 0x08; // precision
  b.writeUInt16BE(height, 7); // pos+3 = 4+3 = 7
  b.writeUInt16BE(width, 9); // pos+5 = 4+5 = 9
  return b;
}

function tgaBuf(width: number, height: number): Buffer {
  const b = Buffer.alloc(16);
  b[2] = 0x02; // image type: uncompressed true-color (valid)
  b.writeUInt16LE(width, 12);
  b.writeUInt16LE(height, 14);
  return b;
}

// ── PNG ───────────────────────────────────────────────────────────────────────

describe('PNG', () => {
  it('reads width and height from PNG header', async () => {
    const p = writeFile('test.png', pngBuf(800, 600));
    const result = await readHeaderDimensions(p);
    expect(result).toEqual({ width: 800, height: 600, isJpeg: false, format: 'PNG' });
  });
});

// ── BMP ───────────────────────────────────────────────────────────────────────

describe('BMP', () => {
  it('reads width and height from BMP header', async () => {
    const p = writeFile('test.bmp', bmpBuf(640, 480));
    const result = await readHeaderDimensions(p);
    expect(result).toEqual({ width: 640, height: 480, isJpeg: false, format: 'BMP' });
  });

  it('handles negative height (bottom-up BMP)', async () => {
    const b = Buffer.alloc(26);
    b[0] = 0x42;
    b[1] = 0x4d;
    b.writeInt32LE(320, 18);
    b.writeInt32LE(-240, 22); // negative height is valid in BMP
    const p = writeFile('test.bmp', b);
    const result = await readHeaderDimensions(p);
    expect(result).toEqual({ width: 320, height: 240, isJpeg: false, format: 'BMP' });
  });
});

// ── WebP ──────────────────────────────────────────────────────────────────────

describe('WebP VP8 (lossy)', () => {
  it('reads width and height', async () => {
    const p = writeFile('test.webp', webpVp8Buf(1024, 768));
    const result = await readHeaderDimensions(p);
    expect(result).toEqual({ width: 1024, height: 768, isJpeg: false, format: 'WEBP' });
  });
});

describe('WebP VP8L (lossless)', () => {
  it('reads width and height', async () => {
    const p = writeFile('test.webp', webpVp8LBuf(256, 128));
    const result = await readHeaderDimensions(p);
    expect(result).toEqual({ width: 256, height: 128, isJpeg: false, format: 'WEBP' });
  });
});

describe('WebP VP8X (extended)', () => {
  it('reads width and height', async () => {
    const p = writeFile('test.webp', webpVp8XBuf(800, 600));
    const result = await readHeaderDimensions(p);
    expect(result).toEqual({ width: 800, height: 600, isJpeg: false, format: 'WEBP' });
  });
});

// ── JPEG ──────────────────────────────────────────────────────────────────────

describe('JPEG', () => {
  it('reads width and height from SOF0', async () => {
    const p = writeFile('test.jpg', jpegBuf(640, 480));
    const result = await readHeaderDimensions(p);
    expect(result).toEqual({ width: 640, height: 480, isJpeg: true, format: 'JPEG' });
  });
});

// ── TGA ───────────────────────────────────────────────────────────────────────

describe('TGA', () => {
  it('reads width and height from .tga file', async () => {
    const p = writeFile('test.tga', tgaBuf(320, 240));
    const result = await readHeaderDimensions(p);
    expect(result).toEqual({ width: 320, height: 240, isJpeg: false, format: 'TGA' });
  });

  it('rejects TGA with invalid image type', async () => {
    const b = tgaBuf(320, 240);
    b[2] = 0xff; // invalid image type
    const p = writeFile('test.tga', b);
    expect(await readHeaderDimensions(p)).toBeNull();
  });
});

// ── Edge cases ────────────────────────────────────────────────────────────────

describe('edge cases', () => {
  it('returns null for unknown format', async () => {
    const b = Buffer.alloc(20).fill(0xaa);
    const p = writeFile('test.xyz', b);
    expect(await readHeaderDimensions(p)).toBeNull();
  });

  it('returns null for truncated buffer (< 12 bytes)', async () => {
    const b = Buffer.alloc(8).fill(0x89); // looks like partial PNG magic
    b[0] = 0x89;
    b[1] = 0x50;
    b[2] = 0x4e;
    b[3] = 0x47;
    const p = writeFile('test.png', b);
    expect(await readHeaderDimensions(p)).toBeNull();
  });

  it('returns null for non-existent file without throwing', async () => {
    expect(await readHeaderDimensions(join(tmpDir, 'no-such-file.png'))).toBeNull();
  });
});
