import { ipcMain, dialog } from 'electron';
import type { BrowserWindow } from 'electron';

import type { NodeGraph } from '../../shared/types.js';
import type { NodeRegistry } from '../nodes/registry.js';
import { computeTextOutputLines } from '../pipeline/text-output.js';
import { IPC } from '../../shared/constants.js';

export function registerTextOutputHandlers(registry: NodeRegistry, getWin: () => BrowserWindow | null): void {
  ipcMain.handle(IPC.TEXT_OUTPUT_BROWSE, async () => {
    const result = await dialog.showSaveDialog(getWin()!, {
      title: 'Choose Output File',
      filters: [
        { name: 'Text File', extensions: ['txt'] },
        { name: 'All Files', extensions: ['*'] },
      ],
      defaultPath: 'output.txt',
    });
    return result.canceled ? null : result.filePath;
  });

  ipcMain.handle(
    IPC.TEXT_OUTPUT_PREVIEW,
    async (_e, { graph, imagePaths, nodeId }: { graph: NodeGraph; imagePaths: string[]; nodeId: string }) => {
      return computeTextOutputLines(graph, imagePaths, nodeId, registry);
    }
  );
}
