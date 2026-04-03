<script lang="ts">
  import { Handle, Position } from '@xyflow/svelte'
  import { portColor } from './portColors.js'
  import { graphStore } from '../stores/graph.svelte.js'

  let {
    id = '',
    data = {},
    selected = false,
  }: { id?: string; data?: Record<string, unknown>; selected?: boolean } = $props()

  const imgColor  = portColor('image')
  const pathColor = portColor('path')
  const anyColor  = portColor('any')
  const boolColor = portColor('boolean')
  const numColor  = portColor('number')

  const params     = $derived((data.params as Record<string, unknown>) ?? {})
  const outputMode = $derived((params.outputMode as string) ?? 'image')

  // ── Text mode derived state ──────────────────────────────────────────────────
  const portIds = $derived((params.portIds as string[]) ?? ['txo-0'])

  const ghostPortId = $derived(portIds[portIds.length - 1])

  const displayPortIds = $derived(
    [...portIds].sort((a, b) => (parseInt(a.slice(4)) || 0) - (parseInt(b.slice(4)) || 0))
  )

  const portLabelMap = $derived.by(() => {
    // Build indexes once rather than scanning edges/nodes once per port
    const inEdgeMap = new Map<string, string>() // targetHandle → source node id
    for (const e of graphStore.edges) {
      if (e.target === id) inEdgeMap.set(e.targetHandle ?? '', e.source)
    }
    const nodeMap = new Map(graphStore.nodes.map(n => [n.id, n]))
    return new Map(portIds.map(portId => {
      const srcId = inEdgeMap.get(portId)
      const label = srcId
        ? ((nodeMap.get(srcId)?.data as Record<string, unknown> | undefined)?.label as string | null ?? null)
        : null
      return [portId, label] as [string, string | null]
    }))
  })

  function handleTop(i: number): string { return `${43 + i * 30}px` }

  const conditionTop = $derived(`${43 + displayPortIds.length * 30}px`)
</script>

