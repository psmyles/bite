<script lang="ts">
  import type { Node } from '@xyflow/svelte';
  import { graphStore } from '../stores/graph.svelte.js';
  import { imageStore } from '../stores/images.svelte.js';
  import { IPC } from '../../shared/constants.js';
  import Dropdown from './Dropdown.svelte';
  import ColorPicker from './ColorPicker.svelte';
  import { IS_ELECTRON } from '../platform.js';
  import { getNodeParams } from '../nodeEditor/nodeEditorHelpers.js';
  import { traceInputNodeId } from '../workflowUtils.js';

  let { selectedNode }: { selectedNode: Node } = $props();

  const params = $derived(getNodeParams(selectedNode?.data));

  const cliName = $derived((params.cliName as string) ?? '');
  const cliNameConflict = $derived.by(() => {
    const WORKFLOW_TYPES = new Set(['inputNode', 'imageOutputNode', 'textOutputNode', 'flipbookOutputNode']);
    return (
      cliName.length > 0 &&
      graphStore.nodes.some(
        (n) => n.id !== selectedNode.id && WORKFLOW_TYPES.has(n.type ?? '') && ((n.data as Record<string, unknown>)?.params as Record<string, unknown>)?.cliName === cliName
      )
    );
  });

  function sanitizeCliName(raw: string): string {
    return raw.toLowerCase().replace(/\s+/g, '-').replace(/[^a-z0-9-]/g, '');
  }

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
  // bgColor stored as number[] [r, g, b, a] in 0-1 range; migrate old string values to transparent
  const fbBgColor = $derived(Array.isArray(params.bgColor) ? (params.bgColor as number[]) : [0, 0, 0, 0]);
  const fbAtlasW = $derived(fbCols * fbCellWidth);
  const fbAtlasH = $derived(fbRows * fbCellHeight);
  const fbCellCount = $derived(fbCols * fbRows);
  const fbImgCount = $derived(imagePaths.length);
  const fbTruncated = $derived(fbImgCount > fbCellCount);
  const fbUnfilled = $derived(fbCellCount > fbImgCount);
  const generateLog = $derived(Boolean(params.generateLog ?? false));

  // ── BG color wiring ───────────────────────────────────────────────────────

  const bgColorWired = $derived(
    graphStore.edges.some((e) => e.target === selectedNode.id && e.targetHandle === 'param-in-bgColor')
  );

  const activeBgColor = $derived.by((): number[] => {
    if (!bgColorWired) return fbBgColor;
    const edge = graphStore.edges.find((e) => e.target === selectedNode.id && e.targetHandle === 'param-in-bgColor');
    if (!edge) return fbBgColor;
    const src = graphStore.nodes.find((n) => n.id === edge.source);
    if (!src) return fbBgColor;
    const srcParams = getNodeParams(src.data);
    const srcParamName = (edge.sourceHandle ?? '').replace('param-out-', '');
    // value_color stores the colour in 'color'; computed outputs (rgba, rgb…) aren't in params
    const val = srcParams[srcParamName] ?? srcParams['color'];
    return Array.isArray(val) ? (val as number[]) : fbBgColor;
  });

  const bgIsTransparent = $derived((activeBgColor[3] ?? 1) < 0.01);

  function toHex(rgba: number[]): string {
    const [r = 0, g = 0, b = 0] = rgba;
    const h = (v: number) =>
      Math.round(Math.max(0, Math.min(1, v)) * 255)
        .toString(16)
        .padStart(2, '0');
    return `#${h(r)}${h(g)}${h(b)}`;
  }

  async function browseFlipbookOutput() {
    const result: string | null = await window.ipcRenderer.invoke(IPC.ATLAS_BROWSE);
    if (result) graphStore.setParam(selectedNode.id, 'flipbookOutputPath', result);
  }
</script>

<div class="flipbook-inspector">
  <!-- ── CLI Name ──────────────────────────────────────────────── -->
  <div class="section">
    <div class="section-title">CLI Name</div>
    <input
      type="text"
      class="text-input"
      value={cliName}
      placeholder="e.g. output-flipbook-1"
      oninput={(e) => graphStore.setParam(selectedNode.id, 'cliName', sanitizeCliName((e.target as HTMLInputElement).value))}
    />
    <span class="flag-hint" class:conflict={cliNameConflict}>
      {#if cliNameConflict}
        Name already used by another node
      {:else if cliName}
        Flag: --{cliName}
      {:else}
        No flag (node won't appear in exported script)
      {/if}
    </span>
  </div>

  <!-- ── Output File ─────────────────────────────────────────── -->
  <div class="section">
    <div class="section-title">Output File</div>
    <div class="path-row">
      <input
        type="text"
        class="text-input path-input"
        value={fbOutputPath}
        placeholder="Enter file path…"
        oninput={(e) =>
          graphStore.setParam(selectedNode.id, 'flipbookOutputPath', (e.target as HTMLInputElement).value)}
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

  <!-- ── Background Color ──────────────────────────────────────── -->
  <div class="section section--color">
    <div class="section-title">
      Background Color
      {#if bgColorWired}<span class="wired-badge">wired</span>{/if}
    </div>
    <ColorPicker
      value={activeBgColor}
      readonly={bgColorWired}
      onchange={(v) => graphStore.setParam(selectedNode.id, 'bgColor', v)}
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
        {#if fbUnfilled}
          — {fbCellCount - fbImgCount} cells:
          {#if bgIsTransparent}
            transparent
          {:else}
            <span class="summary-swatch" style="background:{toHex(activeBgColor)}"></span>{toHex(activeBgColor)}
          {/if}
        {/if}
      </span>
    </div>
  </div>

  <!-- ── Output Log ──────────────────────────────────────────── -->
  <div class="section">
    <div class="section-title">Output Log</div>
    <label class="log-toggle">
      <span>Generate .log file</span>
      <input
        type="checkbox"
        checked={generateLog}
        onchange={(e) => graphStore.setParam(selectedNode.id, 'generateLog', (e.target as HTMLInputElement).checked)}
      />
    </label>
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
    border-bottom: 1px solid var(--node-border);
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

  .flag-hint {
    font-family: var(--font-mono);
    font-size: var(--font-size-xs);
    color: var(--text);
    opacity: 0.6;
  }

  .flag-hint.conflict {
    color: var(--color-error-text);
    opacity: 1;
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
    justify-content: space-between;
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

  /* ── Background color ── */
  .section--color {
    padding: 10px 0;
    gap: 4px;
  }

  .section--color .section-title {
    padding: 0 12px;
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .wired-badge {
    font-family: var(--font-mono);
    font-size: var(--font-size-xs);
    color: var(--port-color-string);
    border: 1px solid var(--port-color-string);
    border-radius: 3px;
    padding: 0 3px;
    line-height: 14px;
  }

  /* ── Summary inline swatch ── */
  .summary-swatch {
    display: inline-block;
    width: 10px;
    height: 10px;
    border-radius: 2px;
    border: 1px solid color-mix(in srgb, var(--border) 60%, transparent);
    vertical-align: middle;
    margin: 0 2px 1px;
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
</style>
