import { ipcMain, dialog, app, shell } from 'electron';
import type { BrowserWindow } from 'electron';
import { readFileSync, writeFileSync, chmodSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import { writeOutputLog } from '../pipeline/output-log.js';
import { log } from '../logger.js';
import { scanFolder } from '../pipeline/scan-folder.js';
import type { NodeGraph } from '../../shared/types.js';
import type { NodeRegistry } from '../nodes/registry.js';
import { PipelineExecutor } from '../pipeline/executor.js';
import { IPC } from '../../shared/constants.js';
import { timings } from '../pipeline/timing.js';
import { registerTextOutputHandlers } from './text-output-handlers.js';
import { registerAtlasHandlers } from './atlas-handlers.js';

export { registerTextOutputHandlers, registerAtlasHandlers };

export function registerRegistryHandlers(registry: NodeRegistry, getWin: () => BrowserWindow | null): void {
  ipcMain.handle(IPC.REGISTRY_GET_ALL, () => registry.getAll());

  registry.onChange((defs) => {
    getWin()?.webContents.send(IPC.REGISTRY_UPDATED, defs);
  });
}

export function registerPipelineHandlers(
  registry: NodeRegistry,
  executor: PipelineExecutor,
  getWin: () => BrowserWindow | null
): void {
  ipcMain.handle(IPC.TIMERS_SET_ENABLED, (_e, enabled: boolean) => {
    timings.enabled = enabled;
  });

  ipcMain.handle(IPC.LOAD_IMAGES, async (_e, paths: string[]) => {
    return Promise.all(paths.map((p) => executor.loadImage(p)));
  });

  ipcMain.handle(IPC.LOAD_IMAGES_WITH_THUMBNAILS, async (_e, paths: string[], size: number) => {
    return Promise.all(paths.map((p) => executor.loadImageWithThumbnail(p, size)));
  });

  // ── Streaming import: N concurrent workers, one push per result ──────────
  // Generation counter prevents a second import from interfering with the first.
  // Each stream start captures its own generation; workers bail if the counter moves.
  let _streamGeneration = 0;

  ipcMain.handle(IPC.LOAD_IMAGES_STREAMING_CANCEL, () => {
    _streamGeneration++;
  });

  ipcMain.handle(IPC.LOAD_IMAGES_STREAMING_START, async (_e, paths: string[], size: number) => {
    const myGen = ++_streamGeneration;
    const isCancelled = () => _streamGeneration !== myGen;
    const importT0 = Date.now();
    log('info', `[import] streaming start: ${paths.length} file(s)`);
    if (timings.enabled) timings.startImport(paths.length);
    const win = getWin();
    const concurrency = os.cpus().length;
    // Batch multiple images per magick spawn to amortize process-spawn overhead.
    // Each worker picks a chunk of BATCH_SIZE images and runs them in one spawn.
    const BATCH_SIZE = 8;
    let idx = 0;
    const allResults: import('../../shared/types.js').ImageInfo[] = [];

    async function worker(): Promise<void> {
      while (true) {
        if (isCancelled()) break;
        // Atomically collect the next batch (sync, no interleaving with other workers)
        const batch: string[] = [];
        while (batch.length < BATCH_SIZE && idx < paths.length) batch.push(paths[idx++]);
        if (batch.length === 0) break;
        try {
          const results = await executor.loadImageWithThumbnailBatch(batch, size);
          for (const result of results) {
            allResults.push(result);
            log('info', `[import] ${result.name} ${result.width}×${result.height} ${result.format}`);
            if (!isCancelled()) win?.webContents.send(IPC.LOAD_IMAGES_STREAMING_RESULT, result);
          }
        } catch (err) {
          console.error('[streaming] Batch failed starting at:', batch[0], err);
        }
      }
    }

    await Promise.all(Array.from({ length: concurrency }, worker));
    log('info', `[import] complete: ${allResults.length} file(s) in ${Date.now() - importT0}ms`);
    if (timings.enabled) timings.endImport(Date.now() - importT0);
    // Return all results so the renderer can recover any events that arrived
    // after the listener was torn down (IPC send vs invoke-resolve race).
    return allResults;
  });

  ipcMain.handle(IPC.GENERATE_THUMBNAIL, async (_e, imagePath: string, size: number) => {
    return executor.generateThumbnail(imagePath, size);
  });

  ipcMain.handle(IPC.EXECUTE_PREVIEW, async (_e, graph: NodeGraph, imagePath: string, fromNodeId?: string) => {
    return executor.executePreview(graph, imagePath, registry, fromNodeId);
  });

  ipcMain.handle(IPC.EXECUTE_BATCH_CANCEL, () => {
    executor.cancelBatch();
  });

  ipcMain.handle(
    IPC.EXECUTE_BATCH,
    async (
      _e,
      graph: NodeGraph,
      imagePaths: string[],
      outputDir: string | null,
      overwrite: 'skip' | 'overwrite',
      generateLog: boolean
    ) => {
      const t0 = Date.now();
      const result = await executor.executeBatch(graph, imagePaths, outputDir, overwrite, registry, (progress) => {
        getWin()?.webContents.send(`${IPC.EXECUTE_BATCH}:progress`, progress);
      });
      if (generateLog) {
        await writeOutputLog({ outputFiles: result.outputFiles, durationMs: Date.now() - t0, outputDir }).catch((e) => {
          console.error('[log] Failed to write output log:', e);
        });
      }
      const { outputFiles: _, ...summary } = result;
      return summary;
    }
  );

  ipcMain.handle(IPC.EXPORT_CLI, async (_e, graph: NodeGraph, shellType: 'powershell' | 'bash' | 'cmd') => {
    const filterMap = {
      powershell: { name: 'PowerShell Script', extensions: ['ps1'] },
      bash: { name: 'Shell Script', extensions: ['sh'] },
      cmd: { name: 'Batch File', extensions: ['bat'] },
    };
    const defaultNames = { powershell: 'imgplex-batch.ps1', bash: 'imgplex-batch.sh', cmd: 'imgplex-batch.bat' };

    const result = await dialog.showSaveDialog(getWin()!, {
      title: 'Export CLI Script',
      defaultPath: defaultNames[shellType],
      filters: [filterMap[shellType], { name: 'All Files', extensions: ['*'] }],
    });
    if (result.canceled || !result.filePath) return null;

    // Companion workflow file lives alongside the script with the same base name
    const scriptBase = path.basename(result.filePath, path.extname(result.filePath));
    const workflowFile = `${scriptBase}.imgplex`;
    const workflowPath = path.join(path.dirname(result.filePath), workflowFile);

    const scriptContent = executor.exportCLI(shellType, workflowFile);
    writeFileSync(result.filePath, scriptContent, 'utf-8');
    writeFileSync(workflowPath, JSON.stringify({ version: '1.0', graph }, null, 2), 'utf-8');

    if (shellType === 'bash') {
      try {
        chmodSync(result.filePath, 0o755);
      } catch {
        /* non-fatal on Windows */
      }
    }
    return result.filePath;
  });
}

const IMAGE_EXTENSIONS = [
  // Common web / display formats
  'jpg',
  'jpeg',
  'png',
  'gif',
  'webp',
  'avif',
  'svg',
  'svgz',
  'ico',
  'bmp',
  // TIFF family
  'tif',
  'tiff',
  // HEIF / Apple
  'heic',
  'heif',
  // JPEG variants
  'jp2',
  'j2k',
  'jpf',
  'jpx',
  'jxl',
  // Professional / compositing
  'psd',
  'psb',
  'exr',
  'hdr',
  'dpx',
  'cin',
  // Camera RAW
  'cr2',
  'cr3',
  'nef',
  'nrw',
  'arw',
  'dng',
  'orf',
  'raf',
  'rw2',
  'pef',
  'srw',
  'x3f',
  '3fr',
  'kdc',
  'mrw',
  'erf',
  'rwl',
  // Legacy / misc raster
  'tga',
  'pcx',
  'ppm',
  'pgm',
  'pbm',
  'pnm',
  'sgi',
  'rgb',
  'rgba',
  'miff',
  'mng',
  'jng',
  'xbm',
  'xpm',
  'xwd',
  'sun',
  'iff',
  'lbm',
  'wbmp',
  'pict',
  'pct',
  'dds',
  'fits',
  'fts',
];

function readWorkflowFile(filePath: string): { graph: unknown; filePath: string } {
  const raw = readFileSync(filePath, 'utf-8');
  const data = JSON.parse(raw) as Record<string, unknown>;
  if (!data || typeof data !== 'object' || !data.graph) {
    throw new Error('Invalid workflow file: missing graph data');
  }
  return { graph: data.graph, filePath };
}

export function registerWorkflowHandlers(getWin: () => BrowserWindow | null): void {
  ipcMain.handle(IPC.WORKFLOW_SAVE, async (_e, graph: unknown, filePath: string | null) => {
    let targetPath = filePath ?? null;
    if (!targetPath) {
      const result = await dialog.showSaveDialog(getWin()!, {
        filters: [{ name: 'imgplex Workflow', extensions: ['imgplex'] }],
        defaultPath: 'workflow.imgplex',
      });
      if (result.canceled || !result.filePath) return null;
      targetPath = result.filePath;
    }
    writeFileSync(targetPath, JSON.stringify({ version: '1.0', graph }, null, 2), 'utf-8');
    return targetPath;
  });

  ipcMain.handle(IPC.WORKFLOW_LOAD, async () => {
    const result = await dialog.showOpenDialog(getWin()!, {
      properties: ['openFile'],
      filters: [
        { name: 'imgplex Workflow', extensions: ['imgplex'] },
        { name: 'All Files', extensions: ['*'] },
      ],
    });
    if (result.canceled || result.filePaths.length === 0) return null;
    return readWorkflowFile(result.filePaths[0]);
  });

  ipcMain.handle(IPC.WORKFLOW_OPEN_PATH, (_e, filePath: string) => {
    return readWorkflowFile(filePath);
  });

  let isQuitting = false;
  ipcMain.handle(IPC.APP_QUIT, () => {
    isQuitting = true;
    app.quit();
  });

  // Intercept the window X button — ask renderer to confirm dirty state first
  const setupCloseInterception = () => {
    const win = getWin();
    if (!win) return;
    win.on('close', (e) => {
      if (!isQuitting) {
        e.preventDefault();
        win.webContents.send(IPC.MENU_EXIT);
      }
    });
  };
  // The window may not exist yet; call after a tick so createWindow() has run
  setImmediate(setupCloseInterception);
}

export function registerDialogHandlers(getWin: () => BrowserWindow | null): void {
  ipcMain.handle(IPC.OPEN_IMAGES_DIALOG, async () => {
    const result = await dialog.showOpenDialog(getWin()!, {
      properties: ['openFile', 'multiSelections'],
      filters: [
        { name: 'All Supported Images', extensions: IMAGE_EXTENSIONS },
        { name: 'All Files', extensions: ['*'] },
      ],
    });
    return result.canceled ? [] : result.filePaths;
  });

  ipcMain.handle(IPC.OPEN_FOLDER_DIALOG, async () => {
    const result = await dialog.showOpenDialog(getWin()!, {
      properties: ['openDirectory', 'createDirectory'],
    });
    return result.canceled ? null : result.filePaths[0];
  });

  ipcMain.handle(IPC.SCAN_FOLDER_DIALOG, async (_e, opts: { recursive: boolean; extensions: string[] }) => {
    const result = await dialog.showOpenDialog(getWin()!, {
      properties: ['openDirectory'],
      buttonLabel: 'Select Folder',
    });
    if (result.canceled || !result.filePaths[0]) return [];
    return scanFolder(result.filePaths[0], opts.recursive, new Set(opts.extensions.map((e) => e.toLowerCase())));
  });
}

export function registerScanHandlers(): void {
  ipcMain.handle(IPC.SCAN_FOLDER, (_e, opts: { folderPath: string; recursive: boolean; extensions: string[] }) => {
    return scanFolder(opts.folderPath, opts.recursive, new Set(opts.extensions.map((e) => e.toLowerCase())));
  });
}

export function registerShellHandlers(): void {
  ipcMain.handle(IPC.SHELL_OPEN_EXTERNAL, (_e, url: string) => shell.openExternal(url));
  ipcMain.handle(IPC.SHELL_OPEN_PATH, (_e, folderPath: string) => shell.openPath(folderPath));
}
