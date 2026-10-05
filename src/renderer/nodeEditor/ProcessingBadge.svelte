<script lang="ts">
  import { graphStore } from '../stores/graph.svelte.js';

  let { nodeId }: { nodeId: string } = $props();

  const show = $derived(graphStore.batchRunning && graphStore.batchRunningNodeId === nodeId);
</script>

{#if show}
  {@const cf = graphStore.batchProgress?.currentFile ?? ''}
  <div class="processing-badge">
    Processing
    {#if cf}<span class="processing-file">{cf}</span>{/if}
  </div>
{/if}

<style>
  .processing-badge {
    position: absolute;
    bottom: calc(100% + 4px);
    top: auto;
    left: 50%;
    transform: translateX(-50%);
    font-family: var(--font-ui);
    font-size: var(--font-size-xs);
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--badge-processing-color);
    background: color-mix(in srgb, var(--bg) 85%, transparent);
    border: 1px solid var(--badge-processing-color);
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
    font-size: var(--font-size-xxs);
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
