<script lang="ts">
  import { Handle, Position } from '@xyflow/svelte';
  import { portColor } from './portColors.js';
  import { imageStore } from '../stores/images.svelte.js';

  interface Props {
    data: { params?: Record<string, unknown>; description?: string };
    selected?: boolean;
  }
  let { data, selected = false }: Props = $props();

  const imgColor = portColor('image');
  const strColor = portColor('string');
  const description = $derived(data.description ?? '');

  let tooltipVisible = $state(false);
  let tooltipTimer: ReturnType<typeof setTimeout> | undefined;
  let tooltipX = $state(0);
  let tooltipY = $state(0);
  let headerEl = $state<HTMLElement | null>(null);

  function portal(el: HTMLElement): { destroy(): void } {
    document.body.appendChild(el);
    return {
      destroy() {
        el.remove();
      },
    };
  }

  function onHeaderEnter() {
    if (!description) return;
    tooltipTimer = setTimeout(() => {
      if (headerEl) {
        const r = headerEl.getBoundingClientRect();
        tooltipX = r.left + r.width / 2;
        tooltipY = r.bottom + 6;
      }
      tooltipVisible = true;
    }, 1000);
  }

  function onHeaderLeave() {
    clearTimeout(tooltipTimer);
    tooltipVisible = false;
  }

  const suffixes = $derived(Array.isArray(data?.params?.suffixes) ? (data.params!.suffixes as string[]) : []);
  const prefix = $derived(String(data?.params?.prefix ?? ''));

  const matchCount = $derived(() => {
    if (!prefix && suffixes.length === 0) return 0;
    let n = 0;
    const seen = new Set<string>();
    for (const img of imageStore.images) {
      const name = img.name.replace(/\.[^.]+$/, '');
      if (!name.startsWith(prefix)) continue;
      const rest = name.slice(prefix.length);
      for (const s of suffixes) {
        if (s && rest.endsWith(s)) {
          const mid = rest.slice(0, rest.length - s.length);
          if (!seen.has(mid)) {
            seen.add(mid);
            n++;
          }
          break;
        }
      }
    }
    return n;
  });

  const setLabel = $derived(
    matchCount() === 0 ? 'no sets matched' : matchCount() === 1 ? '1 set matched' : `${matchCount()} sets matched`
  );

  // Handle positions:
  // Header: 28px, Images row: 26px, Prefix row: 26px, Suffix rows: 26px each
  // in-0 center:      28 + 13 = 41px
  // prefix-in center: 28 + 26 + 13 = 67px
  // suf-in-i / out-i: 28 + 26 + 26 + i*26 + 13 = 93 + 26*i px
  const IMG_TOP = 41;
  const PREFIX_TOP = 67;
  const sufTop = (i: number) => 93 + 26 * i;
</script>

<!-- Image input — fixed below header in the Images row -->
<Handle
  type="target"
  position={Position.Left}
  id="in-0"
  style="top: {IMG_TOP}px; background: {imgColor}; border-color: {imgColor};"
/>

<!-- Prefix string input handle -->
<Handle
  type="target"
  position={Position.Left}
  id="prefix-in"
  style="top: {PREFIX_TOP}px; background: {strColor}; border-color: {strColor};"
/>

<!-- Per-suffix string input handles (left) and image output handles (right) -->
{#each suffixes as _s, i}
  <Handle
    type="target"
    position={Position.Left}
    id="suf-in-{i}"
    style="top: {sufTop(i)}px; background: {strColor}; border-color: {strColor};"
  />
  <Handle
    type="source"
    position={Position.Right}
    id="out-{i}"
    style="top: {sufTop(i)}px; background: {imgColor}; border-color: {imgColor};"
  />
{/each}

<div class="node" class:selected>
  <header class="node-head" bind:this={headerEl} onmouseenter={onHeaderEnter} onmouseleave={onHeaderLeave}>
    <span class="head-label">Process As Set</span>
  </header>

  <!-- Images input row — always visible -->
  <div class="img-row">
    <span class="port-tag" style="color: {imgColor}">Image</span>
  </div>

  <!-- Prefix string input row — always visible -->
  <div class="img-row">
    <span class="port-tag" style="color: {strColor}">Prefix</span>
  </div>

  {#if suffixes.length === 0}
    <div class="node-empty">No suffixes configured</div>
  {:else}
    {#each suffixes as suffix, i}
      <div class="port-row">
        <span class="port-tag" style="color: {imgColor}">{suffix || `suffix${i + 1}`}</span>
      </div>
    {/each}
  {/if}

  <div class="node-footer">
    <span class="set-count">{setLabel}</span>
  </div>
</div>

{#if tooltipVisible && description}
  <div use:portal class="node-tooltip-fixed" style="left:{tooltipX}px; top:{tooltipY}px">{description}</div>
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

  .node-head {
    height: 28px;
    padding: 0 12px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: color-mix(in srgb, var(--node-accent-set-input) 20%, var(--node-head-bg));
    border-bottom: 1px solid var(--node-border);
    border-radius: calc(var(--node-radius) - 1px) calc(var(--node-radius) - 1px) 0 0;
  }

  .head-label {
    font-family: var(--text-node-head-family);
    font-size: var(--text-node-head-size);
    font-weight: var(--text-node-head-weight);
    text-transform: var(--text-node-head-transform);
    letter-spacing: var(--text-node-head-spacing);
    white-space: nowrap;
  }

  .img-row {
    display: flex;
    justify-content: flex-start;
    align-items: center;
    padding: 3px 10px;
    height: 26px;
    border-bottom: 1px solid var(--node-border);
  }

  .node-empty {
    padding: 8px 12px;
    font-family: var(--font-ui);
    font-size: var(--font-size-sm);
    color: var(--text-muted);
    font-style: italic;
  }

  .port-row {
    display: flex;
    justify-content: flex-end;
    align-items: center;
    padding: 3px 10px;
    height: 26px;
  }

  .port-tag {
    font-family: var(--text-port-tag-family);
    font-size: var(--text-port-tag-size);
    font-weight: var(--text-port-tag-weight);
    text-transform: var(--text-port-tag-transform);
    letter-spacing: var(--text-port-tag-spacing);
    white-space: nowrap;
  }

  .node-footer {
    height: 22px;
    display: flex;
    align-items: center;
    padding: 0 10px;
    border-top: 1px solid var(--node-border);
  }

  .set-count {
    font-family: var(--font-mono);
    font-size: var(--font-size-xs);
    color: var(--text);
  }
</style>
