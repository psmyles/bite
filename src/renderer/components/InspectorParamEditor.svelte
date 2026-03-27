<script lang="ts">
  import type { Node } from '@xyflow/svelte'
  import type { NodeDefinition, ParamDefinition, VisibilityRule } from '../../shared/types.js'
  import { graphStore } from '../stores/graph.svelte.js'
  import ColorPicker from './ColorPicker.svelte'
  import Dropdown from './Dropdown.svelte'
  import { getNodeParams } from '../nodeEditor/nodeEditorHelpers.js'
  import { paramInHandle } from '../nodeEditor/wireTypeUtils.js'

  let {
    definition,
    selectedNode,
  }: {
    definition: NodeDefinition
    selectedNode: Node
  } = $props()

  const params = $derived(getNodeParams(selectedNode?.data))

  function getValue(p: ParamDefinition): unknown {
    // For Properties node readonly outputs, use live values computed from the last preview run.
    if (p.readonly && selectedNode && definition?.executor?.startsWith('prop_')) {
      const live = graphStore.propValues[selectedNode.id]?.[p.name]
      if (live !== undefined) return live
    }
    return p.name in params ? params[p.name] : p.default
  }

  /** Returns true if this param has an incoming wire connected to it. */
  function isWired(p: ParamDefinition): boolean {
    return graphStore.edges.some(
      (e) => e.target === selectedNode.id && e.targetHandle === paramInHandle(p.name)
    )
  }

  /** Returns the value arriving via the wire, falling back to the local value. */
  function getWiredValue(p: ParamDefinition): unknown {
    const edge = graphStore.edges.find(
      (e) => e.target === selectedNode.id && e.targetHandle === paramInHandle(p.name)
    )
    if (!edge) return getValue(p)
    const sourceNode = graphStore.nodes.find((n) => n.id === edge.source)
    if (!sourceNode) return getValue(p)
    const srcParamName = (edge.sourceHandle ?? '').replace('param-out-', '')
    const srcParams = getNodeParams(sourceNode.data)
    return srcParams[srcParamName] ?? getValue(p)
  }

  function isAtDefault(p: ParamDefinition): boolean {
    if (p.default === undefined) return true
    const cur = getValue(p)
    return JSON.stringify(cur) === JSON.stringify(p.default)
  }

  function resetToDefault(p: ParamDefinition) {
    if (p.default === undefined) return
    graphStore.setParam(selectedNode.id, p.name, p.default)
  }

  /** Returns false if a params_visibility rule hides this param given the current param values. */
  function isVisible(p: ParamDefinition): boolean {
    const rules: VisibilityRule[] = definition.params_visibility ?? []
    const rule = rules.find((r) => r.show === p.name)
    if (!rule) return true
    return params[rule.when.param] === rule.when.eq
  }

  function onChange(p: ParamDefinition, raw: string | boolean) {
    let value: unknown
    if (p.type === 'int') {
      const n = parseInt(raw as string, 10)
      value = isNaN(n) ? (p.default ?? 0) : n
    } else if (p.type === 'float' || p.type === 'numeric') {
      const n = parseFloat(raw as string)
      value = isNaN(n) ? (p.default ?? 0) : n
    } else if (p.type === 'bool') {
      value = raw
    } else {
      value = raw
    }
    graphStore.setParam(selectedNode.id, p.name, value)
  }

  function onVectorChange(p: ParamDefinition, index: number, rawVal: string) {
    const current = Array.isArray(getValue(p)) ? [...(getValue(p) as number[])] : []
    while (current.length <= index) current.push(0)
    current[index] = parseFloat(rawVal) || 0
    graphStore.setParam(selectedNode.id, p.name, current)
  }
</script>