{#if graphStore.batchRunning}
  <div class="processing-badge">Processing</div>
{/if}

{#if outputMode === 'text'}
  <!-- ── Text mode: dynamic txo ports + condition ── -->
  {#each displayPortIds as portId, i}
    <Handle
      type="target"
      position={Position.Left}
      id={portId}
      style="background: {anyColor}; border-color: {anyColor}; top: {handleTop(i)};"
    />
  {/each}
  <Handle
    type="target"
    position={Position.Left}
    id="txo-condition"
    style="background: {boolColor}; border-color: {boolColor}; top: {conditionTop};"
  />

  <div class="node" class:selected>
    <header class="node-head">
      <span>Output — Text</span>
    </header>

    {#each displayPortIds as portId}
      {@const isGhost = portId === ghostPortId}
      {@const label   = portLabelMap.get(portId)}
      <div class="port-row" class:ghost={isGhost}>
        <span class="port-tag" style="color: {anyColor}">
          {isGhost ? 'New Input' : (label ?? '—')}
        </span>
      </div>
    {/each}

    <div class="condition-row">
      <span class="cond-tag" style="color: {boolColor}">Condition</span>
    </div>
  </div>

{:else if outputMode === 'flipbook'}
  <!-- ── Flipbook mode: Image + 4 param ports ── -->
  <Handle
    type="target"
    position={Position.Left}
    id="in-0"
    style="background: {imgColor}; border-color: {imgColor}; top: 43px;"
  />
  <Handle
    type="target"
    position={Position.Left}
    id="param-in-cols"
    style="background: {numColor}; border-color: {numColor}; top: 73px;"
  />
  <Handle
    type="target"
    position={Position.Left}
    id="param-in-rows"
    style="background: {numColor}; border-color: {numColor}; top: 103px;"
  />
  <Handle
    type="target"
    position={Position.Left}
    id="param-in-cellWidth"
    style="background: {numColor}; border-color: {numColor}; top: 133px;"
  />
  <Handle
    type="target"
    position={Position.Left}
    id="param-in-cellHeight"
    style="background: {numColor}; border-color: {numColor}; top: 163px;"
  />

  <div class="node" class:selected>
    <header class="node-head">
      <span>Output — Flipbook</span>
    </header>

    <div class="node-ports">
      <span class="port-tag" style="color: {imgColor}">Image</span>
    </div>
    <div class="node-ports">
      <span class="port-tag" style="color: {numColor}">Columns</span>
    </div>
    <div class="node-ports">
      <span class="port-tag" style="color: {numColor}">Rows</span>
    </div>
    <div class="node-ports">
      <span class="port-tag" style="color: {numColor}">Cell Width</span>
    </div>
    <div class="node-ports">
      <span class="port-tag" style="color: {numColor}">Cell Height</span>
    </div>
  </div>

{:else}
  <!-- ── Image mode: existing Image + Folder ports ── -->
  <Handle
    type="target"
    position={Position.Left}
    id="in-0"
    style="background: {imgColor}; border-color: {imgColor}; top: 43px;"
  />
  <Handle
    type="target"
    position={Position.Left}
    id="folder-in"
    style="background: {pathColor}; border-color: {pathColor}; top: 73px;"
  />

  <div class="node" class:selected>
    <header class="node-head">
      <span>Output — Image</span>
    </header>

    <div class="node-ports">
      <span class="port-tag" style="color: {imgColor}">Image</span>
    </div>

    <div class="node-ports">
      <span class="port-tag" style="color: {pathColor}">Folder</span>
    </div>
  </div>
{/if}

<style>
  .node {
    background: var(--node-bg);
    border: 1px solid var(--node-border);
    border-radius: var(--node-radius);
    min-width: 190px;
    font-size: var(--font-size-sm);
    color: var(--node-text);
    box-shadow: var(--node-shadow);
  }

  .node.selected {
    border-color: var(--node-selected-border);
    box-shadow: var(--node-selected-shadow);
  }

  /* ── Header ── */
  .node-head {
    height: 28px;
    padding: 0 12px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-family: var(--text-node-head-family);
    font-size: var(--text-node-head-size);
    font-weight: var(--text-node-head-weight);
    text-transform: var(--text-node-head-transform);
    letter-spacing: var(--text-node-head-spacing);
    background: color-mix(in srgb, #f59e0b 18%, var(--node-head-bg));
    border-bottom: 1px solid var(--node-border);
    border-radius: calc(var(--node-radius) - 1px) calc(var(--node-radius) - 1px) 0 0;
    white-space: nowrap;
  }

  /* ── Image mode port rows ── */
  .node-ports {
    display: flex;
    justify-content: flex-start;
    padding: 5px 10px;
    min-height: 30px;
    align-items: center;
  }

  /* ── Text mode port rows ── */
  .port-row {
    display: flex;
    align-items: center;
    padding: 5px 10px;
    min-height: 30px;
  }

  .port-row.ghost { opacity: 0.4; }

  .condition-row {
    display: flex;
    align-items: center;
    padding: 5px 10px;
    min-height: 30px;
    border-top: 1px solid var(--node-border);
  }

  /* ── Port tags ── */
  .port-tag,
  .cond-tag {
    height: 20px;
    line-height: 20px;
    font-family: var(--text-port-tag-family);
    font-size: var(--text-port-tag-size);
    font-weight: var(--text-port-tag-weight);
    text-transform: var(--text-port-tag-transform);
    letter-spacing: var(--text-port-tag-spacing);
    white-space: nowrap;
  }

  .port-tag {
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 160px;
  }

  /* ── Processing badge — floats above the node ── */
  .processing-badge {
    position: absolute;
    top: -22px;
    left: 50%;
    transform: translateX(-50%);
    font-family: var(--font-ui);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: #f59e0b;
    background: color-mix(in srgb, var(--bg) 85%, transparent);
    border: 1px solid #f59e0b;
    border-radius: 3px;
    padding: 2px 7px;
    white-space: nowrap;
    pointer-events: none;
    z-index: 10;
  }
</style>
