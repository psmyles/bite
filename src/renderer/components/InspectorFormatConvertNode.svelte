<script lang="ts">
  import type { Node } from '@xyflow/svelte';
  import type { NodeDefinition, FormatDefinition, ParamDefinition } from '../../shared/types.js';
  import { graphStore } from '../stores/graph.svelte.js';
  import { getNodeParams } from '../nodeEditor/nodeEditorHelpers.js';
  import Dropdown from './Dropdown.svelte';

  let { definition, selectedNode }: { definition: NodeDefinition; selectedNode: Node } = $props();

  const params = $derived(getNodeParams(selectedNode?.data));

  const rawDefs = import.meta.glob('../../../format-definitions/*.json', { eager: true });
  const FORMAT_DEFS: Record<string, FormatDefinition> = {};
  for (const mod of Object.values(rawDefs)) {
    const d = mod as FormatDefinition;
    FORMAT_DEFS[d.id.toUpperCase()] = d;
  }

  const format = $derived(((params.format as string) ?? 'PNG').toUpperCase());
  const activeDef = $derived(FORMAT_DEFS[format]);

  const formatParam = $derived(definition.params.find((p) => p.name === 'format'));

  function isVisible(p: ParamDefinition): boolean {
    const rule = (activeDef?.params_visibility ?? []).find((r) => r.show === p.name);
    if (!rule) return true;
    const actual =
      rule.when.param in params
        ? params[rule.when.param]
        : activeDef?.params.find((q) => q.name === rule.when.param)?.default;
    return actual === rule.when.eq;
  }

  function getValue(p: ParamDefinition): unknown {
    return p.name in params ? params[p.name] : p.default;
  }

  function onChange(p: ParamDefinition, raw: string | boolean) {
    let value: unknown;
    if (p.type === 'int') {
      const n = parseInt(raw as string, 10);
      value = isNaN(n) ? (p.default ?? 0) : n;
    } else if (p.type === 'bool') {
      value = raw;
    } else {
      value = raw;
    }
    graphStore.setParam(selectedNode.id, p.name, value);
  }
</script>

<!-- Format selector -->
<div class="param-row">
  <span class="param-label">{formatParam?.label ?? 'Format'}</span>
  <Dropdown
    value={format}
    options={formatParam?.options ?? []}
    onchange={(v) => graphStore.setParam(selectedNode.id, 'format', v)}
  />
</div>

<!-- Format-specific params -->
{#each (activeDef?.params ?? []).filter((p) => isVisible(p)) as p (p.name)}
  <div class="param-row" class:param-inline={p.widget === 'checkbox'}>
    <span class="param-label">{p.label}</span>
    {#if p.widget === 'slider'}
      <div class="slider-wrap">
        <input
          type="range"
          class="slider"
          min={p.min ?? 0}
          max={p.max ?? 100}
          step={p.step ?? 1}
          value={getValue(p) as number}
          oninput={(e) => onChange(p, (e.target as HTMLInputElement).value)}
        />
        <input
          type="number"
          class="slider-val"
          value={getValue(p) as number}
          step={p.step ?? 1}
          oninput={(e) => onChange(p, (e.target as HTMLInputElement).value)}
        />
      </div>
    {:else if p.widget === 'dropdown'}
      <Dropdown value={getValue(p) as string} options={p.options ?? []} onchange={(v) => onChange(p, v)} />
    {:else if p.widget === 'checkbox'}
      <input
        type="checkbox"
        checked={getValue(p) as boolean}
        onchange={(e) => onChange(p, (e.target as HTMLInputElement).checked)}
      />
    {/if}
  </div>
{/each}

{#if activeDef && activeDef.params.length === 0}
  <div class="no-options-hint">No encoding options for this format.</div>
{/if}

<style>
  .param-row {
    display: flex;
    flex-direction: column;
    gap: var(--inspector-param-gap);
    padding: var(--inspector-param-padding);
    border-bottom: var(--inspector-row-border);
  }

  .param-inline {
    flex-direction: row;
    align-items: center;
    justify-content: space-between;
    gap: 0;
  }

  .param-label {
    font-family: var(--font-ui);
    font-size: var(--font-size-sm);
    color: var(--text-bright);
    opacity: 0.6;
    user-select: none;
  }

  /* ── Slider ── */
  .slider-wrap {
    display: flex;
    align-items: center;
    gap: var(--slider-wrap-gap);
    width: 100%;
  }

  .slider {
    flex: 1;
    appearance: none;
    -webkit-appearance: none;
    height: var(--slider-track-height);
    border-radius: var(--slider-track-radius);
    background: var(--slider-track-bg);
    outline: none;
    cursor: pointer;
  }

  .slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: var(--slider-thumb-size);
    height: var(--slider-thumb-size);
    border-radius: 50%;
    background: var(--accent);
    cursor: pointer;
    border: none;
  }

  .slider::-moz-range-thumb {
    width: var(--slider-thumb-size);
    height: var(--slider-thumb-size);
    border-radius: 50%;
    background: var(--accent);
    cursor: pointer;
    border: none;
  }

  .slider-val {
    font-family: var(--font-mono);
    font-size: var(--font-size-sm);
    color: var(--text-bright);
    width: var(--slider-val-width);
    text-align: right;
    flex-shrink: 0;
    background: var(--panel-header-bg);
    border: 1px solid var(--border);
    border-radius: var(--slider-val-radius);
    outline: none;
    padding: var(--slider-val-padding);
    appearance: textfield;
    -moz-appearance: textfield;
  }

  .slider-val::-webkit-outer-spin-button,
  .slider-val::-webkit-inner-spin-button {
    -webkit-appearance: none;
    margin: 0;
  }

  .slider-val:focus {
    border-color: var(--accent);
  }

  .no-options-hint {
    padding: var(--inspector-param-padding);
    font-family: var(--font-ui);
    font-size: var(--font-size-sm);
    color: var(--text-bright);
    opacity: 0.4;
  }
</style>
