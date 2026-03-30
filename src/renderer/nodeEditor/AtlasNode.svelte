<script lang="ts">
  let { data = {}, selected = false }: { data?: Record<string, unknown>; selected?: boolean } = $props()

  const params      = $derived((data.params as Record<string, unknown>) ?? {})
  const cols        = $derived(Number(params.cols        ?? 4))
  const rows        = $derived(Number(params.rows        ?? 4))
  const cellWidth   = $derived(Number(params.cellWidth   ?? 128))
  const cellHeight  = $derived(Number(params.cellHeight  ?? 128))
  const outputPath  = $derived((params.outputPath as string) ?? '')
  const outputName  = $derived(outputPath ? (outputPath.split(/[/\\]/).pop() ?? outputPath) : '')

  const totalPx = $derived(`${cols * cellWidth} × ${rows * cellHeight} px`)
</script>

<div class="node" class:selected>
  <header class="node-head">
    <span>Flipbook Atlas</span>
  </header>

  <div class="node-body">
    <div class="info-row">
      <span class="grid-label">{cols} × {rows}</span>
      <span class="sep">·</span>
      <span class="dim-label">{cellWidth}×{cellHeight} px/cell</span>
    </div>
    <div class="info-row">
      {#if outputName}
        <span class="file-label">{outputName}</span>
      {:else}
        <span class="file-label empty">no output file set</span>
      {/if}
    </div>
  </div>
</div>

<style>
  .node {
    background: var(--node-bg);
    border: 1px solid var(--node-border);
    border-radius: var(--node-radius);
    min-width: 210px;
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
    background: color-mix(in srgb, #f97316 18%, var(--node-head-bg));
    border-bottom: 1px solid var(--node-border);
    border-radius: calc(var(--node-radius) - 1px) calc(var(--node-radius) - 1px) 0 0;
    white-space: nowrap;
  }

  .node-body {
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 6px 10px 7px;
  }

  .info-row {
    display: flex;
    align-items: center;
    gap: 5px;
    min-height: 18px;
  }

  .grid-label {
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--text-bright);
    font-weight: 600;
  }

  .sep {
    font-size: 11px;
    color: var(--text-bright);
    opacity: 0.4;
  }

  .dim-label {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-bright);
    opacity: 0.7;
  }

  .file-label {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-bright);
    opacity: 0.6;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 190px;
  }

  .file-label.empty {
    opacity: 0.3;
    font-style: italic;
  }
</style>
