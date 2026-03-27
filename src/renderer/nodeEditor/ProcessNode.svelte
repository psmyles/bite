<script lang="ts">
  import { Handle, Position } from '@xyflow/svelte'
  import { portColor } from './portColors.js'
  import { graphStore } from '../stores/graph.svelte.js'
  import { getNodeParams, type ParamPortDef } from './nodeEditorHelpers.js'
  import { paramTypeToWireType, paramInHandle, paramOutHandle } from './wireTypeUtils.js'

  function formatParamValue(type: string, value: unknown): string | null {
    if (value === null || value === undefined) return null
    if (type === 'int') return String(Math.round(Number(value)))
    if (type === 'float' || type === 'numeric') {
      const n = Number(value)
      if (isNaN(n)) return null
      return parseFloat(n.toFixed(2)).toString()
    }
    if (type === 'string') {
      const s = String(value)
      return s.length > 0 ? (s.length > 14 ? s.slice(0, 12) + '…' : s) : null
    }
    if (type === 'bool') return value ? 'true' : 'false'
    if (Array.isArray(value)) {
      const parts = (value as number[]).map((n) => parseFloat(Number(n).toFixed(2)).toString())
      return parts.join(', ')
    }
    return null
  }

  function isChanged(p: ParamPort, params: Record<string, unknown>): boolean {
    if (p.readonly) return false
    const cur = params[p.name]
    if (p.default === undefined) return cur !== undefined && cur !== null
    return JSON.stringify(cur) !== JSON.stringify(p.default)
  }

  function colorToCss(value: unknown): string | null {
    if (!Array.isArray(value) || value.length < 3) return null
    const [r, g, b, a = 1] = value as number[]
    const ri = Math.round(Math.max(0, Math.min(1, r)) * 255)
    const gi = Math.round(Math.max(0, Math.min(1, g)) * 255)
    const bi = Math.round(Math.max(0, Math.min(1, b)) * 255)
    const af = Math.max(0, Math.min(1, a))
    return `rgba(${ri},${gi},${bi},${af})`
  }

  interface NodeData {
    label: string
    description?: string
    inputs?: string[]
    outputs?: string[]
    inputLabels?: string[]
    outputLabels?: string[]
    paramDefs?: ParamPortDef[]
    params?: Record<string, unknown>
  }

  let {
    id = '',
    data,
    selected = false,
  }: { id?: string; data: NodeData; selected?: boolean } = $props()

  const isPreviewTarget = $derived(id !== '' && graphStore.activePreviewNodeId === id)
  const description = $derived(data.description ?? '')

  const inputs       = $derived(data.inputs       ?? ['image'])
  const outputs      = $derived(data.outputs      ?? ['image'])
  const inputLabels  = $derived(data.inputLabels  ?? [])
  const outputLabels = $derived(data.outputLabels ?? [])
  const paramDefs    = $derived(data.paramDefs    ?? [])
  const hasImagePorts = $derived(inputs.length > 0 || outputs.length > 0)

  // Body params: shown as rows in the node body (non-portOnly)
  const bodyParamDefs  = $derived(paramDefs.filter((p) => !p.portOnly))
  // Output slots: right-side port handles with labels, not shown as body rows
  const outputSlotDefs = $derived(paramDefs.filter((p) => p.portOnly))

  // Nodes with both image in AND image out are "actionable" — they get the bypass toggle.
  const isActionable = $derived(inputs.length > 0 && outputs.length > 0)

  // Local toggle state from params (true = active, missing = active)
  // Treat only explicit false/0 as disabled; undefined/null/missing defaults to enabled
  const enabled = $derived(data.params?._enabled !== false && data.params?._enabled !== 0)

  // ── Bypass wire detection ─────────────────────────────────────────────────
  // Check if param-in-_enabled has an incoming wire; if so, hide the manual toggle
  // and derive the effective enabled state from the source node's param value.
  const enabledEdge = $derived(
    isActionable
      ? graphStore.edges.find((e) => e.target === id && e.targetHandle === paramInHandle('_enabled'))
      : undefined,
  )
  const hasEnabledWire = $derived(!!enabledEdge)

  // Best-effort read of the connected wire's current value for visual feedback.
  const wiredEnabledValue = $derived.by(() => {
    const edge = enabledEdge
    if (!edge) return true
    const srcHandle = edge.sourceHandle
    if (!srcHandle?.startsWith('param-out-')) return true
    const srcParamName = srcHandle.slice(paramOutHandle('').length)
    const srcNode = graphStore.nodes.find((n) => n.id === edge.source)
    const srcParams = getNodeParams(srcNode?.data)
    const val = srcParams?.[srcParamName]
    // Treat 0, false, null, undefined as "bypass" (disabled)
    return val !== false && val !== 0 && val !== null && val !== undefined
  })

  // Effective enabled state — wire value takes precedence over manual toggle
  const effectiveEnabled = $derived(hasEnabledWire ? wiredEnabledValue : enabled)

  function toggleEnabled(e: MouseEvent) {
    e.stopPropagation()
    if (id) graphStore.setParam(id, '_enabled', !enabled)
  }

  // ── Header tooltip ────────────────────────────────────────────────────────
  let tooltipVisible = $state(false)
  let tooltipTimer: ReturnType<typeof setTimeout> | undefined
  let tooltipX = $state(0)
  let tooltipY = $state(0)
  let headerEl = $state<HTMLElement | null>(null)

  // Portal action: moves the element to document.body so position:fixed is
  // relative to the viewport, not any CSS-transformed ancestor (Svelte Flow nodes).
  function portal(el: HTMLElement): { destroy(): void } {
    document.body.appendChild(el)
    return { destroy() { el.remove() } }
  }

  function onHeaderEnter() {
    if (!description) return
    tooltipTimer = setTimeout(() => {
      if (headerEl) {
        const r = headerEl.getBoundingClientRect()
        tooltipX = r.left + r.width / 2
        tooltipY = r.bottom + 6
      }
      tooltipVisible = true
    }, 1000)
  }

  function onHeaderLeave() {
    clearTimeout(tooltipTimer)
    tooltipVisible = false
  }

  // ── Layout constants (px) ─────────────────────────────────────────────────
  // Read from CSS custom properties — theme.css is the single source of truth.
  // If you need to change a value, update --node-layout-* in theme.css only.
  function cssLayoutNum(prop: string): number {
    return parseFloat(getComputedStyle(document.documentElement).getPropertyValue(prop)) || 0
  }
  const HEADER_H    = cssLayoutNum('--node-layout-header-h')
  const PORT_PAD    = cssLayoutNum('--node-layout-port-pad')
  const PORT_ROW_H  = cssLayoutNum('--node-layout-port-row-h')
  const PARAM_PAD   = cssLayoutNum('--node-layout-param-pad')
  const PARAM_ROW_H = cssLayoutNum('--node-layout-param-row-h')
  const SEP_H       = cssLayoutNum('--node-layout-sep-h')

  const imgSectionH = $derived(
    hasImagePorts
      ? PORT_PAD * 2 + Math.max(inputs.length, outputs.length, 1) * PORT_ROW_H
      : 0
  )

  // Height of the body-params section, including its border-top separator when image ports exist
  const bodyParamSectionH = $derived(
    bodyParamDefs.length > 0
      ? (hasImagePorts ? SEP_H : 0) + PARAM_PAD * 2 + bodyParamDefs.length * PARAM_ROW_H
      : 0
  )

  function imgHandleTop(i: number): string {
    return `${HEADER_H + PORT_PAD + i * PORT_ROW_H + PORT_ROW_H / 2}px`
  }

  function paramHandleTop(i: number): string {
    const sepOffset = hasImagePorts ? SEP_H : 0
    return `${HEADER_H + imgSectionH + sepOffset + PARAM_PAD + i * PARAM_ROW_H + PARAM_ROW_H / 2}px`
  }

  function outputSlotHandleTop(i: number): string {
    // sep = border-top of the .output-slots div (always present when something precedes it)
    const sepOffset = (hasImagePorts || bodyParamDefs.length > 0) ? SEP_H : 0
    return `${HEADER_H + imgSectionH + bodyParamSectionH + sepOffset + PORT_PAD + i * PORT_ROW_H + PORT_ROW_H / 2}px`
  }

  // Maps portOnly component names to their index in the source array
  const COMPONENT_INDEX: Record<string, number> = { x: 0, y: 1, z: 2, w: 3, r: 0, g: 1, b: 2, a: 3 }
  // Maps portOnly combined names to how many elements to slice from the source array
  const COMBINED_SLICE: Record<string, number> = { xy: 2, xyz: 3, xyzw: 4, rgb: 3, rgba: 4 }

  function resolveSlotValue(p: ParamPort, params: Record<string, unknown>): unknown {
    // Use stored value if already computed (e.g. after executor ran)
    const stored = params[p.name]
    if (stored !== null && stored !== undefined) return stored
    // Otherwise derive from the first array-valued body param (e.g. vec, color)
    const src = bodyParamDefs.map((bp) => params[bp.name]).find((v) => Array.isArray(v)) as number[] | undefined
    if (!src) return undefined
    const sliceLen = COMBINED_SLICE[p.name]
    if (sliceLen !== undefined) return src.slice(0, sliceLen)
    const idx = COMPONENT_INDEX[p.name]
    return idx !== undefined ? (src[idx] ?? 0) : undefined
  }

  /** Resolve the wire type flowing through a source handle by inspecting the source node's data. */
  function getConnectedWireType(targetHandleId: string): string | null {
    const edge = graphStore.edges.find(e => e.target === id && e.targetHandle === targetHandleId)
    if (!edge) return null
    const srcNode = graphStore.nodes.find(n => n.id === edge.source)
    if (!srcNode) return null
    const srcHandle = edge.sourceHandle
    if (!srcHandle) return null
    const srcData = srcNode.data as Record<string, unknown>
    if (srcHandle.startsWith('param-out-')) {
      const pName = srcHandle.slice(paramOutHandle('').length)
      const pd = (srcData.paramDefs as Array<{name: string; type: string}> | undefined)
        ?.find(p => p.name === pName)
      if (!pd) return null
      if (pd.type === 'any') return null
      return paramTypeToWireType(pd.type)
    }
    if (srcHandle.startsWith('out-')) {
      const idx = parseInt(srcHandle.slice(4))
      return (srcData.outputs as string[] | undefined)?.[idx] ?? null
    }
    return null
  }

  function paramPortColor(p: ParamPortDef): string {
    if (p.type === 'any') {
      if (p.readonly) {
        for (const inp of paramDefs.filter(d => d.type === 'any' && !d.readonly)) {
          const wt = getConnectedWireType(paramInHandle(inp.name))
          if (wt) return portColor(wt)
        }
      } else {
        const wt = getConnectedWireType(paramInHandle(p.name))
        if (wt) return portColor(wt)
      }
      return portColor('any')
    }
    return portColor(paramTypeToWireType(p.type))
  }
