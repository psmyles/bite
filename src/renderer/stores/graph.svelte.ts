import type { Node, Edge, Viewport } from '@xyflow/svelte'
import { getNodeParams, buildResizeParamDefs, type ParamPortDef } from '../nodeEditor/nodeEditorHelpers.js'
import { paramInHandle, paramOutHandle } from '../nodeEditor/wireTypeUtils.js'

/** Stable fingerprint for dirty-checking: strips SvelteFlow runtime fields */
function graphFingerprint(nodes: Node[], edges: Edge[]): string {
  return JSON.stringify({
    nodes: nodes.map((n) => ({ id: n.id, type: n.type, position: n.position, width: n.width ?? null, height: n.height ?? null, parentId: n.parentId ?? null, extent: n.extent ?? null, data: n.data })),
    edges: edges.map((e) => ({
      id: e.id,
      source: e.source,
      sourceHandle: e.sourceHandle ?? null,
      target: e.target,
      targetHandle: e.targetHandle ?? null,
    })),
  })
}

const SEED_NODES: Node[] = [
  {
    id: 'workflow-input',
    type: 'inputNode',
    position: { x: 80, y: 180 },
    deletable: false,
    data: { label: 'Input', inputs: [], outputs: ['image'] },
  },
  {
    id: 'workflow-output',
    type: 'outputNode',
    position: { x: 640, y: 180 },
    deletable: false,
    data: {
      label: 'Output',
      inputs: ['image'],
      outputs: [],
      params: {
        outputPath: 'source', customPath: '', overwrite: 'skip',
        outputMode: 'image',
        portIds: ['txo-0'], nextPortIndex: 1,
        separatorType: 'comma', customSeparator: '',
      },
    },
  },
]

class GraphStore {
  nodes = $state<Node[]>([])
  edges = $state<Edge[]>([])
  selectedNodeId = $state<string | null>(null)
  /** User's explicit preview node selection (null = auto-detect last node in chain) */
  previewNodeId = $state<string | null>(null)
  /** Actual node currently being previewed — set by Preview.svelte */
  activePreviewNodeId = $state<string | null>(null)
  /** Live computed output values for Properties nodes — keyed nodeId → { paramName: value }.
   *  Stored separately so updates don't affect graphKey and cause preview re-runs. */
  propValues = $state<Record<string, Record<string, unknown>>>({})

  /** Batch execution state — persisted here so it survives Inspector remounts. */
  batchRunning   = $state(false)
  batchProgress  = $state<{ completed: number; total: number; currentFile: string } | null>(null)
  batchError     = $state<string | null>(null)
  batchDone      = $state(false)
  batchStartTime = $state<number | null>(null)   // performance.now() when run started
  batchElapsedMs = $state<number | null>(null)   // final duration (ms), set on completion
  batchSummary     = $state<{ processed: number; skipped: number; failed: number; outputDir: string | null } | null>(null)
  batchSummaryOpen = $state(false)

  /** Current open file path (null = unsaved) */
  currentFilePath = $state<string | null>(null)
  /** Viewport synced from NodeEditor for saving; set pendingViewport to restore on load */
  viewport = $state<Viewport>({ x: 0, y: 0, zoom: 1 })
  /** Set to a Viewport to trigger NodeEditor to call setViewport() */
  pendingViewport = $state<Viewport | null>(null)

  /** Fingerprint of last saved/loaded state for dirty detection */
  _savedJson = $state<string | null>(null)
  _cleanInitialized = $state(false)

  isDirty = $derived(
    this._cleanInitialized &&
    graphFingerprint(this.nodes, this.edges) !== this._savedJson
  )

  get selectedNode(): Node | null {
    return this.nodes.find((n) => n.id === this.selectedNodeId) ?? null
  }

  /** Call once after the seed nodes are first pushed to the store. No-op thereafter. */
  initClean(): void {
    if (this._cleanInitialized) return
    this._savedJson = graphFingerprint(this.nodes, this.edges)
    this._cleanInitialized = true
  }

  /** Mark current state as saved at the given file path. */
  markClean(filePath: string | null): void {
    this._savedJson = graphFingerprint(this.nodes, this.edges)
    this._cleanInitialized = true
    this.currentFilePath = filePath
  }

  /** Reset graph to the initial seed state (Input + Output nodes, no edges). */
  resetToSeed(): void {
    this.nodes = SEED_NODES.map((n) => ({ ...n }))
    this.edges = []
    this.selectedNodeId = null
    this.previewNodeId = null
    this.propValues = {}
    this.pendingViewport = { x: 0, y: 0, zoom: 1 }
    this.markClean(null)
  }

  setPropValues(nodeId: string, values: Record<string, unknown>): void {
    this.propValues = { ...this.propValues, [nodeId]: values }
  }

  setParam(nodeId: string, paramName: string, value: unknown): void {
    this.nodes = this.nodes.map((n) => {
      if (n.id !== nodeId) return n
      const data = n.data as Record<string, unknown>
      const extraData: Record<string, unknown> = {}

      // When 'channels' changes on channel_merge, sync the visible image input ports.
      if (paramName === 'channels' && data.definitionId === 'channel_merge') {
        const count = Math.min(4, Math.max(2, Number(value) || 3))
        extraData.inputs      = ['image', 'image', 'image', 'image'].slice(0, count)
        extraData.inputLabels = ['R', 'G', 'B', 'A'].slice(0, count)
      }

      return {
        ...n,
        data: {
          ...data,
          ...extraData,
          params: {
            ...getNodeParams(data),
            [paramName]: value,
          },
        },
      }
    })
  }

  /**
   * Recompute paramDefs for a resize node based on its current mode + preserve_aspect,
   * then remove any edges connected to ports that no longer exist.
   */
  updateResizeParamDefs(nodeId: string, mode: string, preserve: boolean): void {
    const newDefs = buildResizeParamDefs(mode, preserve)
    const validHandles = new Set(newDefs.map((p: ParamPortDef) =>
      p.readonly ? paramOutHandle(p.name) : paramInHandle(p.name)
    ))
    this.edges = this.edges.filter((e) => {
      if (e.source === nodeId && e.sourceHandle?.startsWith('param-')) return validHandles.has(e.sourceHandle)
      if (e.target === nodeId && e.targetHandle?.startsWith('param-')) return validHandles.has(e.targetHandle)
      return true
    })
    this.nodes = this.nodes.map((n) => {
      if (n.id !== nodeId) return n
      return { ...n, data: { ...(n.data as Record<string, unknown>), paramDefs: newDefs } }
    })
  }
}

export const graphStore = new GraphStore()
