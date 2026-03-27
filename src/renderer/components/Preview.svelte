<script lang="ts">
  import { untrack } from 'svelte'
  import { imageStore } from '../stores/images.svelte.js'
  import { graphStore } from '../stores/graph.svelte.js'
  import { IPC, EMPTY_GRAPH } from '../../shared/constants.js'
  import type { NodeGraph, GraphNode, GraphEdge } from '../../shared/types.js'
  import type { Node, Edge } from '@xyflow/svelte'
  import { isNodeEffectivelyEnabled } from '../nodeEditor/nodeEnabledState.js'
  import { IS_ELECTRON } from '../platform.js'

  const INPUT_NODE_ID  = 'workflow-input'
  const OUTPUT_NODE_ID = 'workflow-output'

  function toNodeGraph(sfNodes: Node[], sfEdges: Edge[]): NodeGraph {
    return {
      nodes: sfNodes.map((n) => ({
        id: n.id,
        type: n.type ?? 'process',
        position: n.position,
        data: n.data as GraphNode['data'],
      })),
      edges: sfEdges.map((e) => ({
        id: e.id,
        source: e.source,
        sourceHandle: e.sourceHandle ?? undefined,
        target: e.target,
        targetHandle: e.targetHandle ?? undefined,
      })),
      viewport: { x: 0, y: 0, zoom: 1 },
    }
  }

  interface PreviewGraphResult {
    graph: NodeGraph
    effectivePreviewId: string | null
    hasConnected: boolean
  }

  // isNodeEffectivelyEnabled is imported from nodeEnabledState.js

  /**
   * Walk backward through image edges (in-N handles) from `startId`, returning
   * the first non-bypassed predecessor, or null if we reach Input or a dead end.
   */
  function findNonBypassedPredecessor(
    startId: string,
    sfNodes: Node[],
    sfEdges: Edge[],
  ): string | null {
    let current = startId
    const seen = new Set<string>()
    while (true) {
      if (seen.has(current)) return null
      seen.add(current)
      const inEdge = sfEdges.find(
        (e) => e.target === current && (e.targetHandle ?? '').startsWith('in-'),
      )
      if (!inEdge || inEdge.source === INPUT_NODE_ID) return null
      const pred = inEdge.source
      if (isNodeEffectivelyEnabled(pred, sfNodes, sfEdges)) return pred
      current = pred
    }
  }

  /**
   * Builds a filtered graph containing only nodes reachable from Input and up to
   * (and including) the effective preview node. Disconnected nodes are excluded.
   * Bypassed nodes are skipped when auto-detecting the default preview node.
   */
  function buildPreviewGraph(
    sfNodes: Node[],
    sfEdges: Edge[],
    userPreviewId: string | null,
  ): PreviewGraphResult {
    // BFS forward from Input — find all reachable nodes
    const reachable = new Set<string>()
    const q: string[] = [INPUT_NODE_ID]
    while (q.length > 0) {
      const id = q.shift()!
      if (reachable.has(id)) continue
      reachable.add(id)
      for (const e of sfEdges) {
        if (e.source === id && !reachable.has(e.target)) q.push(e.target)
      }
    }

    // Default preview node: walk backward from Output's image input, skipping
    // bypassed nodes, until we land on a non-bypassed process node.
    let defaultPreviewId: string | null = null
    for (const e of sfEdges) {
      if (
        e.target === OUTPUT_NODE_ID &&
        (e.targetHandle ?? '').startsWith('in-') &&
        reachable.has(e.source) &&
        e.source !== INPUT_NODE_ID
      ) {
        defaultPreviewId = isNodeEffectivelyEnabled(e.source, sfNodes, sfEdges)
          ? e.source
          : findNonBypassedPredecessor(e.source, sfNodes, sfEdges)
        break
      }
    }

    // Fallback: last non-bypassed terminal process node in the reachable set
    if (!defaultPreviewId) {
      const processReachable = sfNodes.filter(
        (n) => n.type === 'process' && reachable.has(n.id) && isNodeEffectivelyEnabled(n.id, sfNodes, sfEdges),
      )
      for (let i = processReachable.length - 1; i >= 0; i--) {
        const n = processReachable[i]
        const hasSuccessor = sfEdges.some(
          (e) => e.source === n.id && reachable.has(e.target) && e.target !== OUTPUT_NODE_ID,
        )
        if (!hasSuccessor) {
          defaultPreviewId = n.id
          break
        }
      }
      if (!defaultPreviewId && processReachable.length > 0) {
        defaultPreviewId = processReachable[processReachable.length - 1].id
      }
    }

    // Effective preview node:
    // - If user's choice is reachable and non-bypassed → use it.
    // - If user's choice is bypassed → fall back to nearest non-bypassed predecessor.
    // - Otherwise → use auto-detected default.
    let effectivePreviewId: string | null
    if (
      userPreviewId &&
      reachable.has(userPreviewId) &&
      userPreviewId !== INPUT_NODE_ID &&
      userPreviewId !== OUTPUT_NODE_ID
    ) {
      effectivePreviewId = isNodeEffectivelyEnabled(userPreviewId, sfNodes, sfEdges)
        ? userPreviewId
        : findNonBypassedPredecessor(userPreviewId, sfNodes, sfEdges) ?? defaultPreviewId
    } else {
      effectivePreviewId = defaultPreviewId
    }

    if (!effectivePreviewId) {
      // No connected image pipeline. Still evaluate all pure value/logic nodes
      // (no image inputs, no image outputs) so their computed values appear in the canvas.
      const pureNodes = sfNodes.filter((n) => {
        if (n.type !== 'process') return false
        const d = n.data as Record<string, unknown>
        const ins  = (d.inputs  as string[] | undefined) ?? []
        const outs = (d.outputs as string[] | undefined) ?? []
        return ins.length === 0 && outs.length === 0
      })
      if (pureNodes.length === 0) {
        return { graph: EMPTY_GRAPH, effectivePreviewId: null, hasConnected: false }
      }
      const pureIds = new Set(pureNodes.map((n) => n.id))
      const pureEdges = sfEdges.filter((e) => pureIds.has(e.source) || pureIds.has(e.target))
      return { graph: toNodeGraph(pureNodes, pureEdges), effectivePreviewId: null, hasConnected: false }
      // Note: mean_value-like nodes (image input, no image output) are excluded here
      // because there's no image pipeline to source their inputs from.
    }

    // BFS backward from effectivePreviewId to find its ancestors
    const ancestors = new Set<string>()
    const bq: string[] = [effectivePreviewId]
    while (bq.length > 0) {
      const id = bq.shift()!
      if (ancestors.has(id)) continue
      ancestors.add(id)
      for (const e of sfEdges) {
        if (e.target === id && !ancestors.has(e.source)) bq.push(e.source)
      }
    }

    // Nodes to include: all ancestors of effectivePreviewId.
    // We do NOT intersect with `reachable` because pure logic/math/value nodes
    // (e.g. Compare, Dimensions) have no image ports so they are never "reachable
    // from Input" via the forward BFS, yet they are valid param-wire predecessors
    // that must be evaluated to compute _enabled and other wired params.
    const nodeSet = ancestors

    const filteredNodes = sfNodes.filter((n) => nodeSet.has(n.id))
    const filteredEdges = sfEdges.filter((e) => nodeSet.has(e.source) && nodeSet.has(e.target))
    const hasConnected = filteredNodes.some((n) => n.type === 'process')

    // Include all nodes NOT in the ancestor set that:
    //  1. Produce no image outputs (so they don't extend the image pipeline), AND
    //  2. All their image inputs come from nodes already in the ancestor set
    //     (so imageBuffers has the data they need — covers mean_value-like nodes),
    //     OR they have no image inputs at all (pure value/logic/property nodes).
    const extraNodes = sfNodes.filter((n) => {
      if (nodeSet.has(n.id) || n.type !== 'process') return false
      const d = n.data as Record<string, unknown>
      const outs = (d.outputs as string[] | undefined) ?? []
      if (outs.length > 0) return false  // skip nodes that produce image outputs
      const imageInEdges = sfEdges.filter(
        (e) => e.target === n.id && (e.targetHandle ?? '').startsWith('in-'),
      )
      return imageInEdges.every((e) => nodeSet.has(e.source))
    })
    const extraIds = new Set(extraNodes.map((n) => n.id))
    const propEdges = sfEdges.filter(
      (e) => (extraIds.has(e.source) || extraIds.has(e.target)) &&
        !filteredEdges.some((fe) => fe.id === e.id)
    )

    return {
      graph: toNodeGraph([...filteredNodes, ...extraNodes], [...filteredEdges, ...propEdges]),
      effectivePreviewId,
      hasConnected,
    }
  }

  let previewSrc = $state<string | null>(null)
  let showInfo   = $state(true)

  // ── Persistent debouncer ────────────────────────────────────────────────────
  let _timer: ReturnType<typeof setTimeout> | null = null
  let _seq = 0

  function schedulePreview(path: string, graph: NodeGraph, delayMs: number) {
    if (_timer !== null) clearTimeout(_timer)
    const seq = ++_seq
    _timer = setTimeout(async () => {
      _timer = null
      try {
        const result: { dataUrl: string; propParams: Record<string, Record<string, unknown>> } =
          await window.ipcRenderer.invoke(IPC.EXECUTE_PREVIEW, graph, path)
        if (seq === _seq && imageStore.selected?.path === path) {
          previewSrc = result.dataUrl
          untrack(() => {
            for (const [nodeId, values] of Object.entries(result.propParams)) {
              graphStore.setPropValues(nodeId, values)
            }
          })
        }
      } catch (err) { console.error('[preview] Render failed:', err) }
    }, delayMs)
  }

  // Pipeline-relevant content key — excludes position/selection/measured-height.
  // Includes previewNodeId so the effect re-runs when the user changes preview target.
  const graphKey = $derived(
    graphStore.nodes
      .map((n) => {
        const d = n.data as Record<string, unknown>
        return `${n.id}:${n.type}:${JSON.stringify(d?.params ?? {})}`
      })
      .join('|')
    + '||' +
    graphStore.edges
      .map((e) => `${e.source}:${e.sourceHandle ?? ''}->${e.target}:${e.targetHandle ?? ''}`)
      .join('|')
    + '||' + (graphStore.previewNodeId ?? 'auto')
  )

  $effect(() => {
    if (!IS_ELECTRON) return   // preview requires ImageMagick; no-op in browser

    const selected = imageStore.selected
    const _key     = graphKey  // track pipeline content + preview node selection

    if (!selected) {
      if (_timer !== null) { clearTimeout(_timer); _timer = null }
      ++_seq
      previewSrc = null
      untrack(() => { graphStore.activePreviewNodeId = null })
      return
    }

    if (!previewSrc) previewSrc = selected.thumbnailDataUrl ?? null

    const path       = selected.path
    const sfNodes    = $state.snapshot(untrack(() => graphStore.nodes)) as Node[]
    const sfEdges    = $state.snapshot(untrack(() => graphStore.edges)) as Edge[]
    const userPrevId = untrack(() => graphStore.previewNodeId)

    const { graph, effectivePreviewId, hasConnected } = buildPreviewGraph(sfNodes, sfEdges, userPrevId)

    // Propagate the active preview node so ProcessNode can render the badge
    untrack(() => { graphStore.activePreviewNodeId = effectivePreviewId })

    const delay = hasConnected ? 80 : 0
    schedulePreview(path, graph, delay)
  })

  function formatSize(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
  }

  const info = $derived(imageStore.selected ? {
    name:   imageStore.selected.name,
    format: imageStore.selected.format,
    dims:   `${imageStore.selected.width} × ${imageStore.selected.height}`,
    size:   formatSize(imageStore.selected.sizeBytes),
  } : null)
