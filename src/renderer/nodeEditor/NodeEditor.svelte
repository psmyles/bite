<script lang="ts">
  import {
    SvelteFlow,
    Controls,
    Background,
    BackgroundVariant,
    MiniMap,
    addEdge,
    type Node,
    type Edge,
    type Connection,
    type Viewport,
  } from '@xyflow/svelte'
  import { untrack, tick } from 'svelte'
  import '@xyflow/svelte/dist/style.css'
  import DropHelper from './DropHelper.svelte'
  import ProcessNode from './ProcessNode.svelte'
  import InputNode from './InputNode.svelte'
  import OutputNode from './OutputNode.svelte'
  import CommentNode from './CommentNode.svelte'
  import GroupNode from './GroupNode.svelte'
  import FolderPathNode from './FolderPathNode.svelte'
  import CompareNode from './CompareNode.svelte'
  import AtlasNode from './AtlasNode.svelte'
  import ColoredEdge from './ColoredEdge.svelte'
  import NodeContextMenu from './NodeContextMenu.svelte'
  import { portColor } from './portColors.js'
  import { isNodeEffectivelyEnabled } from './nodeEnabledState.js'
  import { nodeTypeForDef, buildNodeData, firstMatchingHandle } from './nodeEditorHelpers.js'
  import { numericWireTypes, scalarTypes, wireTypesCompatible, paramTypeToWireType, paramInHandle } from './wireTypeUtils.js'
  import type { NodeDefinition } from '../../shared/types.js'
  import { graphStore } from '../stores/graph.svelte.js'

  let { definitions }: { definitions: NodeDefinition[] } = $props()

  // ── Special node IDs (permanent, undeletable) ─────────────────────────────
  export const INPUT_NODE_ID  = 'workflow-input'
  export const OUTPUT_NODE_ID = 'workflow-output'

  // ── Custom node / edge types ───────────────────────────────────────────────
  const nodeTypes = { process: ProcessNode, inputNode: InputNode, outputNode: OutputNode, commentNode: CommentNode, group: GroupNode, folderPathNode: FolderPathNode, compareNode: CompareNode, atlasNode: AtlasNode }
  const edgeTypes = { colored: ColoredEdge }

  // ── Helpers ───────────────────────────────────────────────────────────────
  const NODE_W = 150
  const NODE_H = 58 // header (~32px) + ports row (~26px)

  /** Infer wire type from the handle that started the drag. */
  function handleToWireType(nodeId: string, handleId: string | null, handleType: string | null): string {
    // Named special handles
    if (handleId === 'folder-in') return 'path'
    // Text output ports
    if (handleId === 'txo-condition') return 'boolean'
    if (handleId?.startsWith('txo-')) return 'any'

    const src = nodes.find((n) => n.id === nodeId)
    const nodeData = src?.data as Record<string, unknown> | undefined
    if (handleId?.startsWith('param-')) {
      const paramName = handleId.replace(/^param-(in|out)-/, '')
      if (paramName === '_enabled') return 'boolean'
      const pd = (nodeData?.paramDefs as Array<{ name: string; type: string }> | undefined)
        ?.find((p) => p.name === paramName)
      if (!pd) return 'number'
      return paramTypeToWireType(pd.type)
    }
    return handleType === 'source'
      ? ((nodeData?.outputs as string[] | undefined)?.[0] ?? 'image')
      : ((nodeData?.inputs  as string[] | undefined)?.[0] ?? 'image')
  }

  /** BFS from `startId` following edge sources; returns true if `goalId` is reachable. */
  function canReach(startId: string, goalId: string): boolean {
    const visited = new Set<string>()
    const queue = [startId]
    while (queue.length > 0) {
      const cur = queue.shift()!
      if (cur === goalId) return true
      if (visited.has(cur)) continue
      visited.add(cur)
      for (const e of edges) {
        if (e.source === cur && !visited.has(e.target)) queue.push(e.target)
      }
    }
    return false
  }

  /**
   * When a port is typed 'any', look at sibling 'any' input params that are already
   * wired to determine the effective constrained wire type. Used to correctly filter
   * the context menu when wire-dropping from an 'any' port that has constraints.
   */
  function resolveEffectiveWireType(nodeId: string, handleId: string | null): string {
    const raw = handleToWireType(nodeId, handleId, null)
    if (raw !== 'any') return raw
    const nodeData = (nodes.find(n => n.id === nodeId)?.data) as Record<string, unknown> | undefined
    const paramDefs = nodeData?.paramDefs as Array<{name: string; type: string; readonly?: boolean}> | undefined
    if (!paramDefs) return 'any'
    for (const p of paramDefs.filter(pd => pd.type === 'any' && !pd.readonly)) {
      const edge = edges.find(e => e.target === nodeId && e.targetHandle === paramInHandle(p.name))
      if (!edge) continue
      const t = handleToWireType(edge.source, edge.sourceHandle ?? null, 'source')
      if (t !== 'any') return t
    }
    return 'any'
  }

  type AnyParamDef = { name: string; type: string; readonly?: boolean }

  /** channel_merge special case: scalar numeric wires can drive image inputs as gray fill 0–1. */
  function isChannelMergeScalarInput(conn: Connection, srcType: string, tgtType: string): boolean {
    return tgtType === 'image' &&
      scalarTypes.has(srcType) &&
      !!conn.targetHandle?.startsWith('in-') &&
      (nodes.find(n => n.id === conn.target)?.data as Record<string, unknown>)?.definitionId === 'channel_merge'
  }

  /** All other 'any' inputs on the target node must carry a type compatible with srcType. */
  function siblingAnyInputsCompatible(conn: Connection, srcType: string): boolean {
    const paramDefs = (nodes.find(n => n.id === conn.target)?.data as Record<string, unknown>)
      ?.paramDefs as AnyParamDef[] | undefined
    const siblings = paramDefs?.filter(p => p.type === 'any' && !p.readonly && `param-in-${p.name}` !== conn.targetHandle)
    for (const sib of siblings ?? []) {
      const sibEdge = edges.find(e => e.target === conn.target && e.targetHandle === `param-in-${sib.name}`)
      if (!sibEdge) continue
      if (!wireTypesCompatible(srcType, handleToWireType(sibEdge.source, sibEdge.sourceHandle ?? null, 'source'))) return false
    }
    return true
  }

  /**
   * When an 'any' input gets a concrete type, the node's 'any' outputs are constrained.
   * Verify all existing edges from those outputs remain compatible with resolvedSrc.
   * Example: Branch Result→folder already wired; connecting image→If True must be rejected.
   */
  function downstreamAnyOutputsCompatible(conn: Connection, resolvedSrc: string): boolean {
    if (resolvedSrc === 'any') return true
    const paramDefs = (nodes.find(n => n.id === conn.target)?.data as Record<string, unknown>)
      ?.paramDefs as AnyParamDef[] | undefined
    for (const p of paramDefs?.filter(p => p.type === 'any' && p.readonly) ?? []) {
      for (const outEdge of edges.filter(e => e.source === conn.target && e.sourceHandle === `param-out-${p.name}`)) {
        if (!wireTypesCompatible(resolvedSrc, handleToWireType(outEdge.target, outEdge.targetHandle ?? null, 'target'))) return false
      }
    }
    return true
  }

  /** Returns true only when the connection is type-compatible, non-self, and cycle-free. */
  function isValidConnection(connection: Connection): boolean {
    if (connection.source === connection.target) return false
    if (canReach(connection.target, connection.source)) return false

    const srcType = handleToWireType(connection.source, connection.sourceHandle ?? null, 'source')
    const tgtType = handleToWireType(connection.target, connection.targetHandle ?? null, 'target')
    const resolvedSrc = srcType === 'any' ? resolveEffectiveWireType(connection.source, connection.sourceHandle ?? null) : srcType
    const resolvedTgt = tgtType === 'any' ? resolveEffectiveWireType(connection.target, connection.targetHandle ?? null) : tgtType

    if (!isChannelMergeScalarInput(connection, srcType, tgtType) && !wireTypesCompatible(resolvedSrc, resolvedTgt)) return false
    if (tgtType === 'any' && !siblingAnyInputsCompatible(connection, srcType)) return false
    if (tgtType === 'any' && !downstreamAnyOutputsCompatible(connection, resolvedSrc)) return false

    return true
  }

  function edgeStyle(type: string) {
    const c = portColor(type)
    return {
      type: 'colored',
      style: `stroke: ${c}; stroke-width: 2`,
    }
  }

  // ── Graph state — local, synced bidirectionally with graphStore ───────────
  let nodes: Node[] = $state.raw([
    {
      id: INPUT_NODE_ID,
      type: 'inputNode',
      position: { x: 80, y: 180 },
      deletable: false,
      data: { label: 'Input', inputs: [], outputs: ['image'] },
    },
    {
      id: OUTPUT_NODE_ID,
      type: 'outputNode',
      position: { x: 640, y: 180 },
      deletable: false,
      data: {
        label: 'Output',
        inputs: ['image'],
        outputs: [],
        params: { outputPath: 'source', customPath: '', overwrite: 'skip' },
      },
    },
  ])
  let edges: Edge[] = $state.raw([])

  // Push local → store (SvelteFlow mutations: drag, delete, etc.)
  $effect(() => {
    const n = nodes
    untrack(() => {
      if (n !== graphStore.nodes) graphStore.nodes = n
      graphStore.initClean()
    })
  })
  $effect(() => {
    const e = edges
    untrack(() => { if (e !== graphStore.edges) graphStore.edges = e })
  })

  // Pull store → local (Inspector param changes create a new array)
  $effect(() => {
    const n = graphStore.nodes
    untrack(() => { if (n !== nodes) nodes = n })
  })
  $effect(() => {
    const e = graphStore.edges
    untrack(() => { if (e !== edges) edges = e })
  })

  // ── Text output node: keep portIds in sync with actual edge connections ──────
  // Runs whenever edges change. Removes middle unconnected ports and ensures
  // exactly one unconnected ghost port exists at the bottom of each text_output node.
  $effect(() => {
    const currentEdges = edges   // reactive dep — re-runs on any edge change
    untrack(() => {
      const txNodes = nodes.filter((n) =>
        n.id === 'workflow-output' &&
        (n.data as Record<string, unknown>).params?.outputMode === 'text'
      )
      if (txNodes.length === 0) return

      let changed = false
      const updated = nodes.map((n) => {
        if (!(n.id === 'workflow-output' && (n.data as Record<string, unknown>).params?.outputMode === 'text')) return n
        const p = (n.data.params ?? {}) as Record<string, unknown>
        const portIds = [...((p.portIds as string[]) ?? ['txo-0'])]
        let nextPortIndex = (p.nextPortIndex as number) ?? 1

        const connected = new Set(
          currentEdges
            .filter((e) => e.target === n.id && e.targetHandle?.startsWith('txo-'))
            .map((e) => e.targetHandle as string)
        )

        // Keep connected ports + always keep the last (ghost) port
        const filtered = portIds.filter((pid, idx) => idx === portIds.length - 1 || connected.has(pid))

        // If the ghost port itself got a wire, add a new ghost port
        if (filtered.length > 0 && connected.has(filtered[filtered.length - 1])) {
          filtered.push(`txo-${nextPortIndex}`)
          nextPortIndex++
        }

        if (JSON.stringify(filtered) === JSON.stringify(portIds) &&
            nextPortIndex === (p.nextPortIndex as number)) return n

        changed = true
        return { ...n, data: { ...n.data, params: { ...p, portIds: filtered, nextPortIndex } } }
      })

      if (changed) nodes = updated
    })
  })

  function onselectionchange({ nodes: sel }: { nodes: Node[] }) {
    graphStore.selectedNodeId = sel[0]?.id ?? null
  }

  // ── Undo / Redo ───────────────────────────────────────────────────────────
  const MAX_HISTORY = 100
  const history: { nodes: Node[]; edges: Edge[] }[] = []
  let historyIdx = -1

  function snapshot() {
    return {
      nodes: JSON.parse(JSON.stringify(nodes)) as Node[],
      edges: JSON.parse(JSON.stringify(edges)) as Edge[],
    }
  }

  function pushHistory() {
    history.splice(historyIdx + 1)          // discard any redo future
    history.push(snapshot())
    if (history.length > MAX_HISTORY) history.shift()
    historyIdx = history.length - 1
  }

  // Deferred variant: batches rapid back-to-back calls (e.g. node+edge deleted
  // in the same tick) into a single history entry.
  let _pendingPush = false
  function scheduleHistoryPush() {
    if (_pendingPush) return
    _pendingPush = true
    Promise.resolve().then(() => { _pendingPush = false; pushHistory() })
  }

  function undo() {
    if (historyIdx <= 0) return
    historyIdx--
    nodes = history[historyIdx].nodes
    edges = history[historyIdx].edges
  }

  function redo() {
    if (historyIdx >= history.length - 1) return
    historyIdx++
    nodes = history[historyIdx].nodes
    edges = history[historyIdx].edges
  }

  // Capture initial empty-canvas state so the first undo returns to blank.
  pushHistory()

  // ── Node grouping ──────────────────────────────────────────────────────────
  const GROUP_PADDING = 40

  const groupable = $derived(
    nodes.some(n => n.selected && n.id !== INPUT_NODE_ID && n.id !== OUTPUT_NODE_ID && n.type !== 'group')
  )
  const ungroupable = $derived(
    nodes.some(n => n.selected && n.type === 'group')
  )

  function groupSelection() {
    const toGroup = nodes.filter(
      n => n.selected && n.id !== INPUT_NODE_ID && n.id !== OUTPUT_NODE_ID && n.type !== 'group'
    )
    if (toGroup.length === 0) return

    // Bounding box around all selected nodes
    let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity
    for (const n of toGroup) {
      const w = n.width  ?? NODE_W
      const h = n.height ?? NODE_H
      minX = Math.min(minX, n.position.x)
      minY = Math.min(minY, n.position.y)
      maxX = Math.max(maxX, n.position.x + w)
      maxY = Math.max(maxY, n.position.y + h)
    }

    const groupX = minX - GROUP_PADDING
    const groupY = minY - GROUP_PADDING
    const groupW = (maxX - minX) + 2 * GROUP_PADDING
    const groupH = (maxY - minY) + 2 * GROUP_PADDING

    const groupId = `group-${Date.now()}`
    const groupNode: Node = {
      id: groupId,
      type: 'group',
      position: { x: groupX, y: groupY },
      width: groupW,
      height: groupH,
      zIndex: 0,
      data: { label: 'Group', definitionId: '', params: {} } as Record<string, unknown>,
    }

    const childIds = new Set(toGroup.map(n => n.id))
    const nonChildren: Node[] = []
    const children: Node[] = []

    for (const n of nodes) {
      if (childIds.has(n.id)) {
        children.push({
          ...n,
          position: { x: n.position.x - groupX, y: n.position.y - groupY },
          parentId: groupId,
          extent: 'parent' as const,
          selected: false,
        })
      } else {
        nonChildren.push({ ...n, selected: false })
      }
    }

    // Group node must appear before its children in the array
    nodes = [groupNode, ...nonChildren, ...children]
    pushHistory()
  }

  function ungroupSelection() {
    const selectedGroups = nodes.filter(n => n.selected && n.type === 'group')
    if (selectedGroups.length === 0) return

    const groupIds = new Set(selectedGroups.map(g => g.id))
    const groupMap = new Map(selectedGroups.map(g => [g.id, g]))

    const updatedNodes = nodes
      .filter(n => !groupIds.has(n.id))
      .map(n => {
        if (!n.parentId || !groupIds.has(n.parentId)) return n
        const parent = groupMap.get(n.parentId)!
        return {
          ...n,
          position: { x: n.position.x + parent.position.x, y: n.position.y + parent.position.y },
          parentId: undefined,
          extent: undefined,
        }
      })

    nodes = updatedNodes
    pushHistory()
  }

  // ── Editor element ref + last-mouse tracking (for keyboard invocation) ──────
  let editorEl = $state<HTMLElement | undefined>(undefined)
  let lastMouseX = 0
  let lastMouseY = 0
  function onMouseMove(e: MouseEvent) { lastMouseX = e.clientX; lastMouseY = e.clientY }

  // ── Context menu ──────────────────────────────────────────────────────────
  interface WireLine { x1: number; y1: number; x2: number; y2: number; color: string }
  interface MenuState {
    x: number
    y: number
    canvasPos: { x: number; y: number }
    filterType: string | null
    wireLine: WireLine | null   // set when menu was triggered by a wire drop
  }
  let menuState: MenuState | null = $state(null)

  function openMenu(screenX: number, screenY: number, filterType: string | null = null) {
    if (!screenToCanvas) return

    // Compute wire preview line endpoints (source handle → menu position).
    // Query the actual rendered handle element so the start point is exact regardless
    // of node height, which port was dragged, zoom level, or viewport offset.
    let wireLine: WireLine | null = null
    if (wireSource && editorEl) {
      const handleEl = editorEl.querySelector(
        `.svelte-flow__node[data-id="${wireSource.nodeId}"] [data-handleid="${wireSource.handleId}"]`
      ) as HTMLElement | null
      if (handleEl) {
        const hr = handleEl.getBoundingClientRect()
        wireLine = {
          x1: hr.left + hr.width  / 2,
          y1: hr.top  + hr.height / 2,
          x2: screenX,
          y2: screenY,
          color: portColor(wireType ?? 'image'),
        }
      }
    }

    menuState = {
      x: screenX,
      y: screenY,
      canvasPos: screenToCanvas({ x: screenX, y: screenY }),
      filterType,
      wireLine,
    }
  }

  // Draw/remove a bezier wire preview on document.body while menu is open from wire-drop
  $effect(() => {
    const line = menuState?.wireLine
    if (!line) return
    const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg') as SVGSVGElement
    svg.style.cssText = 'position:fixed;inset:0;width:100%;height:100%;pointer-events:none;z-index:998;overflow:visible'
    const cp = Math.max(Math.abs(line.x2 - line.x1) * 0.4, 50)
    const path = document.createElementNS('http://www.w3.org/2000/svg', 'path') as SVGPathElement
    path.setAttribute('d', `M ${line.x1} ${line.y1} C ${line.x1+cp} ${line.y1} ${line.x2-cp} ${line.y2} ${line.x2} ${line.y2}`)
    path.setAttribute('stroke', line.color)
    path.setAttribute('stroke-width', '2')
    path.setAttribute('stroke-dasharray', '6 4')
    path.setAttribute('fill', 'none')
    path.setAttribute('opacity', '0.8')
    svg.appendChild(path)
    document.body.appendChild(svg)
    return () => svg.remove()
  })

  function closeMenu() {
    menuState = null
    wireSource = null
    wireType = null
    wireMade = false
  }

  function onMenuSelect(def: NodeDefinition) {
    if (!menuState) return
    const pos = menuState.canvasPos
    const newId = `${def.id}-${Date.now()}`
    // Wire-drop: top-left of new node lands on the drop point so the input handle
    // is right where the wire ended. Otherwise center the node on the spawn point.
    const position = wireSource
      ? { x: pos.x, y: pos.y }
      : { x: pos.x - NODE_W / 2, y: pos.y - NODE_H / 2 }

    const isComment = def.id === 'comment'
    nodes = [...nodes, {
      id: newId,
      type: nodeTypeForDef(def),
      position,
      data: buildNodeData(def),
      ...(isComment ? { width: 280, height: 120 } : {}),
    }]

    // If menu was triggered by a wire drop, auto-connect the edge
    if (wireSource) {
      const wt = wireType ?? 'image'
      const newSide = wireSource.handleType === 'source' ? 'in' : 'out'
      const newHandle = firstMatchingHandle(def, wt, newSide)
      if (newHandle) {
        const connection = wireSource.handleType === 'source'
          ? { source: wireSource.nodeId, sourceHandle: wireSource.handleId, target: newId, targetHandle: newHandle }
          : { source: newId, sourceHandle: newHandle, target: wireSource.nodeId, targetHandle: wireSource.handleId }
        if (isValidConnection(connection)) {
          edges = addEdge({ ...connection, ...edgeStyle(wt) }, edges)
        }
      }
    }
    pushHistory()
  }

  // ── Connections ───────────────────────────────────────────────────────────
  function onConnect(connection: Connection) {
    wireMade = true

    const type = handleToWireType(connection.source, connection.sourceHandle ?? null, 'source')
    const style = edgeStyle(type)

    // Remove any existing edge going into the same target handle (single-input rule).
    // Also drop any auto-added plain edge SvelteFlow may have inserted for this exact connection.
    edges = edges.filter(e =>
      !(e.target === connection.target && e.targetHandle === connection.targetHandle)
    )

    const exists = edges.some(
      e => e.source === connection.source &&
           e.target === connection.target &&
           e.sourceHandle === connection.sourceHandle &&
           e.targetHandle === connection.targetHandle
    )
    if (exists) {
      edges = edges.map(e =>
        e.source === connection.source &&
        e.target === connection.target &&
        e.sourceHandle === connection.sourceHandle &&
        e.targetHandle === connection.targetHandle
          ? { ...e, ...style }
          : e
      )
    } else {
      edges = addEdge({ ...connection, ...style }, edges)
    }
    pushHistory()
  }

  // ── Wire-drop → context menu (filtered by port type) ──────────────────────
  let wireSource: { nodeId: string; handleId: string | null; handleType: string | null } | null = null
  let wireType: string | null = null
  let wireMade = false

  function onConnectStart(
    _e: MouseEvent | TouchEvent,
    params: { nodeId: string | null; handleId: string | null; handleType: string | null }
  ) {
    if (params.nodeId) {
      wireSource = { nodeId: params.nodeId, handleId: params.handleId, handleType: params.handleType }
      wireType = handleToWireType(params.nodeId, params.handleId, params.handleType)
    }
    wireMade = false
  }

  function onConnectEnd(e: MouseEvent | TouchEvent) {
    if (!wireMade && wireSource && e instanceof MouseEvent) {
      const target = e.target as Element
      if (!target.closest('.svelte-flow__handle') && !target.closest('.svelte-flow__node')) {
        // Resolve a more specific type if the 'any' port has sibling constraints
        const effectiveWireType = wireType === 'any' && wireSource
          ? resolveEffectiveWireType(wireSource.nodeId, wireSource.handleId)
          : wireType
        openMenu(e.clientX, e.clientY, effectiveWireType)
        // Keep wireSource + wireType alive so onMenuSelect can create the edge
        return
      }
    }
    wireSource = null
    wireType = null
    wireMade = false
  }

  // ── Background pattern (read from theme.css CSS vars) ─────────────────────
  function cssProp(name: string): string {
    return getComputedStyle(document.documentElement).getPropertyValue(name).trim()
  }

  const bgVariantMap: Record<string, BackgroundVariant> = {
    dots:  BackgroundVariant.Dots,
    lines: BackgroundVariant.Lines,
    cross: BackgroundVariant.Cross,
  }

  // Strip surrounding quotes that CSS string values include e.g. `"dots"` → `dots`
  const bgVariant   = bgVariantMap[cssProp('--graph-bg-variant').replace(/['"]/g, '')] ?? BackgroundVariant.Dots
  const bgGap       = parseFloat(cssProp('--graph-bg-gap'))   || 20
  const bgLineWidth = parseFloat(cssProp('--graph-bg-line-width')) || 1
  const bgColor     = cssProp('--graph-bg-color')
  const bgBaseColor = cssProp('--graph-bg-base-color')

  // Connection line style — changes color while dragging a wire
  const connectionLineStyle = $derived(
    wireType
      ? `stroke: ${portColor(wireType)}; stroke-width: 2; stroke-dasharray: 6 4;`
      : `stroke: #6b7280; stroke-width: 2; stroke-dasharray: 6 4;`
  )

  // ── Viewport (zoom level) ──────────────────────────────────────────────────
  let viewport: Viewport = $state({ x: 0, y: 0, zoom: 1 })
  const zoomPct = $derived(Math.round(viewport.zoom * 100) + '%')

  // ── screenToFlowPosition + setViewport + updateNodeInternals (from DropHelper) ─
  let screenToCanvas: ((pos: { x: number; y: number }) => { x: number; y: number }) | null =
    $state(null)
  let setViewport: ((v: Viewport) => void) | null = $state(null)
  let updateNodeInternals: ((ids: string | string[]) => void) | null = $state(null)

  // Re-measure handle positions whenever the output node's port order changes (text mode).
  // @xyflow only remeasures handles on node resize; CSS top changes need an explicit nudge.
  $effect(() => {
    // Track portIds of workflow-output when in text mode as a reactive dependency
    const txoNodes = nodes.filter((n) =>
      n.id === 'workflow-output' &&
      (n.data as Record<string, unknown>).params?.outputMode === 'text'
    )
    txoNodes.forEach((n) => { void JSON.stringify((n.data?.params as Record<string, unknown>)?.portIds) })
    const ids = txoNodes.map((n) => n.id)
    tick().then(() => { if (updateNodeInternals && ids.length) updateNodeInternals(ids) })
  })

  // isNodeEffectivelyEnabled is imported from nodeEnabledState.js
  // Wrap to close over the local nodes/edges reactive arrays.
  function checkNodeEnabled(nodeId: string): boolean {
    return isNodeEffectivelyEnabled(nodeId, nodes, edges)
  }

  // ── Double-click: on node → set preview target; on canvas → reset zoom ──────
  function onDblClick(e: MouseEvent) {
    const nodeEl = (e.target as Element).closest('.svelte-flow__node')
    if (nodeEl) {
      const nodeId = nodeEl.getAttribute('data-id')
      const nodeType = nodes.find((n) => n.id === nodeId)?.type
      if (
        nodeId &&
        nodeId !== INPUT_NODE_ID &&
        nodeId !== OUTPUT_NODE_ID &&
        nodeType !== 'commentNode' &&
        checkNodeEnabled(nodeId)
      ) {
        // Toggle: double-click the current preview node again to revert to auto
        graphStore.previewNodeId = graphStore.previewNodeId === nodeId ? null : nodeId
      }
      return
    }
    setViewport?.({ x: viewport.x, y: viewport.y, zoom: 1 })
  }

  // ── Right-click on canvas → open context menu ─────────────────────────────
  function onContextMenu(e: MouseEvent) {
    e.preventDefault()
    const nodeEl = (e.target as Element).closest('.svelte-flow__node')
    if (nodeEl) {
      // Allow context menu on group nodes (for ungrouping)
      const nodeId = nodeEl.getAttribute('data-id')
      if (nodes.find(n => n.id === nodeId)?.type === 'group') openMenu(e.clientX, e.clientY)
      return
    }
    openMenu(e.clientX, e.clientY)
  }

  // ── Delete / drag-end — push history after xyflow mutates nodes/edges ──────
  function onNodeDragStop(_e: MouseEvent | TouchEvent, node: Node | undefined) {
    // Snap child nodes back below the header band if the user drags them into it
    if (node?.parentId) {
      const parent = nodes.find(n => n.id === node.parentId)
      if (parent?.type === 'group' && node.position.y < 0) {
        nodes = nodes.map(n =>
          n.id === node.id ? { ...n, position: { x: n.position.x, y: 0 } } : n
        )
      }
    }
    scheduleHistoryPush()
  }

  // ── Custom delete handler — xyflow's deleteKey is disabled so we own this ────
  // Deleting a group ungroupes its children (converts to absolute) rather than
  // removing them. We handle both nodes and edges here.
  function deleteSelected() {
    const active = document.activeElement
    if (active && (active.tagName === 'INPUT' || active.tagName === 'TEXTAREA' || (active as HTMLElement).isContentEditable)) return

    const selectedGroups = nodes.filter(n => n.selected && n.type === 'group')
    const selectedGroupIds = new Set(selectedGroups.map(g => g.id))
    const groupMap = new Map(selectedGroups.map(g => [g.id, g]))

    // Non-group nodes to delete: selected, not Input/Output, not children of a selected group
    const toDelete = new Set(
      nodes
        .filter(n => n.selected && n.type !== 'group' && n.id !== INPUT_NODE_ID && n.id !== OUTPUT_NODE_ID)
        .filter(n => !n.parentId || !selectedGroupIds.has(n.parentId))
        .map(n => n.id)
    )

    const selectedEdgeIds = new Set(edges.filter(e => e.selected).map(e => e.id))

    if (selectedGroupIds.size === 0 && toDelete.size === 0 && selectedEdgeIds.size === 0) return

    nodes = nodes
      .filter(n => !selectedGroupIds.has(n.id) && !toDelete.has(n.id))
      .map(n => {
        // Children of deleted groups get converted to absolute-positioned free nodes
        if (n.parentId && selectedGroupIds.has(n.parentId)) {
          const parent = groupMap.get(n.parentId)!
          return {
            ...n,
            position: { x: n.position.x + parent.position.x, y: n.position.y + parent.position.y },
            parentId: undefined,
            extent: undefined,
          }
        }
        return n
      })

    edges = edges.filter(e =>
      !selectedEdgeIds.has(e.id) &&
      !toDelete.has(e.source) &&
      !toDelete.has(e.target)
    )

    pushHistory()
  }

  // ── Space / Tab key while canvas is focused → open context menu ───────────
  function onKeydown(e: KeyboardEvent) {
    // Undo / Redo
    if ((e.key === 'z' || e.key === 'Z') && (e.ctrlKey || e.metaKey)) {
      e.preventDefault()
      e.shiftKey ? redo() : undo()
      return
    }
    if ((e.key === 'y' || e.key === 'Y') && (e.ctrlKey || e.metaKey)) {
      e.preventDefault()
      redo()
      return
    }

    // Ctrl+G — group selected nodes; Ctrl+Shift+G — ungroup
    if ((e.key === 'g' || e.key === 'G') && (e.ctrlKey || e.metaKey)) {
      e.preventDefault()
      e.shiftKey ? ungroupSelection() : groupSelection()
      return
    }

    if ((e.key === ' ' || e.key === 'Tab') && !menuState) {
      e.preventDefault()
      // Use the last known mouse position so the menu appears where the cursor is,
      // matching right-click and wire-drop behavior.
      // Fall back to canvas center if the mouse has never entered the canvas.
      if (lastMouseX || lastMouseY) {
        openMenu(lastMouseX, lastMouseY)
      } else {
        const rect = (e.currentTarget as HTMLElement).getBoundingClientRect()
        openMenu(rect.left + rect.width / 2, rect.top + rect.height / 2)
      }
    }

    // Delete / Backspace — custom handler so group deletion ungroupes children
    if (e.key === 'Delete' || e.key === 'Backspace') {
      deleteSelected()
      return
    }

    // Ctrl/Cmd+D — duplicate selected nodes (never duplicates Input/Output)
    if ((e.key === 'd' || e.key === 'D') && (e.ctrlKey || e.metaKey)) {
      e.preventDefault()
      duplicateNodes(nodes.filter(n => n.selected && n.id !== INPUT_NODE_ID && n.id !== OUTPUT_NODE_ID))
    }
  }

  function duplicateNodes(targets: Node[]) {
    if (targets.length === 0) return
    // Include children of any selected group nodes that aren't already in targets
    const selectedGroupIds = new Set(targets.filter(n => n.type === 'group').map(n => n.id))
    if (selectedGroupIds.size > 0) {
      const extraChildren = nodes.filter(
        n => n.parentId && selectedGroupIds.has(n.parentId) && !targets.some(t => t.id === n.id)
      )
      targets = [...targets, ...extraChildren]
    }
    const ts = Date.now()
    const idMap = new Map(targets.map((n, i) => [n.id, `${n.id}-dup${ts}${i}`]))
    const groups: Node[] = []
    const rest: Node[] = []
    for (const n of targets) {
      const dup: Node = {
        ...n,
        id: idMap.get(n.id)!,
        position: { x: n.position.x + 20, y: n.position.y + 20 },
        selected: true,
        data: { ...(n.data as object) },
        // Remap parentId to duplicated group if parent was also duplicated
        ...(n.parentId && idMap.has(n.parentId) ? { parentId: idMap.get(n.parentId) } : {}),
      }
      if (n.type === 'group') groups.push(dup)
      else rest.push(dup)
    }
    // Groups must precede their children
    nodes = [...nodes.map(n => ({ ...n, selected: false })), ...groups, ...rest]
    pushHistory()
  }

  // ── Drag-and-drop ─────────────────────────────────────────────────────────
  function onDragOver(e: DragEvent) {
    e.preventDefault()
    if (e.dataTransfer) e.dataTransfer.dropEffect = 'copy'
  }

  function onDrop(e: DragEvent) {
    e.preventDefault()

    const definitionId = e.dataTransfer?.getData('application/imgplex-node-id')
    const label = e.dataTransfer?.getData('application/imgplex-node-label')
    if (!definitionId || !screenToCanvas) return

    const canvasPos = screenToCanvas({ x: e.clientX, y: e.clientY })
    const position = {
      x: canvasPos.x - NODE_W / 2,
      y: canvasPos.y - NODE_H / 2,
    }

    const def = definitions.find((d) => d.id === definitionId)
    const isComment = def?.id === 'comment'
    const newNode: Node = {
      id: `${definitionId}-${Date.now()}`,
      type: def ? nodeTypeForDef(def) : 'process',
      position,
      data: def ? buildNodeData(def) : { label: label ?? definitionId, inputs: ['image'], outputs: ['image'], definitionId, params: {} },
      ...(isComment ? { width: 280, height: 120 } : {}),
    }

    nodes = [...nodes, newNode]
    pushHistory()
  }

  // ── Sync viewport to store (for saving) ───────────────────────────────────
  $effect(() => {
    const v = viewport
    untrack(() => { graphStore.viewport = v })
  })

  // ── Restore viewport when requested by store (after load/new) ─────────────
  $effect(() => {
    const pv = graphStore.pendingViewport
    const sv = setViewport
    if (pv && sv) {
      untrack(() => {
        sv(pv)
        graphStore.pendingViewport = null
      })
    }
  })

  // ── Menu IPC: duplicate / delete ───────────────────────────────────────────
  $effect(() => {
    function onDuplicate() {
      const selected = nodes.filter(
        (n) => n.selected && n.id !== INPUT_NODE_ID && n.id !== OUTPUT_NODE_ID,
      )
      // If nothing selected, duplicate the focused/last selected node
      const targets = selected.length > 0
        ? selected
        : graphStore.selectedNodeId &&
          graphStore.selectedNodeId !== INPUT_NODE_ID &&
          graphStore.selectedNodeId !== OUTPUT_NODE_ID
          ? nodes.filter((n) => n.id === graphStore.selectedNodeId)
          : []
      duplicateNodes(targets)
    }

    function onDelete() {
      const targetIds = new Set(
        nodes
          .filter(
            (n) =>
              n.selected &&
              n.id !== INPUT_NODE_ID &&
              n.id !== OUTPUT_NODE_ID,
          )
          .map((n) => n.id),
      )
      if (targetIds.size === 0 && graphStore.selectedNodeId) {
        const id = graphStore.selectedNodeId
        if (id !== INPUT_NODE_ID && id !== OUTPUT_NODE_ID) targetIds.add(id)
      }
      if (targetIds.size === 0) return
      nodes = nodes.filter((n) => !targetIds.has(n.id))
      edges = edges.filter((e) => !targetIds.has(e.source) && !targetIds.has(e.target))
      graphStore.selectedNodeId = null
      pushHistory()
    }

    window.ipcRenderer.on('menu:duplicate', onDuplicate)
    window.ipcRenderer.on('menu:delete', onDelete)
    return () => {
      window.ipcRenderer.off('menu:duplicate', onDuplicate)
      window.ipcRenderer.off('menu:delete', onDelete)
    }
  })
</script>

<div
  class="editor-wrap"
  tabindex="-1"
  bind:this={editorEl}
  ondragover={onDragOver}
  ondrop={onDrop}
  ondblclick={onDblClick}
  oncontextmenu={onContextMenu}
  onkeydown={onKeydown}
  onmousemove={onMouseMove}
>
  <SvelteFlow
    bind:nodes
    bind:edges
    bind:viewport
    {nodeTypes}
    {edgeTypes}
    onconnect={onConnect}
    onconnectstart={onConnectStart}
    onconnectend={onConnectEnd}
    onnodedragstop={onNodeDragStop}
    onnoderesizeend={() => scheduleHistoryPush()}
    onselectionchange={onselectionchange}
    {isValidConnection}
    colorMode="dark"
    zoomOnDoubleClick={false}
    proOptions={{ hideAttribution: true }}
    deleteKey={null}
    {connectionLineStyle}
  >
    <DropHelper
      onReady={(fn) => { screenToCanvas = fn }}
      onViewportReady={(fn) => { setViewport = fn }}
      onUpdateNodeInternalsReady={(fn) => { updateNodeInternals = fn }}
    />
    <Controls />
    <Background variant={bgVariant} gap={bgGap} lineWidth={bgLineWidth} patternColor={bgColor} bgColor={bgBaseColor} />
    <MiniMap />
  </SvelteFlow>
  <div class="zoom-label">{zoomPct}</div>

  {#if menuState}
    <NodeContextMenu
      x={menuState.x}
      y={menuState.y}
      filterType={menuState.filterType}
      {definitions}
      onSelect={onMenuSelect}
      onClose={closeMenu}
      {groupable}
      {ungroupable}
      onGroupSelection={() => { groupSelection(); closeMenu() }}
      onUngroup={() => { ungroupSelection(); closeMenu() }}
    />
  {/if}
</div>

<style>
  .editor-wrap {
    width: 100%;
    height: 100%;
    position: relative;
    outline: none;
  }

  /* xyflow dark mode adds a border and its own background to .svelte-flow — strip both */
  :global(.svelte-flow) {
    border: none !important;
    border-radius: 0 !important;
    background: transparent !important;
    /* Override dark mode defaults that paint edges/handles white */
    --xy-edge-stroke: #6b7280;
    --xy-edge-stroke-selected: #ffffff;
    --xy-handle-background-color: #6b7280;
    --xy-handle-border-color: #6b7280;
  }

  :global(.svelte-flow__minimap) {
    border-radius: var(--minimap-radius);
    overflow: hidden;
    opacity: var(--minimap-opacity);
    transition: opacity 0.2s;
  }

  :global(.svelte-flow__minimap:hover) {
    opacity: var(--minimap-opacity-hover);
  }

  :global(.svelte-flow__controls) {
    opacity: var(--controls-opacity);
    transition: opacity 0.2s;
  }

  :global(.svelte-flow__controls:hover) {
    opacity: var(--controls-opacity-hover);
  }

  .zoom-label {
    position: absolute;
    /* sit at the bottom-centre of the 200 px minimap (right: 10px) */
    bottom: var(--zoom-label-bottom);
    right: 10px;
    width: 200px;
    text-align: center;
    font-family: var(--text-zoom-label-family);
    font-size: var(--text-zoom-label-size);
    font-weight: var(--text-zoom-label-weight);
    text-transform: var(--text-zoom-label-transform);
    letter-spacing: var(--text-zoom-label-spacing);
    font-variant-numeric: tabular-nums;
    color: var(--zoom-label-color);
    pointer-events: none;
    z-index: 5;
  }
</style>
