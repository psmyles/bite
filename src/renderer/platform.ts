// True when running inside Electron — preload.ts installs window.ipcRenderer synchronously
// via contextBridge before the renderer starts.  False in a plain browser (GitHub Pages, etc.).
export const IS_ELECTRON: boolean =
  typeof window !== 'undefined' &&
  Object.prototype.hasOwnProperty.call(window, 'ipcRenderer')
