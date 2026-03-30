<script lang="ts">
  import type { Node } from '@xyflow/svelte'
  import { graphStore } from '../stores/graph.svelte.js'
  import { imageStore } from '../stores/images.svelte.js'
  import { IPC } from '../../shared/constants.js'
  import Dropdown from './Dropdown.svelte'
  import { IS_ELECTRON } from '../platform.js'
  import { getNodeParams } from '../nodeEditor/nodeEditorHelpers.js'

  let { selectedNode }: { selectedNode: Node } = $props()

  const params = $derived(getNodeParams(selectedNode?.data))

  const outputPath  = $derived((params.outputPath  as string)  ?? '')
  const cols        = $derived(Number(params.cols        ?? 4))
  const rows        = $derived(Number(params.rows        ?? 4))
  const cellWidth   = $derived(Number(params.cellWidth   ?? 128))
  const cellHeight  = $derived(Number(params.cellHeight  ?? 128))
  const sortBy      = $derived((params.sortBy      as string)  ?? 'import_order')

  // Atlas dimensions summary
  const atlasWidth  = $derived(cols * cellWidth)
  const atlasHeight = $derived(rows * cellHeight)
  const cellCount   = $derived(cols * rows)

  // Image count summary
  const imageCount = $derived(imageStore.images.length)
  const usedCount  = $derived(Math.min(imageCount, cellCount))
  const truncated  = $derived(imageCount > cellCount)
  const unfilled   = $derived(cellCount > imageCount)

  // ── Generation state ────────────────────────────────────────────────────────
  let generating = $state(false)
  let genError   = $state<string | null>(null)
  let genDone    = $state(false)
  let genPath    = $state<string | null>(null)

  async function browseOutput() {
    const result: string | null = await window.ipcRenderer.invoke(IPC.ATLAS_BROWSE)
    if (result) graphStore.setParam(selectedNode.id, 'outputPath', result)
  }

  async function generate() {
    if (generating) return
    if (!outputPath.trim()) { genError = 'Set an output file path first.'; return }
    if (imageCount === 0)   { genError = 'No images loaded.'; return }

    generating = true
    genError   = null
    genDone    = false
    genPath    = null

    try {
      const imagePaths = imageStore.images.map((img) => img.path)
      const result = await window.ipcRenderer.invoke(IPC.ATLAS_GENERATE, imagePaths, {
        outputPath,
        rows,
        cols,
        cellWidth,
        cellHeight,
        sortBy,
      }) as string
      genDone = true
      genPath = result
    } catch (err) {
      genError = err instanceof Error ? err.message : String(err)
    } finally {
      generating = false
    }
  }

  function openOutputFolder() {
    if (!genPath) return
    const dir = genPath.replace(/[/\\][^/\\]+$/, '')
    window.ipcRenderer.invoke(IPC.SHELL_OPEN_PATH, dir)
  }
</script>

