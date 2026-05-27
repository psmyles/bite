<script lang="ts">
  import type { Node } from '@xyflow/svelte';
  import { graphStore } from '../stores/graph.svelte.js';
  import { imageStore } from '../stores/images.svelte.js';
  import { IPC } from '../../shared/constants.js';
  import Dropdown from './Dropdown.svelte';
  import { IS_ELECTRON } from '../platform.js';
  import { getNodeParams } from '../nodeEditor/nodeEditorHelpers.js';
  import { traceInputNodeId } from '../workflowUtils.js';

  let { selectedNode }: { selectedNode: Node } = $props();

  const params = $derived(getNodeParams(selectedNode?.data));

  // Find which input node feeds this flipbook output
  const inputNodeId = $derived(traceInputNodeId(graphStore.nodes, graphStore.edges, selectedNode.id));
  const imagePaths = $derived(
    inputNodeId ? imageStore.getImages(inputNodeId).map((img) => img.path) : imageStore.images.map((img) => img.path)
  );

  const fbOutputPath = $derived((params.flipbookOutputPath as string) ?? '');
  const fbCols = $derived(Number(params.cols ?? 4));
  const fbRows = $derived(Number(params.rows ?? 4));
  const fbCellWidth = $derived(Number(params.cellWidth ?? 128));
  const fbCellHeight = $derived(Number(params.cellHeight ?? 128));
  const fbSortBy = $derived((params.sortBy as string) ?? 'import_order');
  const fbAtlasW = $derived(fbCols * fbCellWidth);
  const fbAtlasH = $derived(fbRows * fbCellHeight);
  const fbCellCount = $derived(fbCols * fbRows);
  const fbImgCount = $derived(imagePaths.length);
  const fbTruncated = $derived(fbImgCount > fbCellCount);
  const fbUnfilled = $derived(fbCellCount > fbImgCount);
  const generateLog = $derived(Boolean(params.generateLog ?? false));

  let flipbookGenerating = $state(false);
  let flipbookError = $state<string | null>(null);
  let flipbookDone = $state(false);
  let flipbookPath = $state<string | null>(null);

  async function browseFlipbookOutput() {
    const result: string | null = await window.ipcRenderer.invoke(IPC.ATLAS_BROWSE);
    if (result) {
      graphStore.setParam(selectedNode.id, 'flipbookOutputPath', result);
      flipbookDone = false;
    }
  }

  async function generateFlipbook() {
    if (flipbookGenerating) return;
    if (!fbOutputPath.trim()) {
      flipbookError = 'Set an output file path first.';
      return;
    }
    if (imagePaths.length === 0) {
      flipbookError = 'No images loaded.';
      return;
    }

    flipbookGenerating = true;
    flipbookError = null;
    flipbookDone = false;
    flipbookPath = null;
    try {
      const result = (await window.ipcRenderer.invoke(IPC.ATLAS_GENERATE, imagePaths, {
        outputPath: fbOutputPath,
        rows: fbRows,
        cols: fbCols,
        cellWidth: fbCellWidth,
        cellHeight: fbCellHeight,
        sortBy: fbSortBy,
        generateLog,
      })) as string;
      flipbookDone = true;
      flipbookPath = result;
    } catch (err) {
      flipbookError = err instanceof Error ? err.message : String(err);
    } finally {
      flipbookGenerating = false;
    }
  }

  function openFlipbookFolder() {
    if (!flipbookPath) return;
    const dir = flipbookPath.replace(/[/\\][^/\\]+$/, '');
    window.ipcRenderer.invoke(IPC.SHELL_OPEN_PATH, dir);
  }
</script>

