import { ipcMain, dialog } from 'electron';
import type { BrowserWindow } from 'electron';
import path from 'node:path';
import { spawn } from 'node:child_process';

import { getMagickBinary } from '../pipeline/magick-path.js';
import { IPC } from '../../shared/constants.js';
import { writeOutputLog } from '../pipeline/output-log.js';

interface AtlasConfig {
  outputPath: string;
  rows: number;
  cols: number;
  cellWidth: number;
  cellHeight: number;
  sortBy: string;
  generateLog?: boolean;
}

export function registerAtlasHandlers(getWin: () => BrowserWindow | null): void {
  ipcMain.handle(IPC.ATLAS_BROWSE, async () => {
    const result = await dialog.showSaveDialog(getWin()!, {
      title: 'Save Atlas As',
      filters: [
        { name: 'PNG Image', extensions: ['png'] },
        { name: 'WebP Image', extensions: ['webp'] },
        { name: 'JPEG Image', extensions: ['jpg', 'jpeg'] },
        { name: 'All Files', extensions: ['*'] },
      ],
      defaultPath: 'atlas.png',
    });
    return result.canceled ? null : result.filePath;
  });

  ipcMain.handle(IPC.ATLAS_GENERATE, async (_e, imagePaths: string[], config: AtlasConfig) => {
    const { outputPath, rows, cols, cellWidth, cellHeight, sortBy, generateLog } = config;
    const atlasT0 = Date.now();

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

    const magick = getMagickBinary();
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
      'none',
      outputPath,
    ];

    await new Promise<void>((resolve, reject) => {
      const child = spawn(magick, args, { stdio: ['ignore', 'ignore', 'pipe'] });
      let stderr = '';
      child.stderr?.on('data', (d: Buffer) => {
        stderr += d.toString();
      });
      child.on('close', (code) => {
        if (code === 0) resolve();
        else reject(new Error(`magick montage exited with code ${code}: ${stderr.slice(0, 300).trim()}`));
      });
      child.on('error', (err: Error) => reject(new Error(`Failed to launch magick: ${err.message}`)));
    });

    if (generateLog) {
      await writeOutputLog({
        outputFiles: [outputPath],
        durationMs: Date.now() - atlasT0,
        outputDir: path.dirname(outputPath),
      }).catch((e) => {
        console.error('[log] Failed to write output log:', e);
      });
    }

    return outputPath;
  });
}
