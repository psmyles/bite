// Flipbook atlas generation — assembles source images into a fixed-grid montage
// via ImageMagick. Pure pipeline logic (no Electron imports) so it is usable
// from both the IPC layer and the CLI.

import fs from 'node:fs';
import path from 'node:path';

import type { NodeGraph } from '../../shared/types.js';
import { spawnMagick } from './magick-spawn.js';
import { log } from '../logger.js';

export interface AtlasConfig {
  outputPath: string;
  rows: number;
  cols: number;
  cellWidth: number;
  cellHeight: number;
  sortBy: string;
  bgColor?: number[] | string;
}

function resolveBgColor(bgColor: number[] | string | undefined): string {
  if (Array.isArray(bgColor)) {
    const [r = 0, g = 0, b = 0, a = 1] = bgColor;
    if (a < 0.01) return 'none';
    const h = (v: number) =>
      Math.round(Math.max(0, Math.min(1, v)) * 255)
        .toString(16)
        .padStart(2, '0');
    return `#${h(r)}${h(g)}${h(b)}${h(a)}`;
  }
  return typeof bgColor === 'string' && bgColor ? bgColor : 'none';
}

/** Sorts, truncates to grid capacity, and montages the images. Returns the placed image count. */
export async function generateAtlas(imagePaths: string[], config: AtlasConfig): Promise<number> {
  const { outputPath, rows, cols, cellWidth, cellHeight, sortBy, bgColor } = config;
  const atlasT0 = Date.now();
  log(
    'info',
    `[atlas] generate start: ${imagePaths.length} image(s), ${cols}×${rows} grid (${cellWidth}×${cellHeight}px) → ${outputPath}`
  );

  if (!outputPath?.trim()) throw new Error('No output file path specified.');
  if (imagePaths.length === 0) throw new Error('No images loaded.');

  // Sort images
  const sorted =
    sortBy === 'name'
      ? [...imagePaths].sort((a, b) =>
          path.basename(a).localeCompare(path.basename(b), undefined, { numeric: true, sensitivity: 'base' })
        )
      : sortBy === 'name_desc'
        ? [...imagePaths].sort((a, b) =>
            path.basename(b).localeCompare(path.basename(a), undefined, { numeric: true, sensitivity: 'base' })
          )
        : imagePaths;

  // Truncate to grid capacity
  const selected = sorted.slice(0, rows * cols);

  // -geometry WxH!+0+0 : force exact cell size (stretch), 0px border between tiles
  // -background none   : transparent fill for unfilled cells (PNG/WebP)
  // -tile COLSxROWS    : fixed grid layout
  const args = [
    'montage',
    ...selected,
    '-tile',
    `${cols}x${rows}`,
    '-geometry',
    `${cellWidth}x${cellHeight}!+0+0`,
    '-background',
    resolveBgColor(bgColor),
    outputPath,
  ];

  try {
    await spawnMagick(args);
    log('info', `[atlas] done in ${Date.now() - atlasT0}ms → ${outputPath}`);
  } catch (err) {
    log('error', `[atlas] failed: ${(err as Error).message}`);
    throw err;
  }

  return selected.length;
}

/**
 * Batch execution for a Flipbook Output node — the flipbookOutputNode branch of
 * executeBatch. Reads the grid config from the node's params (including a
 * param-wired background color) and montages the source images into the atlas.
 */
export async function executeFlipbookBatch(
  graph: NodeGraph,
  outputNodeId: string,
  imagePaths: string[],
  overwrite: 'skip' | 'overwrite'
): Promise<{ processed: number; skipped: number; failed: number; errors: string[]; outputFiles: string[] }> {
  const fbNode = graph.nodes.find((n) => n.id === outputNodeId);
  if (!fbNode) throw new Error('Flipbook output node not found in graph.');

  const p = (fbNode.data.params ?? {}) as Record<string, unknown>;
  const outputPath = (p.flipbookOutputPath as string) ?? '';
  if (!outputPath.trim()) throw new Error('No output file path set on the Flipbook Output node.');
  if (imagePaths.length === 0) return { processed: 0, skipped: 0, failed: 0, errors: [], outputFiles: [] };

  if (overwrite === 'skip') {
    const exists = await fs.promises
      .access(outputPath)
      .then(() => true)
      .catch(() => false);
    if (exists) {
      log('info', `[atlas] skipped (exists): ${outputPath}`);
      return { processed: 0, skipped: imagePaths.length, failed: 0, errors: [], outputFiles: [] };
    }
  }

  // bgColor may be wired from a color node (same resolution as the inspector preview)
  let bgColor = Array.isArray(p.bgColor) ? (p.bgColor as number[]) : undefined;
  const bgEdge = graph.edges.find((e) => e.target === outputNodeId && e.targetHandle === 'param-in-bgColor');
  if (bgEdge) {
    const src = graph.nodes.find((n) => n.id === bgEdge.source);
    const srcParams = (src?.data.params ?? {}) as Record<string, unknown>;
    const srcParamKey = (bgEdge.sourceHandle ?? '').replace('param-out-', '');
    const val = srcParams[srcParamKey] ?? srcParams['color'];
    if (Array.isArray(val)) bgColor = val as number[];
  }

  const placed = await generateAtlas(imagePaths, {
    outputPath,
    rows: Number(p.rows ?? 4),
    cols: Number(p.cols ?? 4),
    cellWidth: Number(p.cellWidth ?? 128),
    cellHeight: Number(p.cellHeight ?? 128),
    sortBy: (p.sortBy as string) ?? 'import_order',
    bgColor,
  });

  return {
    processed: placed,
    skipped: imagePaths.length - placed,
    failed: 0,
    errors: [],
    outputFiles: [outputPath],
  };
}