</script>

<div class="preview">
  <div class="panel-header">
    <span>Preview</span>
    {#if imageStore.selected}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <button
        class="info-btn"
        class:active={showInfo}
        onclick={() => showInfo = !showInfo}
        title="Toggle image info"
        aria-label="Toggle image info"
        aria-pressed={showInfo}
      >i</button>
    {/if}
  </div>

  <div class="preview-area">
    {#if previewSrc}
      <img src={previewSrc} alt={imageStore.selected?.name} class="preview-img" />
      {#if showInfo && info}
        <div class="info-overlay">
          <span class="info-name">{info.name}</span>
          <span class="info-meta">{info.format} &middot; {info.dims} &middot; {info.size}</span>
        </div>
      {/if}
    {:else}
      <span class="empty-hint">
        {IS_ELECTRON ? 'No image selected.' : 'Preview requires the desktop app.'}
      </span>
    {/if}
  </div>
</div>

<style>
  .preview {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    background: var(--preview-bg);
  }

  /* ── Header ── */
  .panel-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 8px 0 12px;
    height: 36px;
    flex-shrink: 0;
    font-family: var(--text-panel-header-family);
    font-size: var(--text-panel-header-size);
    font-weight: var(--text-panel-header-weight);
    text-transform: var(--text-panel-header-transform);
    letter-spacing: var(--text-panel-header-spacing);
    color: var(--text-bright);
    background: var(--panel-header-bg);
  }

  .info-btn {
    width: 20px;
    height: 20px;
    border-radius: 50%;
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text-bright);
    font-family: var(--font-ui);
    font-size: 11px;
    font-style: italic;
    font-weight: 600;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    opacity: 0.5;
    transition: opacity 0.15s, background 0.15s;
    flex-shrink: 0;
    line-height: 1;
  }

  .info-btn:hover { opacity: 1; }

  .info-btn.active {
    opacity: 1;
    background: color-mix(in srgb, var(--accent) 20%, transparent);
    border-color: var(--accent);
  }

  /* ── Preview area ── */
  .preview-area {
    flex: 1;
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
  }

  .preview-img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: contain;
  }

  /* ── Info overlay — frosted strip at the bottom of the preview area ── */
  .info-overlay {
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
    padding: 10px 12px 8px;
    display: flex;
    flex-direction: column;
    gap: 3px;
    background: linear-gradient(to top, rgba(0,0,0,0.72) 0%, rgba(0,0,0,0) 100%);
    backdrop-filter: blur(6px);
    -webkit-backdrop-filter: blur(6px);
  }

  .info-name {
    font-family: var(--font-ui);
    font-size: 12px;
    font-weight: 600;
    color: #fff;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .info-meta {
    font-family: var(--font-mono);
    font-size: 11px;
    color: rgba(255, 255, 255, 0.65);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .empty-hint {
    font-family: var(--text-hint-family);
    font-size: var(--text-hint-size);
    font-weight: var(--text-hint-weight);
    color: var(--text-bright);
    opacity: 0.5;
  }
</style>
