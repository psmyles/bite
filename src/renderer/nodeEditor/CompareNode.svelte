<script lang="ts" context="module">
  const OPERATOR_SYMBOLS: Record<string, string> = {
    'equal':            '=',
    'not equal':        '≠',
    'greater than':     '>',
    'less than':        '<',
    'greater or equal': '≥',
    'less or equal':    '≤',
  }
</script>

<script lang="ts">
  import { Handle, Position } from '@xyflow/svelte'
  import { portColor } from './portColors.js'
  import { graphStore } from '../stores/graph.svelte.js'
  import { paramInHandle, paramOutHandle } from './wireTypeUtils.js'

  let {
    id = '',
    data = {},
    selected = false,
  }: { id?: string; data?: Record<string, unknown>; selected?: boolean } = $props()

  const params = $derived((data.params as Record<string, unknown>) ?? {})

  const operatorSymbol = $derived(OPERATOR_SYMBOLS[params.operator as string] ?? '?')

  // ── Wire detection (single pass) ─────────────────────────────────────────────
  const handleA = paramInHandle('a')
  const handleB = paramInHandle('b')
  const { aWired, bWired } = $derived.by(() => {
    let a = false, b = false
    for (const e of graphStore.edges) {
      if (e.target !== id) continue
      if (e.targetHandle === handleA) a = true
      else if (e.targetHandle === handleB) b = true
      if (a && b) break
    }
    return { aWired: a, bWired: b }
  })

  // ── Live values (resolved by preview executor) ───────────────────────────────
  const liveA = $derived(id ? (graphStore.propValues[id]?.a ?? params.a) : params.a)
  const liveB = $derived(id ? (graphStore.propValues[id]?.b ?? params.b) : params.b)
  const liveResult = $derived(id ? (graphStore.propValues[id]?.result ?? params.result) : params.result)

  function formatVal(v: unknown): string | null {
    if (v === null || v === undefined) return null
    if (typeof v === 'boolean') return v ? 'true' : 'false'
    if (typeof v === 'number') return parseFloat(Number(v).toFixed(3)).toString()
    if (typeof v === 'string') return v.length > 12 ? v.slice(0, 10) + '…' : v
    if (Array.isArray(v)) {
      return (v as number[]).slice(0, 4).map(n => parseFloat(Number(n).toFixed(2)).toString()).join(', ')
    }
    return null
  }

  // Show A/B value only when not wired — the static/internal value
  const displayA = $derived(!aWired ? formatVal(liveA) : null)
  const displayB = $derived(!bWired ? formatVal(liveB) : null)
  const displayResult = $derived(formatVal(liveResult))

  const anyColor  = portColor('any')
  const boolColor = portColor('boolean')

  // Handle positions — must match ProcessNode's paramHandleTop with no image ports.
  const HEADER_H = 28, PARAM_PAD = 4, PARAM_ROW_H = 22, HANDLE_OFFSET = 11
  const paramTop = (i: number) => `${HEADER_H + PARAM_PAD + i * PARAM_ROW_H + HANDLE_OFFSET}px`
  const topA      = paramTop(0)
  const topB      = paramTop(1)
  const topResult = paramTop(2)
</script>

<!-- A input handle -->
<Handle
  type="target"
  position={Position.Left}
  id={paramInHandle('a')}
  style="top: {topA}; background: {anyColor}; border-color: {anyColor};"
/>

<!-- B input handle -->
<Handle
  type="target"
  position={Position.Left}
  id={paramInHandle('b')}
  style="top: {topB}; background: {anyColor}; border-color: {anyColor};"
/>

<!-- Result output handle -->
<Handle
  type="source"
  position={Position.Right}
  id={paramOutHandle('result')}
  style="top: {topResult}; background: {boolColor}; border-color: {boolColor};"
/>

<div class="node" class:selected>
  <header class="node-head">
    <span class="head-label">Compare</span>
    <span class="op-badge">{operatorSymbol}</span>
  </header>

  <div class="param-ports">
    <!-- A row -->
    <div class="row">
      <span class="p-name" style="color: {anyColor}">A</span>
      {#if displayA !== null}
        <span class="p-value">{displayA}</span>
      {/if}
    </div>

    <!-- B row -->
    <div class="row">
      <span class="p-name" style="color: {anyColor}">B</span>
      {#if displayB !== null}
        <span class="p-value">{displayB}</span>
      {/if}
    </div>

    <!-- Result row (readonly output) -->
    <div class="row readonly-row">
      {#if displayResult !== null}
        <span class="p-value">{displayResult}</span>
      {/if}
      <span class="p-name readonly" style="color: {boolColor}">Result</span>
    </div>
  </div>
</div>

<style>
  .node {
    background: var(--node-bg);
    border: 1px solid var(--node-border);
    border-radius: var(--node-radius);
    min-width: var(--node-min-width);
    font-size: var(--font-size-sm);
    color: var(--node-text);
    box-shadow: var(--node-shadow);
  }

  .node.selected {
    border-color: var(--node-selected-border);
    box-shadow: var(--node-selected-shadow);
  }

  .node-head {
    height: 28px; /* --node-layout-header-h */
    padding: 0 10px;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    font-family: var(--text-node-head-family);
    font-size: var(--text-node-head-size);
    font-weight: var(--text-node-head-weight);
    text-transform: var(--text-node-head-transform);
    letter-spacing: var(--text-node-head-spacing);
    background: var(--node-head-bg);
    border-bottom: 1px solid var(--node-border);
    border-radius: calc(var(--node-radius) - 1px) calc(var(--node-radius) - 1px) 0 0;
    white-space: nowrap;
  }

  .head-label {
    flex-shrink: 0;
  }

  .op-badge {
    font-family: var(--font-mono);
    font-size: 13px;
    font-weight: 700;
    color: var(--accent);
    line-height: 1;
    flex-shrink: 0;
  }

  .param-ports {
    padding: 4px 0; /* --node-layout-param-pad */
  }

  .row {
    min-height: 22px; /* --node-layout-param-row-h */
    display: flex;
    align-items: center;
    padding: 0 10px;
  }

  .readonly-row {
    border-top: none;
  }

  .p-name {
    font-family: var(--font-mono);
    font-size: 11px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }

  .p-name.readonly {
    margin-left: auto;
  }

  .p-value {
    margin-left: auto;
    padding-left: 6px;
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text);
    white-space: nowrap;
    flex-shrink: 0;
  }

  .readonly-row .p-value {
    margin-left: 0;
    padding-left: 0;
  }
</style>