<!-- Output file -->
<div class="param-row">
  <span class="param-label">Output file</span>
  <div class="path-row">
    <input
      type="text"
      class="text-input path-input"
      value={fbOutputPath}
      placeholder="Enter file path…"
      oninput={(e) => {
        graphStore.setParam(selectedNode.id, 'flipbookOutputPath', (e.target as HTMLInputElement).value);
        flipbookDone = false;
      }}
    />
    {#if IS_ELECTRON}<button class="browse-btn" onclick={browseFlipbookOutput} title="Browse…">…</button>{/if}
  </div>
</div>

<!-- Grid dimensions -->
<div class="param-row two-col">
  <div class="field">
    <span class="param-label">Columns</span>
    <input
      type="number"
      class="text-input num-input"
      value={fbCols}
      min="1"
      max="64"
      oninput={(e) => {
        const v = parseInt((e.target as HTMLInputElement).value, 10);
        if (!isNaN(v) && v >= 1) graphStore.setParam(selectedNode.id, 'cols', v);
      }}
    />
  </div>
  <div class="field">
    <span class="param-label">Rows</span>
    <input
      type="number"
      class="text-input num-input"
      value={fbRows}
      min="1"
      max="64"
      oninput={(e) => {
        const v = parseInt((e.target as HTMLInputElement).value, 10);
        if (!isNaN(v) && v >= 1) graphStore.setParam(selectedNode.id, 'rows', v);
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
      value={fbCellWidth}
      min="1"
      max="4096"
      oninput={(e) => {
        const v = parseInt((e.target as HTMLInputElement).value, 10);
        if (!isNaN(v) && v >= 1) graphStore.setParam(selectedNode.id, 'cellWidth', v);
      }}
    />
  </div>
  <div class="field">
    <span class="param-label">Cell height</span>
    <input
      type="number"
      class="text-input num-input"
      value={fbCellHeight}
      min="1"
      max="4096"
      oninput={(e) => {
        const v = parseInt((e.target as HTMLInputElement).value, 10);
        if (!isNaN(v) && v >= 1) graphStore.setParam(selectedNode.id, 'cellHeight', v);
      }}
    />
  </div>
</div>

<!-- Sort order -->
<div class="param-row">
  <span class="param-label">Sort order</span>
  <Dropdown
    value={fbSortBy}
    options={['import_order', 'name', 'name_desc']}
    labels={['Import order', 'File name (A→Z)', 'File name (Z→A)']}
    onchange={(v) => graphStore.setParam(selectedNode.id, 'sortBy', v)}
  />
</div>

<!-- Atlas summary -->
<div class="summary-box">
  <div class="summary-row">
    <span class="summary-key">Atlas size</span>
    <span class="summary-val">{fbAtlasW} × {fbAtlasH} px</span>
  </div>
  <div class="summary-row">
    <span class="summary-key">Cells</span>
    <span class="summary-val">{fbCellCount} ({fbCols} × {fbRows})</span>
  </div>
  <div class="summary-row">
    <span class="summary-key">Images</span>
    <span class="summary-val" class:warn={fbTruncated || fbUnfilled}>
      {fbImgCount} loaded
      {#if fbTruncated}— {fbImgCount - fbCellCount} will be truncated{/if}
      {#if fbUnfilled}— {fbCellCount - fbImgCount} cells will be transparent{/if}
    </span>
  </div>
</div>

<!-- Output log -->
<div class="param-row">
  <span class="param-label">Output log</span>
  <label class="log-toggle">
    <input
      type="checkbox"
      checked={generateLog}
      onchange={(e) => graphStore.setParam(selectedNode.id, 'generateLog', (e.target as HTMLInputElement).checked)}
    />
    <span>Generate .log file</span>
  </label>
</div>

<!-- Generate button -->
{#if !IS_ELECTRON}
  <p class="web-note">Flipbook generation requires the desktop app.</p>
{/if}
<div class="gen-section" class:hidden={!IS_ELECTRON}>
  <button class="gen-btn" class:running={flipbookGenerating} onclick={generateFlipbook} disabled={flipbookGenerating}>
    {flipbookGenerating ? 'Generating…' : 'Generate Flipbook'}
  </button>

  {#if flipbookDone && !flipbookGenerating && flipbookPath}
    <button class="open-btn" onclick={openFlipbookFolder}>Open Output Folder</button>
  {/if}

  {#if flipbookError}
    <span class="gen-error">{flipbookError}</span>
  {/if}
</div>

<style>
  .two-col {
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

  .num-input {
    width: 100%;
    -moz-appearance: textfield;
  }

  .num-input::-webkit-inner-spin-button,
  .num-input::-webkit-outer-spin-button {
    -webkit-appearance: none;
  }

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

  .summary-val.warn {
    color: #fbbf24;
  }

  .log-toggle {
    display: flex;
    align-items: center;
    gap: 6px;
    cursor: pointer;
    font-family: var(--font-ui);
    font-size: 12px;
    color: var(--text-bright);
    user-select: none;
  }

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

  .gen-btn:hover:not(:disabled) {
    opacity: 0.85;
  }
  .gen-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .gen-btn.running {
    opacity: 0.6;
  }

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

  .open-btn:hover {
    opacity: 0.8;
  }

  .gen-error {
    font-family: var(--font-mono);
    font-size: 11px;
    color: #ff7070;
    opacity: 0.9;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .web-note {
    font-family: var(--font-ui);
    font-size: 12px;
    color: var(--text-muted);
    padding: 4px 12px 0;
    margin: 0;
  }

  .hidden {
    display: none;
  }
</style>
