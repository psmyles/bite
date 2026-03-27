// Browser shim for window.ipcRenderer.
// Installed before the Svelte app mounts when running outside Electron so all
// existing invoke/on/off call-sites work unchanged.  Only the channels needed
// by the workflow builder are implemented; image-processing channels return
// safe empty values and are never actually reached (Preview / batch UI is
// hidden via IS_ELECTRON guards).

import type { NodeDefinition } from '../shared/types.js'
import { IPC } from '../shared/constants.js'

// Bundle every node-definition JSON at build time (Vite static glob import).
// The path is relative to this file: src/renderer/ → ../../node-definitions/
const NODE_DEFS: NodeDefinition[] = Object.values(
  import.meta.glob('../../node-definitions/*.json', { eager: true })
) as NodeDefinition[]

type Listener = (...args: unknown[]) => void

export function installBrowserIpc(): void {
  // Simple listener registry — on/off are no-ops for OS-menu events that
  // don't exist in a browser, but we keep a real map so components that
  // call on() + off() symmetrically don't throw.
  const listenerMap = new Map<string, Set<Listener>>()

  const ipc = {
    async invoke(channel: string, ...args: unknown[]): Promise<unknown> {
      switch (channel) {

        // ── Node definitions ────────────────────────────────────────────────
        case IPC.REGISTRY_GET_ALL:
          return NODE_DEFS

        // ── Workflow save — triggers a browser file download ─────────────────
        case IPC.WORKFLOW_SAVE: {
          const [graph, filePath] = args as [unknown, string | null]
          const fileName = filePath
            ? (filePath.split(/[\\/]/).pop() ?? 'workflow.imgplex')
            : 'workflow.imgplex'
          const content = JSON.stringify({ version: '1.0', graph }, null, 2)
          const blob = new Blob([content], { type: 'application/json' })
          const url  = URL.createObjectURL(blob)
          const a    = document.createElement('a')
          a.href     = url
          a.download = fileName
          document.body.appendChild(a)
          a.click()
          document.body.removeChild(a)
          setTimeout(() => URL.revokeObjectURL(url), 1000)
          return fileName   // used by graphStore.markClean()
        }

        // ── Workflow load — opens a file picker ──────────────────────────────
        case IPC.WORKFLOW_LOAD:
          return new Promise<unknown>((resolve) => {
            const input = document.createElement('input')
            input.type   = 'file'
            input.accept = '.imgplex,application/json'
            input.addEventListener('change', async () => {
              const file = input.files?.[0]
              if (!file) { resolve(null); return }
              try {
                const text = await file.text()
                const data = JSON.parse(text) as Record<string, unknown>
                if (!data.graph) {
                  alert('Invalid workflow file: missing graph data.')
                  resolve(null)
                  return
                }
                resolve({ graph: data.graph, filePath: file.name })
              } catch {
                alert('Failed to parse workflow file.')
                resolve(null)
              }
            })
            input.addEventListener('cancel', () => resolve(null))
            input.click()
          })

        // ── Image / processing channels — return safe empty values ───────────
        // These are only called when IS_ELECTRON guards are absent; they return
        // values that cause the calling code to short-circuit gracefully.
        case IPC.OPEN_IMAGES_DIALOG:
        case IPC.SCAN_FOLDER_DIALOG:
        case IPC.LOAD_IMAGES:
        case IPC.LOAD_IMAGES_STREAMING_START:
          return []

        case IPC.LOAD_IMAGES_STREAMING_CANCEL:
          return undefined

        case IPC.OPEN_FOLDER_DIALOG:
          return null

        // ── App lifecycle ────────────────────────────────────────────────────
        case IPC.APP_QUIT:
          // Can't close a browser tab programmatically; just no-op.
          return undefined

        default:
          return null
      }
    },

    on(channel: string, listener: Listener): void {
      if (!listenerMap.has(channel)) listenerMap.set(channel, new Set())
      listenerMap.get(channel)!.add(listener)
    },

    off(channel: string, listener: Listener): void {
      listenerMap.get(channel)?.delete(listener)
    },

    send(_channel: string, ..._args: unknown[]): void {
      // No-op — no main process to message.
    },
  }

  ;(window as unknown as Record<string, unknown>).ipcRenderer = ipc
}
