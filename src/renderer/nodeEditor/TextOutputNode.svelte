<script lang="ts">
  import { Handle, Position } from '@xyflow/svelte';
  import { portColor } from './portColors.js';
  import { graphStore } from '../stores/graph.svelte.js';

  let {
    id = '',
    data = {},
    selected = false,
  }: { id?: string; data?: Record<string, unknown>; selected?: boolean } = $props();

  const imgColor = portColor('image');
  const anyColor = portColor('any');

  const params = $derived((data.params as Record<string, unknown>) ?? {});
  const portIds = $derived((params.portIds as string[]) ?? ['txo-0']);
  const ghostPortId = $derived(portIds[portIds.length - 1]);
  const displayPortIds = $derived(
    [...portIds].sort((a, b) => (parseInt(a.slice(4)) || 0) - (parseInt(b.slice(4)) || 0))
  );

  const portLabelMap = $derived.by(() => {
    const inEdgeMap = new Map<string, string>();
    for (const e of graphStore.edges) {
      if (e.target === id) inEdgeMap.set(e.targetHandle ?? '', e.source);
    }
    const nodeMap = new Map(graphStore.nodes.map((n) => [n.id, n]));
    return new Map(
      portIds.map((portId) => {
        const srcId = inEdgeMap.get(portId);
        const label = srcId
          ? (((nodeMap.get(srcId)?.data as Record<string, unknown> | undefined)?.label as string | null) ?? null)
          : null;
        return [portId, label] as [string, string | null];
      })
    );
  });

  const outputPath = $derived((params.outputPath as string) ?? '');
  const footerLabel = $derived(outputPath || 'no output file set');

  function handleTop(i: number): string {
    return `${71 + i * 30}px`;
  }
</script>

<!-- Image input — top-left -->
<Handle
  type="target"
  position={Position.Left}
  id="in-0"
  style="background: {imgColor}; border-color: {imgColor}; top: 43px;"
/>

<!-- Dynamic txo param ports -->
{#each displayPortIds as portId, i}
  <Handle
    type="target"
    position={Position.Left}
    id={portId}
    style="background: {anyColor}; border-color: {anyColor}; top: {handleTop(i)};"
  />
{/each}

<div class="node" class:selected>
  <header class="node-head">
    <span>Text Output</span>
  </header>

  <!-- Image port row -->
  <div class="node-ports img-row">
    <span class="port-tag" style="color: {imgColor}">Image</span>
  </div>

  <!-- Txo param port rows -->
  {#each displayPortIds as portId}
    {@const isGhost = portId === ghostPortId}
    {@const label = portLabelMap.get(portId)}
    <div class="port-row" class:ghost={isGhost}>
      <span class="port-tag" style="color: {anyColor}">
        {isGhost ? 'New Input' : (label ?? '—')}
      </span>
    </div>
  {/each}

  <div class="node-footer">
    <span class="footer-label" title={footerLabel}>{footerLabel}</span>
  </div>
</div>

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
    font-family: var(--text-node-head-family);
    font-size: var(--text-node-head-size);
    font-weight: var(--text-node-head-weight);
    text-transform: var(--text-node-head-transform);
    letter-spacing: var(--text-node-head-spacing);
    background: color-mix(in srgb, #3b82f6 18%, var(--node-head-bg));
    border-bottom: 1px solid var(--node-border);
    border-radius: calc(var(--node-radius) - 1px) calc(var(--node-radius) - 1px) 0 0;
    white-space: nowrap;
  }

  .img-row {
    display: flex;
    justify-content: flex-start;
    padding: 5px 10px;
    min-height: 30px;
    align-items: center;
    border-bottom: 1px solid var(--node-border);
  }

  .port-row {
    display: flex;
    align-items: center;
    padding: 5px 10px;
    min-height: 30px;
  }

  .port-row.ghost {
    opacity: 0.4;
  }

  .port-tag {
    height: 20px;
    line-height: 20px;
    font-family: var(--text-port-tag-family);
    font-size: var(--text-port-tag-size);
    font-weight: var(--text-port-tag-weight);
    text-transform: var(--text-port-tag-transform);
    letter-spacing: var(--text-port-tag-spacing);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 160px;
  }

  .node-footer {
    height: 22px;
    display: flex;
    align-items: center;
    padding: 0 10px;
    border-top: 1px solid var(--node-border);
  }

  .footer-label {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 165px;
  }
</style>
