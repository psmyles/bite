<script lang="ts">
  import { Handle, Position } from '@xyflow/svelte'
  import { portColor } from './portColors.js'
  import { imageStore } from '../stores/images.svelte.js'

  let { selected = false }: { data: unknown; selected?: boolean } = $props()

  const imgColor = portColor('image')

  const countLabel = $derived(
    imageStore.images.length === 0 ? 'no images'
    : imageStore.images.length === 1 ? '1 image'
    : `${imageStore.images.length} images`
  )
</script>

<!-- Output handle — right side, vertically centered -->
<Handle
  type="source"
  position={Position.Right}
  id="out-0"
  style="background: {imgColor}; border-color: {imgColor};"
/>

<div class="node" class:selected>
  <header class="node-head">
    <span class="head-label">Input</span>
  </header>

  <div class="node-ports">
    <span class="port-tag" style="color: {imgColor}">Image</span>
  </div>

  <div class="node-footer">
    <span class="img-count">{countLabel}</span>
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

  /* ── Header ── */
  .node-head {
    height: 28px;
    padding: 0 12px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: color-mix(in srgb, #22c55e 18%, var(--node-head-bg));
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

  /* ── Port row ── */
  .node-ports {
    display: flex;
    justify-content: flex-end;
    padding: 5px 10px;
    min-height: 30px;
    align-items: center;
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
  }

  /* ── Footer ── */
  .node-footer {
    height: 22px;
    display: flex;
    align-items: center;
    padding: 0 10px;
    border-top: 1px solid var(--node-border);
  }

  .img-count {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text);
  }
</style>