</script>

{#if isPreviewTarget}
  <div class="previewing-badge">Previewing</div>
{/if}

<!-- Image input handles (left side) — only when node has image ports -->
{#if hasImagePorts}
  {#each inputs as type, i}
    <Handle
      type="target"
      position={Position.Left}
      id={`in-${i}`}
      style={`top: ${imgHandleTop(i)}; background: ${portColor(type)}; border-color: ${portColor(type)};`}
    />
  {/each}
{/if}

<!-- Param input handles (left side) — writable body params only -->
{#each bodyParamDefs as p, i}
  {#if !p.readonly && !p.noPort}
    <Handle
      type="target"
      position={Position.Left}
      id={paramInHandle(p.name)}
      style={`top: ${paramHandleTop(i)}; background: ${paramPortColor(p)}; border-color: ${paramPortColor(p)};`}
    />
  {/if}
{/each}

<div class="node" class:selected class:bypassed={!effectiveEnabled}>
  <!-- has-toggle adds left padding so the label doesn't overlap the tick -->
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <header
    bind:this={headerEl}
    class="node-head"
    class:has-toggle={isActionable && !hasEnabledWire}
    onmouseenter={onHeaderEnter}
    onmouseleave={onHeaderLeave}
  >
    {#if isActionable && !hasEnabledWire}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <span
        class="bypass-tick"
        class:active={enabled}
        onclick={toggleEnabled}
        ondblclick={(e) => e.stopPropagation()}
        role="checkbox"
        aria-checked={enabled}
        aria-label="Toggle node bypass"
        tabindex="-1"
      ></span>
    {/if}
    {data.label}
  </header>

  <!-- Image ports section (hidden for pure value/math/logic nodes) -->
  {#if hasImagePorts}
    <div class="node-ports">
      <div class="port-col">
        {#each inputs as type, i}
          <span class="port-tag" style={`color: ${portColor(type)}`}>{inputLabels[i] ?? type}</span>
        {/each}
      </div>
      <div class="port-col right">
        {#each outputs as type, i}
          <span class="port-tag" style={`color: ${portColor(type)}`}>{outputLabels[i] ?? type}</span>
        {/each}
      </div>
    </div>
  {/if}

  <!-- Param ports section (body params only — portOnly params go in output slots) -->
  {#if bodyParamDefs.length > 0}
    <div class="param-ports" class:no-sep={!hasImagePorts}>
      {#each bodyParamDefs as p}
        {@const params = (data.params ?? {}) as Record<string, unknown>}
        {@const liveVal = id ? (graphStore.propValues[id]?.[p.name] ?? params[p.name]) : params[p.name]}
        <div class="param-port-row" class:readonly-row={p.readonly}>
          {#if p.readonly}
            <!-- readonly: value left, label right (mirrors output-slot layout) -->
            {#if p.type !== 'color'}
              {@const formatted = formatParamValue(p.type, liveVal)}
              {#if formatted !== null}
                <span class="param-value">{formatted}</span>
              {/if}
            {/if}
            <span class="param-name readonly" style={`color: ${paramPortColor(p)}`}>{p.label}</span>
          {:else}
            {#if p.type === 'color'}
              {@const swatch = colorToCss(params[p.name])}
              {#if swatch}
                <span class="color-swatch" style={`background: ${swatch};`}></span>
              {/if}
            {/if}
            <span class="param-name" style={`color: ${paramPortColor(p)}`}>{p.label}</span>
            {#if p.type !== 'color' && isChanged(p, params)}
              {@const formatted = formatParamValue(p.type, params[p.name])}
              {#if formatted !== null}
                <span class="param-value">{formatted}</span>
              {/if}
            {/if}
          {/if}
        </div>
      {/each}
    </div>
  {/if}

  <!-- Output slots section — port-only derived outputs (e.g. color channels) -->
  {#if outputSlotDefs.length > 0}
    <div class="output-slots">
      {#each outputSlotDefs as p}
        {@const params = (data.params ?? {}) as Record<string, unknown>}
        {@const liveParams = id ? { ...params, ...graphStore.propValues[id] } : params}
        {@const slotVal = formatParamValue(p.type, resolveSlotValue(p, liveParams))}
        <div class="output-slot-row">
          {#if slotVal !== null && COMBINED_SLICE[p.name] === undefined}
            <span class="param-value">{slotVal}</span>
          {/if}
          <span class="port-tag" style={`color: ${paramPortColor(p)}`}>{p.label}</span>
        </div>
      {/each}
    </div>
  {/if}
</div>

<!-- Image output handles (right side) — only when node has image ports -->
{#if hasImagePorts}
  {#each outputs as type, i}
    <Handle
      type="source"
      position={Position.Right}
      id={`out-${i}`}
      style={`top: ${imgHandleTop(i)}; background: ${portColor(type)}; border-color: ${portColor(type)};`}
    />
  {/each}
{/if}

<!-- Param output handles (right side) — readonly body params -->
{#each bodyParamDefs as p, i}
  {#if p.readonly}
    <Handle
      type="source"
      position={Position.Right}
      id={paramOutHandle(p.name)}
      style={`top: ${paramHandleTop(i)}; background: ${paramPortColor(p)}; border-color: ${paramPortColor(p)};`}
    />
  {/if}
{/each}
<!-- Output slot handles (right side) — portOnly derived outputs -->
{#each outputSlotDefs as p, i}
  <Handle
    type="source"
    position={Position.Right}
    id={paramOutHandle(p.name)}
    style={`top: ${outputSlotHandleTop(i)}; background: ${paramPortColor(p)}; border-color: ${paramPortColor(p)};`}
  />
{/each}

<!-- Bypass boolean handle — rendered AFTER .node so it sits on top of the header area.
     Positioned at header vertical center (top: HEADER_H/2). -->
{#if isActionable}
  <Handle
    type="target"
    position={Position.Left}
    id={paramInHandle('_enabled')}
    style={`top: ${HEADER_H / 2}px; background: ${portColor('boolean')}; border-color: ${portColor('boolean')}; z-index: 2;`}
  />
{/if}

<!-- Tooltip portaled to document.body so position:fixed is viewport-relative,
     not relative to the CSS-transformed Svelte Flow node ancestor. -->
{#if tooltipVisible && description}
  <div
    use:portal
    class="node-tooltip-fixed"
    style="left:{tooltipX}px; top:{tooltipY}px"
  >{description}</div>
{/if}

<style>
  .node {
    background: var(--node-bg);
    border: 1px solid var(--node-border);
    border-radius: var(--node-radius);
    min-width: var(--node-min-width);
    font-size: var(--font-size-sm);
    color: var(--node-text);
    box-shadow: var(--node-shadow);
    transition: opacity 0.15s;
  }

  .node.selected {
    border-color: var(--node-selected-border);
    box-shadow: var(--node-selected-shadow);
  }

  /* Bypassed nodes are visually dimmed */
  .node.bypassed {
    opacity: 0.45;
  }

  /* ── Header ── */
  .node-head {
    height: 28px; /* --node-layout-header-h */
    padding: 0 12px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-family: var(--text-node-head-family);
    font-size: var(--text-node-head-size);
    font-weight: var(--text-node-head-weight);
    text-transform: var(--text-node-head-transform);
    letter-spacing: var(--text-node-head-spacing);
    text-align: center;
    background: var(--node-head-bg);
    border-bottom: 1px solid var(--node-border);
    border-radius: calc(var(--node-radius) - 1px) calc(var(--node-radius) - 1px) 0 0;
    white-space: nowrap;
    position: relative;
  }

  /* When the tick is visible, offset both sides equally so the label stays centred */
  .node-head.has-toggle {
    padding-left: 34px;
    padding-right: 34px;
  }

  /* ── Bypass tick — positioned in the top-left of the header ── */
  .bypass-tick {
    position: absolute;
    left: 10px;
    top: 50%;
    transform: translateY(-50%);
    width: 14px;
    height: 14px;
    border: 1.5px solid color-mix(in srgb, var(--node-text) 35%, transparent);
    border-radius: 2px;
    cursor: pointer;
    flex-shrink: 0;
    transition: border-color 0.12s, background 0.12s;
    background: transparent;
  }

  /* Tick mark drawn via pseudo-element when active */
  .bypass-tick.active {
    border-color: var(--port-color-boolean, #22d3ee);
    background: color-mix(in srgb, var(--port-color-boolean, #22d3ee) 20%, transparent);
  }

  .bypass-tick.active::after {
    content: '';
    position: absolute;
    inset: 0;
    margin: auto;
    width: 4px;
    height: 7px;
    border-right: 2px solid var(--port-color-boolean, #22d3ee);
    border-bottom: 2px solid var(--port-color-boolean, #22d3ee);
    transform: rotate(45deg);
  }

  .bypass-tick:hover {
    border-color: var(--port-color-boolean, #22d3ee);
  }

  /* ── Image port rows ── */
  .node-ports {
    display: flex;
    justify-content: space-between;
    padding: 5px 10px; /* --node-layout-port-pad */
    gap: 20px;
    min-height: 30px;
  }

  .port-col {
    display: flex;
    flex-direction: column;
    gap: 0;
  }

  .port-col.right {
    text-align: right;
    margin-left: auto;
  }

  .port-tag {
    height: 20px; /* --node-layout-port-row-h */
    line-height: 20px;
    font-family: var(--text-port-tag-family);
    font-size: var(--text-port-tag-size);
    font-weight: var(--text-port-tag-weight);
    text-transform: var(--text-port-tag-transform);
    letter-spacing: var(--text-port-tag-spacing);
    white-space: nowrap;
  }

  /* ── Param ports ── */
  .param-ports {
    border-top: 1px solid var(--node-border); /* --node-layout-sep-h */
    padding: 4px 0; /* --node-layout-param-pad */
  }

  .param-ports.no-sep {
    border-top: none;
  }

  .param-port-row {
    min-height: 22px; /* --node-layout-param-row-h */
    display: flex;
    align-items: center;
    padding: 0 10px;
  }

  .param-name {
    font-family: var(--font-mono);
    font-size: 11px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }

  .param-name.readonly {
    margin-left: auto;
  }

  /* readonly body params: value left, label right — same layout as output-slot rows */
  .param-port-row.readonly-row .param-value {
    margin-left: 0;
    padding-left: 0;
  }

  .param-value {
    margin-left: auto;
    padding-left: 6px;
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text);
    white-space: nowrap;
    flex-shrink: 0;
  }

  /* ── Output slots (portOnly params — e.g. color channels) ── */
  .output-slots {
    border-top: 1px solid var(--node-border);
    padding: 5px 0;
  }

  .output-slot-row {
    height: 20px;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    padding: 0 10px;
  }

  .output-slot-row .param-value {
    margin-left: 0;
    margin-right: auto;
    padding-left: 0;
  }

  .color-swatch {
    display: inline-block;
    width: 20px;
    height: 20px;
    border-radius: 3px;
    border: 1px solid rgba(255, 255, 255, 0.18);
    flex-shrink: 0;
    margin-left: -6px;
    margin-right: 6px;
  }

  /* ── Header tooltip — fixed so it escapes node stacking contexts ── */
  :global(.node-tooltip-fixed) {
    position: fixed;
    transform: translateX(-50%);
    background: color-mix(in srgb, var(--bg) 92%, transparent);
    border: 1px solid var(--node-border);
    border-radius: 4px;
    padding: 5px 9px;
    font-family: var(--font-ui);
    font-size: 12px;
    color: var(--text-bright, #aaa);
    white-space: normal;
    width: max-content;
    max-width: 320px;
    min-width: 180px;
    text-align: left;
    pointer-events: none;
    z-index: 9999;
    box-shadow: 0 2px 8px rgba(0,0,0,0.4);
  }

  /* ── Previewing badge — floats above the node ── */
  .previewing-badge {
    position: absolute;
    top: -22px;
    left: 50%;
    transform: translateX(-50%);
    font-family: var(--font-ui);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: #39ff14;
    background: color-mix(in srgb, var(--bg) 85%, transparent);
    border: 1px solid #39ff14;
    border-radius: 3px;
    padding: 2px 7px;
    white-space: nowrap;
    pointer-events: none;
    z-index: 10;
  }

  /* Override SvelteFlow's default handle style */
  :global(.svelte-flow__handle) {
    width: var(--handle-port-size);
    height: var(--handle-port-size);
    border-width: var(--handle-port-border);
  }
</style>
