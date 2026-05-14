import fs from 'node:fs';
import path from 'node:path';

const JPEG_SOF_MARKERS = new Set([0xc0, 0xc1, 0xc2, 0xc3, 0xc5, 0xc6, 0xc7, 0xc9, 0xca, 0xcb, 0xcd, 0xce, 0xcf]);
const TGA_VALID_TYPES = new Set([0, 1, 2, 3, 9, 10, 11]);
const HEADER_READ_BYTES = 131072; // 128 KB — covers EXIF/APP markers before SOF in virtually all JPEGs

export interface HeaderDims {
  width: number;
  height: number;
  isJpeg: boolean;
  format: string; // inferred from header/extension
}

export async function readHeaderDimensions(filePath: string): Promise<HeaderDims | null> {
  let fd: fs.promises.FileHandle | null = null;
  try {
    fd = await fs.promises.open(filePath, 'r');
    const buf = Buffer.alloc(HEADER_READ_BYTES);
    const { bytesRead } = await fd.read(buf, 0, HEADER_READ_BYTES, 0);
    if (bytesRead < 12) return null;

    // ── PNG ──────────────────────────────────────────────────────────────────
    if (buf[0] === 0x89 && buf[1] === 0x50 && buf[2] === 0x4e && buf[3] === 0x47) {
      if (bytesRead < 24) return null;
      return { width: buf.readUInt32BE(16), height: buf.readUInt32BE(20), isJpeg: false, format: 'PNG' };
    }

    // ── BMP ──────────────────────────────────────────────────────────────────
    if (buf[0] === 0x42 && buf[1] === 0x4d) {
      // "BM"
      if (bytesRead < 26) return null;
      const w = Math.abs(buf.readInt32LE(18));
      const h = Math.abs(buf.readInt32LE(22));
      if (w > 0 && h > 0) return { width: w, height: h, isJpeg: false, format: 'BMP' };
      return null;
    }

    // ── WEBP ─────────────────────────────────────────────────────────────────
    if (
      buf[0] === 0x52 &&
      buf[1] === 0x49 &&
      buf[2] === 0x46 &&
      buf[3] === 0x46 &&
      buf[8] === 0x57 &&
      buf[9] === 0x45 &&
      buf[10] === 0x42 &&
      buf[11] === 0x50
    ) {
      if (bytesRead < 30) return null;
      const chunkType = buf.subarray(12, 16).toString('ascii');
      if (chunkType === 'VP8 ') {
        if (buf[23] === 0x9d && buf[24] === 0x01 && buf[25] === 0x2a) {
          const w = (buf[26] | (buf[27] << 8)) & 0x3fff;
          const h = (buf[28] | (buf[29] << 8)) & 0x3fff;
          if (w > 0 && h > 0) return { width: w, height: h, isJpeg: false, format: 'WEBP' };
        }
        return null;
      }
      if (chunkType === 'VP8L') {
        if (bytesRead < 25 || buf[20] !== 0x2f) return null;
        const val = buf.readUInt32LE(21);
        return { width: (val & 0x3fff) + 1, height: ((val >> 14) & 0x3fff) + 1, isJpeg: false, format: 'WEBP' };
      }
      if (chunkType === 'VP8X') {
        const w = (buf[24] | (buf[25] << 8) | (buf[26] << 16)) + 1;
        const h = (buf[27] | (buf[28] << 8) | (buf[29] << 16)) + 1;
        return { width: w, height: h, isJpeg: false, format: 'WEBP' };
      }
      return null;
    }

    // ── JPEG ─────────────────────────────────────────────────────────────────
    if (buf[0] === 0xff && buf[1] === 0xd8) {
      let pos = 2;
      while (pos + 1 < bytesRead) {
        if (buf[pos] !== 0xff) break;
        const marker = buf[pos + 1];
        pos += 2;
        if (marker === 0xd8 || marker === 0xd9 || (marker >= 0xd0 && marker <= 0xd7)) continue;
        if (marker === 0xda) break;
        if (pos + 1 >= bytesRead) break;
        const length = buf.readUInt16BE(pos);
        if (length < 2) break;
        if (JPEG_SOF_MARKERS.has(marker) && pos + 6 < bytesRead) {
          const h = buf.readUInt16BE(pos + 3);
          const w = buf.readUInt16BE(pos + 5);
          if (w > 0 && h > 0) return { width: w, height: h, isJpeg: true, format: 'JPEG' };
        }
        pos += length;
      }
      return null;
    }

    // ── TGA ──────────────────────────────────────────────────────────────────
    // No universal magic bytes — detect by extension + header validation
    const extLower = path.extname(filePath).toLowerCase();
    if (extLower === '.tga' || extLower === '.targa') {
      if (bytesRead < 16) return null;
      if (!TGA_VALID_TYPES.has(buf[2])) return null; // image type must be valid
      const w = buf.readUInt16LE(12);
      const h = buf.readUInt16LE(14);
      if (w > 0 && h > 0) return { width: w, height: h, isJpeg: false, format: 'TGA' };
      return null;
    }

    return null; // unrecognized format
  } catch {
    return null;
  } finally {
    await fd?.close().catch(() => {});
  }
}

export async function fileToDataUrl(filePath: string): Promise<string> {
  const data = await fs.promises.readFile(filePath);
  const ext = path.extname(filePath).slice(1).toLowerCase();
  const mime = ext === 'jpg' || ext === 'jpeg' ? 'image/jpeg' : ext === 'webp' ? 'image/webp' : 'image/png';
  return `data:${mime};base64,${data.toString('base64')}`;
}
