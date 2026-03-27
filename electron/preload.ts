import { ipcRenderer, contextBridge } from 'electron'
import { createListenerTracker, type AnyFn } from './ipcListenerTracker.js'

// Track wrapper functions so off() can remove the right reference from ipcRenderer.
const tracker = createListenerTracker()

// --------- Expose some API to the Renderer process ---------
contextBridge.exposeInMainWorld('ipcRenderer', {
  on(channel: string, listener: AnyFn) {
    const wrapper = tracker.wrap(channel, listener)
    return ipcRenderer.on(channel, wrapper)
  },
  off(channel: string, listener: AnyFn) {
    const wrapper = tracker.getWrapper(channel, listener)
    if (wrapper) {
      tracker.remove(channel, listener)
      return ipcRenderer.off(channel, wrapper)
    }
    return ipcRenderer.off(channel, listener)
  },
  send(...args: Parameters<typeof ipcRenderer.send>) {
    const [channel, ...omit] = args
    return ipcRenderer.send(channel, ...omit)
  },
  invoke(...args: Parameters<typeof ipcRenderer.invoke>) {
    const [channel, ...omit] = args
    return ipcRenderer.invoke(channel, ...omit)
  },
})
