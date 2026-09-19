import os from 'node:os';
import path from 'node:path';
import { performance } from 'node:perf_hooks';

import { scanFolder } from '../src/main/pipeline/scan-folder.js';
import { loadImageWithThumbnailBatch } from '../src/main/pipeline/thumbnail-service.js';

const root = path.resolve(process.argv[2]);
const size = Number(process.argv[3] ?? 128);
const paths = await scanFolder(root, true, new Set([
  'jpg', 'jpeg', 'png', 'gif', 'webp', 'avif', 'bmp', 'tga', 'tif', 'tiff', 'psd', 'exr', 'hdr',
]));

async function load(): Promise<number> {
  const chunks: string[][] = [];
  for (let index = 0; index < paths.length; index += 8) chunks.push(paths.slice(index, index + 8));
  let next = 0;
  let images = 0;
  async function worker(): Promise<void> {
    while (next < chunks.length) {
      const chunk = chunks[next++];
      const loaded = await loadImageWithThumbnailBatch(chunk, size);
      images += loaded.length;
    }
  }
  await Promise.all(Array.from({ length: Math.min(os.cpus().length, chunks.length) }, worker));
  return images;
}

const coldStarted = performance.now();
const coldImages = await load();
const coldMs = performance.now() - coldStarted;
const warmStarted = performance.now();
const warmImages = await load();
const warmMs = performance.now() - warmStarted;
console.log(JSON.stringify({
  backend: 'node',
  root,
  images: paths.length,
  cold: { elapsed_ms: coldMs, images: coldImages },
  warm: { elapsed_ms: warmMs, images: warmImages },
}));
