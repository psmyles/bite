/**
 * Pure utility for tracking ipcRenderer listener wrappers.
 *
 * Electron's contextBridge requires wrapping renderer-side listeners in new
 * functions before passing them to ipcRenderer.on(). This means the function
 * reference registered with ipcRenderer differs from the one the renderer
 * passes to off(). Without tracking, off() silently fails and listeners leak.
 *
 * Usage:
 *   const tracker = createListenerTracker()
 *   const wrapper = tracker.wrap(channel, listener)   // call ipcRenderer.on with wrapper
 *   const wrapper = tracker.getWrapper(channel, listener) // call ipcRenderer.off with wrapper
 *   tracker.remove(channel, listener)                 // clean up after off()
 */

export type AnyFn = (...args: unknown[]) => void

export function createListenerTracker() {
  // outer key: original listener; inner key: channel
  const map = new Map<AnyFn, Map<string, AnyFn>>()

  return {
    /** Create and store a wrapper for (channel, listener). Returns the wrapper. */
    wrap(channel: string, listener: AnyFn): AnyFn {
      const wrapper: AnyFn = (...args) => listener(...args)
      if (!map.has(listener)) map.set(listener, new Map())
      map.get(listener)!.set(channel, wrapper)
      return wrapper
    },

    /** Return the stored wrapper for (channel, listener), or undefined if not tracked. */
    getWrapper(channel: string, listener: AnyFn): AnyFn | undefined {
      return map.get(listener)?.get(channel)
    },

    /** Remove the stored wrapper after a successful off(). */
    remove(channel: string, listener: AnyFn): void {
      const byChannel = map.get(listener)
      if (!byChannel) return
      byChannel.delete(channel)
      if (byChannel.size === 0) map.delete(listener)
    },
  }
}
