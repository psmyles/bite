import { ipcMain, dialog } from 'electron';
import type { BrowserWindow } from 'electron';

import { IPC } from '../../shared/constants.js';

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
}
