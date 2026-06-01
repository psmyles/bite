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

<div class="flipbook-inspector">

  <!-- ── Output File ─────────────────────────────────────────── -->
  <div class="section">
    <div class="section-title">Output File</div>
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
      {#if IS_ELECTRON}<button class="btn btn--neutral" onclick={browseFlipbookOutput} title="Browse…">…</button>{/if}
    </div>
  </div>

  <!-- ── Grid ────────────────────────────────────────────────── -->
  <div class="section">
    <div class="section-title">Grid</div>
    <div class="two-col">
      <div class="field">
        <span class="field-label">Columns</span>
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
        <span class="field-label">Rows</span>
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
    <div class="two-col">
      <div class="field">
        <span class="field-label">Cell width</span>
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
        <span class="field-label">Cell height</span>
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
  </div>

  <!-- ── Sort Order ──────────────────────────────────────────── -->
  <div class="section">
    <div class="section-title">Sort Order</div>
    <Dropdown
      value={fbSortBy}
      options={['import_order', 'name', 'name_desc']}
      labels={['Import order', 'File name (A→Z)', 'File name (Z→A)']}
      onchange={(v) => graphStore.setParam(selectedNode.id, 'sortBy', v)}
    />
  </div>

  <!-- ── Atlas Summary ───────────────────────────────────────── -->
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

  <!-- ── Output Log ──────────────────────────────────────────── -->
  <div class="section">
    <div class="section-title">Output Log</div>
    <label class="log-toggle">
      <input
        type="checkbox"
        checked={generateLog}
        onchange={(e) => graphStore.setParam(selectedNode.id, 'generateLog', (e.target as HTMLInputElement).checked)}
      />
      <span>Generate .log file</span>
    </label>
  </div>

  <!-- ── Generate ────────────────────────────────────────────── -->
  {#if !IS_ELECTRON}
    <p class="web-note">Flipbook generation requires the desktop app.</p>
  {/if}
  <div class="gen-section" class:hidden={!IS_ELECTRON}>
    <button class="btn btn--primary btn--full gen-btn" class:running={flipbookGenerating} onclick={generateFlipbook} disabled={flipbookGenerating}>
      {flipbookGenerating ? 'Generating…' : 'Generate Flipbook'}
    </button>

    {#if flipbookDone && !flipbookGenerating && flipbookPath}
      <button class="btn btn--neutral btn--full" onclick={openFlipbookFolder}>Open Output Folder</button>
    {/if}

    {#if flipbookError}
      <span class="gen-error">{flipbookError}</span>
    {/if}
  </div>

</div>

<style>
  .flipbook-inspector {
    display: flex;
    flex-direction: column;
  }

  /* ── Section layout ── */
  .section {
    padding: 10px 12px;
    border-bottom: 1px solid var(--node-border, rgba(255, 255, 255, 0.07));
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .section-title {
    font-family: var(--font-ui);
    font-size: var(--font-size-sm);
    color: var(--text-bright);
    opacity: 0.6;
    margin-bottom: 2px;
  }

  .two-col {
    display: flex;
    gap: 8px;
  }

  .field {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 5px;
    min-width: 0;
  }

  .field-label {
    font-family: var(--font-ui);
    font-size: var(--font-size-sm);
    color: var(--text-bright);
    opacity: 0.6;
    user-select: none;
  }

  .log-toggle {
    display: flex;
    align-items: center;
    gap: 6px;
    cursor: pointer;
    font-family: var(--font-ui);
    font-size: var(--font-size-sm);
    color: var(--text-bright);
    user-select: none;
  }

  /* ── Path row ── */
  .path-row {
    display: flex;
    gap: 4px;
    align-items: center;
  }

  .path-input {
    flex: 1;
    min-width: 0;
  }

  /* ── Number inputs ── */
  .num-input {
    width: 100%;
    -moz-appearance: textfield;
  }

  .num-input::-webkit-inner-spin-button,
  .num-input::-webkit-outer-spin-button {
    -webkit-appearance: none;
  }

  /* ── Atlas summary ── */
  .summary-box {
    margin: 8px 12px;
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
    font-size: var(--font-size-xs);
    color: var(--text-bright);
    opacity: 0.5;
    flex-shrink: 0;
  }

  .summary-val {
    font-family: var(--font-mono);
    font-size: var(--font-size-xs);
    color: var(--text-bright);
    text-align: right;
  }

  .summary-val.warn {
    color: var(--color-warning-text);
  }

  /* ── Generate section ── */
  .gen-section {
    padding: 10px 12px 12px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .gen-btn.running {
    opacity: 0.7;
  }

  .gen-error {
    font-family: var(--font-mono);
    font-size: var(--font-size-xs);
    color: var(--color-error-text);
    opacity: 0.9;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .web-note {
    font-family: var(--font-ui);
    font-size: var(--font-size-sm);
    color: var(--text);
    padding: 4px 12px 0;
    margin: 0;
  }

  .hidden {
    display: none;
  }
</style>
