<script lang="ts">
  import { Handle, Position } from '@xyflow/svelte';
  import { portColor } from './portColors.js';

  let {
    data = {},
    selected = false,
  }: { data?: Record<string, unknown>; selected?: boolean } = $props();

  const imgColor = portColor('image');
  const numColor = portColor('number');

  const params = $derived((data.params as Record<string, unknown>) ?? {});
  const cols = $derived(Number(params.cols ?? 4));
  const rows = $derived(Number(params.rows ?? 4));
  const footerLabel = $derived(`${cols} × ${rows} grid`);
</script>

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
    <span>Flipbook Output</span>
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

  <div class="node-footer">
    <span class="footer-label">{footerLabel}</span>
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
    background: color-mix(in srgb, #a855f7 18%, var(--node-head-bg));
    border-bottom: 1px solid var(--node-border);
    border-radius: calc(var(--node-radius) - 1px) calc(var(--node-radius) - 1px) 0 0;
    white-space: nowrap;
  }

  .node-ports {
    display: flex;
    justify-content: flex-start;
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
  }
</style>
