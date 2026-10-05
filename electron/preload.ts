import { ipcRenderer, contextBridge, webUtils } from 'electron';
import { createListenerTracker, type AnyFn } from './ipcListenerTracker.js';
import { IPC } from '../src/shared/constants.js';

// Track wrapper functions so off() can remove the right reference from ipcRenderer.
const tracker = createListenerTracker();

// Allowlist of valid IPC channels. Even though the renderer is our own code, a
// future XSS would otherwise be able to invoke *any* channel a main handler
// registers. Restricting to known channels narrows that blast radius.
const ALLOWED_CHANNELS = new Set<string>(Object.values(IPC));

function assertAllowed(channel: string): void {
  if (!ALLOWED_CHANNELS.has(channel)) {
    throw new Error(`Blocked IPC on unknown channel: ${channel}`);
  }
}

// --------- Expose some API to the Renderer process ---------
contextBridge.exposeInMainWorld('ipcRenderer', {
  on(channel: string, listener: AnyFn) {
    assertAllowed(channel);
    const wrapper = tracker.wrap(channel, listener);
    return ipcRenderer.on(channel, wrapper);
  },
  off(channel: string, listener: AnyFn) {
    assertAllowed(channel);
    const wrapper = tracker.getWrapper(channel, listener);
    if (wrapper) {
      tracker.remove(channel, listener);
      return ipcRenderer.off(channel, wrapper);
    }
    return ipcRenderer.off(channel, listener);
  },
  send(...args: Parameters<typeof ipcRenderer.send>) {
    const [channel, ...omit] = args;
    assertAllowed(channel);
    return ipcRenderer.send(channel, ...omit);
  },
  invoke(...args: Parameters<typeof ipcRenderer.invoke>) {
    const [channel, ...omit] = args;
    assertAllowed(channel);
    return ipcRenderer.invoke(channel, ...omit);
  },
});

// Electron 32+ removed the non-standard File.path property; the filesystem path
// of a dropped file must now be resolved through webUtils.getPathForFile.
contextBridge.exposeInMainWorld('webUtils', {
  getPathForFile(file: File) {
    return webUtils.getPathForFile(file);
  },
});
