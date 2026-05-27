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
  const pathColor = portColor('path');

  const params = $derived((data.params as Record<string, unknown>) ?? {});

  const outputPath = $derived((params.outputPath as string) ?? 'source');
  const customPath = $derived((params.customPath as string) ?? '');

  const footerLabel = $derived(
    outputPath === 'source'
      ? 'same folder as source'
      : outputPath === 'custom'
        ? customPath || 'no path set'
        : outputPath
  );

  // Detect a connected Folder Path node on the folder-in handle
  const folderEdge = $derived(graphStore.edges.find((e) => e.target === id && e.targetHandle === 'folder-in') ?? null);
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
  id="folder-in"
  style="background: {pathColor}; border-color: {pathColor}; top: 73px;"
/>

{#if graphStore.batchRunning}
  {@const cf = graphStore.batchProgress?.currentFile ?? ''}
  <div class="processing-badge">
    Processing
    {#if cf}<span class="processing-file">{cf}</span>{/if}
  </div>
{/if}

<div class="node" class:selected>
  <header class="node-head">
    <span>Image Output</span>
  </header>

  <div class="node-ports">
    <span class="port-tag" style="color: {imgColor}">Image</span>
  </div>
  <div class="node-ports">
    <span class="port-tag" style="color: {pathColor}">{folderEdge ? 'Folder (wired)' : 'Folder'}</span>
  </div>

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
    background: color-mix(in srgb, #f59e0b 18%, var(--node-head-bg));
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

  .processing-badge {
    position: absolute;
    bottom: calc(100% + 4px);
    top: auto;
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
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1px;
  }

  .processing-file {
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: 400;
    letter-spacing: 0;
    text-transform: none;
    color: var(--text);
    opacity: 0.75;
    max-width: 180px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