{#each definition.params.filter((p) => !p.portOnly && isVisible(p)) as p (p.name)}
  {@const wired = isWired(p)}
  <div class="param-row" class:wired>
    <label class="param-label" for="param-{p.name}">
      {p.label}
      {#if wired}<span class="wired-badge">wired</span>{/if}
      {#if p.readonly && !wired && definition?.executor}<span class="output-badge">out</span>{/if}
      {#if !wired && !p.readonly && p.default !== undefined && !isAtDefault(p)}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <span
          class="reset-btn"
          onclick={() => resetToDefault(p)}
          title="Reset to default"
          role="button"
          tabindex="-1"
        >↺</span>
      {/if}
    </label>

    {#if wired}
      <!-- Show the value arriving from the connected node -->
      {@const wv = getWiredValue(p)}
      <div class="wired-value">{Array.isArray(wv) ? (wv as number[]).map(v => (v as number).toFixed(3)).join(', ') : wv}</div>

    {:else if p.readonly && definition.executor}
      <!-- Computed output — display only regardless of widget type -->
      {@const cv = getValue(p)}
      <div class="computed-value">{Array.isArray(cv) ? (cv as number[]).map(v => v.toFixed(3)).join(', ') : typeof cv === 'boolean' ? String(cv) : cv}</div>

    {:else if p.widget === 'slider'}
      <div class="slider-wrap">
        <input
          id="param-{p.name}"
          type="range"
          min={p.min ?? 0}
          max={p.max ?? 100}
          step={p.step ?? (p.type === 'float' ? 0.01 : 1)}
          value={getValue(p) as number}
          oninput={(e) => onChange(p, (e.target as HTMLInputElement).value)}
          class="slider"
        />
        <input
          type="number"
          class="slider-val"
          value={getValue(p) as number}
          step={p.step ?? (p.type === 'float' ? 0.01 : 1)}
          oninput={(e) => onChange(p, (e.target as HTMLInputElement).value)}
        />
      </div>

    {:else if p.widget === 'number'}
      <input
        id="param-{p.name}"
        type="number"
        value={getValue(p) as number}
        step={p.step ?? (p.type === 'int' ? 1 : 'any')}
        oninput={(e) => onChange(p, (e.target as HTMLInputElement).value)}
        class="number-input"
      />

    {:else if p.widget === 'dropdown'}
      <Dropdown
        value={getValue(p) as string}
        options={p.options ?? []}
        onchange={(v) => onChange(p, v)}
      />

    {:else if p.widget === 'checkbox'}
      <input
        id="param-{p.name}"
        type="checkbox"
        checked={getValue(p) as boolean}
        onchange={(e) => onChange(p, (e.target as HTMLInputElement).checked)}
        class="checkbox"
      />

    {:else if p.widget === 'color-picker'}
      <input
        id="param-{p.name}"
        type="color"
        value={getValue(p) as string}
        oninput={(e) => onChange(p, (e.target as HTMLInputElement).value)}
        class="color-pick"
      />

    {:else if p.widget === 'vector' && p.type === 'color'}
      {@const vals = Array.isArray(getValue(p)) ? (getValue(p) as number[]) : [0, 0, 0, 1]}
      <ColorPicker
        value={vals}
        onchange={(v) => { graphStore.setParam(selectedNode.id, p.name, v) }}
      />

    {:else if p.widget === 'vector'}
      {@const labels = p.type === 'vector4' ? ['X','Y','Z','W'] : p.type === 'vector3' ? ['X','Y','Z'] : ['X','Y']}
      {@const vals = Array.isArray(getValue(p)) ? (getValue(p) as number[]) : []}
      <div class="vector-wrap">
        {#each labels as lbl, idx}
          <div class="vector-field">
            <span class="vector-lbl">{lbl}</span>
            <input
              type="number"
              class="vector-input"
              value={vals[idx] ?? 0}
              step={p.step ?? 0.01}
              oninput={(e) => onVectorChange(p, idx, (e.target as HTMLInputElement).value)}
            />
          </div>
        {/each}
      </div>

    {:else}
      <!-- text / fallback -->
      <input
        id="param-{p.name}"
        type="text"
        value={getValue(p) as string}
        oninput={(e) => onChange(p, (e.target as HTMLInputElement).value)}
        class="text-input"
      />
    {/if}
  </div>
{/each}

<style>
  /* ── Param row ── */
  .param-row {
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding: 7px 12px;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 25%, transparent);
  }

  .param-label {
    display: flex;
    align-items: center;
    gap: 6px;
    font-family: var(--font-ui);
    font-size: 12px;
    color: var(--text-bright);
    opacity: 0.6;
    user-select: none;
  }

  .wired-badge {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--port-color-number);
    opacity: 1;
    border: 1px solid var(--port-color-number);
    border-radius: 3px;
    padding: 0 3px;
    line-height: 14px;
  }

  .output-badge {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-bright);
    opacity: 0.5;
    border: 1px solid var(--border);
    border-radius: 3px;
    padding: 0 3px;
    line-height: 14px;
  }

  .reset-btn {
    margin-left: auto;
    font-size: 12px;
    color: var(--text-bright);
    opacity: 0.6;
    cursor: pointer;
    line-height: 1;
    transition: opacity 0.12s, color 0.12s;
    user-select: none;
    flex-shrink: 0;
  }

  .reset-btn:hover {
    opacity: 1;
    color: var(--accent);
  }

  .wired-value {
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--port-color-number);
    opacity: 0.8;
    padding: 3px 0;
  }

  .computed-value {
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--text-bright);
    opacity: 0.5;
    padding: 3px 0;
  }

  .param-row.wired { opacity: 0.75; }

  /* ── Slider ── */
  .slider-wrap {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .slider {
    flex: 1;
    appearance: none;
    -webkit-appearance: none;
    height: 3px;
    border-radius: 2px;
    background: color-mix(in srgb, var(--border) 60%, transparent);
    outline: none;
    cursor: pointer;
  }

  .slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--accent);
    cursor: pointer;
    border: none;
  }

  .slider::-moz-range-thumb {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--accent);
    cursor: pointer;
    border: none;
  }

  .slider-val {
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--text-bright);
    width: 52px;
    text-align: right;
    flex-shrink: 0;
    background: var(--panel-header-bg);
    border: 1px solid var(--border);
    border-radius: 4px;
    outline: none;
    padding: 4px 6px;
    appearance: textfield;
    -moz-appearance: textfield;
  }

  .slider-val::-webkit-outer-spin-button,
  .slider-val::-webkit-inner-spin-button { -webkit-appearance: none; margin: 0; }

  .slider-val:focus { border-color: var(--accent); }

  /* ── Number input ── */
  .number-input {
    width: 100%;
    background: var(--panel-header-bg);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text-bright);
    font-family: var(--font-mono);
    font-size: 12px;
    padding: 4px 6px;
    outline: none;
    appearance: textfield;
    -moz-appearance: textfield;
  }

  .number-input::-webkit-outer-spin-button,
  .number-input::-webkit-inner-spin-button { -webkit-appearance: none; margin: 0; }

  .number-input:focus { border-color: var(--accent); }

  /* ── Checkbox ── */
  .checkbox {
    width: 14px;
    height: 14px;
    cursor: pointer;
    accent-color: var(--accent);
  }

  /* ── Color picker ── */
  .color-pick {
    width: 100%;
    height: 28px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: transparent;
    cursor: pointer;
    padding: 2px;
  }

  /* ── Vector input ── */
  .vector-wrap {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .vector-field {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .vector-lbl {
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--text-bright);
    opacity: 0.5;
    width: 14px;
    flex-shrink: 0;
    text-align: center;
  }

  .vector-input {
    flex: 1;
    background: var(--panel-header-bg);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text-bright);
    font-family: var(--font-mono);
    font-size: 12px;
    padding: 3px 6px;
    outline: none;
    appearance: textfield;
    -moz-appearance: textfield;
  }

  .vector-input::-webkit-outer-spin-button,
  .vector-input::-webkit-inner-spin-button { -webkit-appearance: none; margin: 0; }

  .vector-input:focus { border-color: var(--accent); }

  /* ── Text input ── */
  .text-input {
    width: 100%;
    background: var(--panel-header-bg);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text-bright);
    font-family: var(--font-mono);
    font-size: 12px;
    padding: 4px 6px;
    outline: none;
  }

  .text-input:focus { border-color: var(--accent); }
</style>
