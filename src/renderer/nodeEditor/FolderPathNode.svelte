<script lang="ts">
  import { Handle, Position } from '@xyflow/svelte'
  import { portColor } from './portColors.js'

  let { data = {}, selected = false }: { data?: Record<string, unknown>; selected?: boolean } = $props()

  const pathColor  = portColor('path')
  const params     = $derived((data.params as Record<string, unknown>) ?? {})
  const folder     = $derived((params.folderPath as string) ?? '')
  const folderName = $derived(folder ? (folder.split(/[/\\]/).pop() ?? folder) : '')
</script>

<!-- Folder path output handle -->
<Handle
  type="source"
  position={Position.Right}
  id="out-0"
  style="background: {pathColor}; border-color: {pathColor};"
/>

<div class="node" class:selected>
  <header class="node-head">
    <span>Folder Path</span>
  </header>

  <div class="node-body">
    <span class="port-tag" style="color: {pathColor}">path</span>
    {#if folderName}
      <span class="folder-name">{folderName}</span>
    {:else}
      <span class="folder-name empty">not set</span>
    {/if}
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
    background: color-mix(in srgb, #86efac 18%, var(--node-head-bg));
    border-bottom: 1px solid var(--node-border);
    border-radius: calc(var(--node-radius) - 1px) calc(var(--node-radius) - 1px) 0 0;
    white-space: nowrap;
  }

  .node-body {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 10px;
    min-height: 30px;
    overflow: hidden;
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
    flex-shrink: 0;
  }

  .folder-name {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--node-text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }

  .folder-name.empty {
    color: var(--text-bright);
    opacity: 0.4;
    font-style: italic;
  }
</style>