<!-- Output file -->
<div class="param-row">
  <span class="param-label">Output file</span>
  <div class="path-row">
    <input
      type="text"
      class="text-input path-input"
      value={outputPath}
      placeholder="Enter file path…"
      oninput={(e) => {
        graphStore.setParam(selectedNode.id, 'outputPath', (e.target as HTMLInputElement).value)
        genDone = false
      }}
    />
    {#if IS_ELECTRON}<button class="browse-btn" onclick={browseOutput} title="Browse…">…</button>{/if}
  </div>
</div>

<!-- Grid dimensions -->
<div class="param-row two-col">
  <div class="field">
    <span class="param-label">Columns</span>
    <input
      type="number"
      class="text-input num-input"
      value={cols}
      min="1" max="64"
      oninput={(e) => {
        const v = parseInt((e.target as HTMLInputElement).value, 10)
        if (!isNaN(v) && v >= 1) graphStore.setParam(selectedNode.id, 'cols', v)
      }}
    />
  </div>
  <div class="field">
    <span class="param-label">Rows</span>
    <input
      type="number"
      class="text-input num-input"
      value={rows}
      min="1" max="64"
      oninput={(e) => {
        const v = parseInt((e.target as HTMLInputElement).value, 10)
        if (!isNaN(v) && v >= 1) graphStore.setParam(selectedNode.id, 'rows', v)
      }}
    />
  </div>
</div>

<!-- Cell dimensions -->
<div class="param-row two-col">
  <div class="field">
    <span class="param-label">Cell width</span>
    <input
      type="number"
      class="text-input num-input"
      value={cellWidth}
      min="1" max="4096"
      oninput={(e) => {
        const v = parseInt((e.target as HTMLInputElement).value, 10)
        if (!isNaN(v) && v >= 1) graphStore.setParam(selectedNode.id, 'cellWidth', v)
      }}
    />
  </div>
  <div class="field">
    <span class="param-label">Cell height</span>
    <input
      type="number"
      class="text-input num-input"
      value={cellHeight}
      min="1" max="4096"
      oninput={(e) => {
        const v = parseInt((e.target as HTMLInputElement).value, 10)
        if (!isNaN(v) && v >= 1) graphStore.setParam(selectedNode.id, 'cellHeight', v)
      }}
    />
  </div>
</div>

<!-- Sort order -->
<div class="param-row">
  <span class="param-label">Sort order</span>
  <Dropdown
    value={sortBy}
    options={['import_order', 'name']}
    labels={['Import order', 'File name (A→Z)']}
    onchange={(v) => graphStore.setParam(selectedNode.id, 'sortBy', v)}
  />
</div>

<!-- Atlas summary -->
<div class="summary-box">
  <div class="summary-row">
    <span class="summary-key">Atlas size</span>
    <span class="summary-val">{atlasWidth} × {atlasHeight} px</span>
  </div>
  <div class="summary-row">
    <span class="summary-key">Cells</span>
    <span class="summary-val">{cellCount} ({cols} × {rows})</span>
  </div>
  <div class="summary-row">
    <span class="summary-key">Images</span>
    <span class="summary-val" class:warn={truncated || unfilled}>
      {imageCount} loaded
      {#if truncated}— {imageCount - cellCount} will be truncated{/if}
      {#if unfilled}— {cellCount - imageCount} cells will be transparent{/if}
    </span>
  </div>
</div>

<!-- Generate button -->
{#if IS_ELECTRON}
<div class="gen-section">
  <button
    class="gen-btn"
    class:running={generating}
    onclick={generate}
    disabled={generating}
  >
    {generating ? 'Generating…' : 'Generate Atlas'}
  </button>

  {#if genDone && !generating && genPath}
    <button class="open-btn" onclick={openOutputFolder}>Open Output Folder</button>
  {/if}

  {#if genError}
    <span class="gen-error">{genError}</span>
  {/if}
</div>
{:else}
  <p class="web-note">Atlas generation requires the desktop app.</p>
{/if}

<style>
  .param-row {
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding: 7px 12px;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 25%, transparent);
  }

  .param-row.two-col {
    flex-direction: row;
    gap: 8px;
  }

  .field {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 5px;
    min-width: 0;
  }

  .param-label {
    font-family: var(--font-ui);
    font-size: 12px;
    color: var(--text-bright);
    opacity: 0.6;
    user-select: none;
  }

  .path-row {
    display: flex;
    gap: 4px;
    align-items: center;
  }

  .path-input { flex: 1; min-width: 0; }

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

  .num-input {
    width: 100%;
    -moz-appearance: textfield;
  }

  .num-input::-webkit-inner-spin-button,
  .num-input::-webkit-outer-spin-button { -webkit-appearance: none; }

  .browse-btn {
    flex-shrink: 0;
    width: 28px;
    height: 28px;
    background: var(--panel-header-bg);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text-bright);
    font-size: 13px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    outline: none;
  }

  .browse-btn:hover { border-color: var(--accent); }

  /* ── Summary box ── */
  .summary-box {
    margin: 8px 12px 2px;
    background: color-mix(in srgb, var(--border) 20%, transparent);
    border: 1px solid color-mix(in srgb, var(--border) 40%, transparent);
    border-radius: 4px;
    padding: 7px 9px;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .summary-row {
    display: flex;
    justify-content: space-between;
    gap: 8px;
  }

  .summary-key {
    font-family: var(--font-ui);
    font-size: 11px;
    color: var(--text-bright);
    opacity: 0.5;
    flex-shrink: 0;
  }

  .summary-val {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-bright);
    text-align: right;
  }

  .summary-val.warn { color: #fbbf24; }

  /* ── Generate section ── */
  .gen-section {
    padding: 10px 12px 12px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .gen-btn {
    width: 100%;
    padding: 7px 0;
    background: #7c3aed;
    border: none;
    border-radius: 4px;
    color: #fff;
    font-family: var(--font-ui);
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    outline: none;
    transition: opacity 0.15s;
  }

  .gen-btn:hover:not(:disabled) { opacity: 0.85; }
  .gen-btn:disabled { opacity: 0.5; cursor: default; }
  .gen-btn.running { opacity: 0.6; }

  .open-btn {
    width: 100%;
    padding: 5px 0;
    background: color-mix(in srgb, #7c3aed 12%, var(--panel-header-bg));
    border: 2px solid color-mix(in srgb, #7c3aed 40%, transparent);
    border-radius: 4px;
    color: #a78bfa;
    font-family: var(--font-ui);
    font-size: 13px;
    cursor: pointer;
    outline: none;
    transition: opacity 0.15s;
  }

  .open-btn:hover { opacity: 0.8; }

  .gen-error {
    font-family: var(--font-mono);
    font-size: 11px;
    color: #ff7070;
    opacity: 0.9;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .web-note {
    margin: 10px 12px 0;
    font-size: 11px;
    color: var(--text-bright);
    opacity: 0.6;
    font-style: italic;
  }
</style>
