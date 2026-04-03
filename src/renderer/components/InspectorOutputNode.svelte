<script lang="ts">
  import { untrack } from 'svelte'
  import type { Node } from '@xyflow/svelte'
  import { graphStore } from '../stores/graph.svelte.js'
  import { imageStore } from '../stores/images.svelte.js'
  import { IPC } from '../../shared/constants.js'
  import type { NodeGraph } from '../../shared/types.js'
  import Dropdown from './Dropdown.svelte'
  import InspectorTextOutputNode from './InspectorTextOutputNode.svelte'
  import { IS_ELECTRON } from '../platform.js'
  import { getNodeParams } from '../nodeEditor/nodeEditorHelpers.js'

  // ── Flipbook state ────────────────────────────────────────────────────────
  let flipbookGenerating = $state(false)
  let flipbookError      = $state<string | null>(null)
  let flipbookDone       = $state(false)
  let flipbookPath       = $state<string | null>(null)

  async function browseFlipbookOutput() {
    const result: string | null = await window.ipcRenderer.invoke(IPC.ATLAS_BROWSE)
    if (result) {
      graphStore.setParam(selectedNode.id, 'flipbookOutputPath', result)
      flipbookDone = false
    }
  }

  async function generateFlipbook() {
    if (flipbookGenerating) return
    const p = getNodeParams(selectedNode?.data)
    const flipbookOutputPath = (p.flipbookOutputPath as string) ?? ''
    const cols       = Number(p.cols       ?? 4)
    const rows       = Number(p.rows       ?? 4)
    const cellWidth  = Number(p.cellWidth  ?? 128)
    const cellHeight = Number(p.cellHeight ?? 128)
    const sortBy     = (p.sortBy as string) ?? 'import_order'

    if (!flipbookOutputPath.trim()) { flipbookError = 'Set an output file path first.'; return }
    if (imageStore.images.length === 0) { flipbookError = 'No images loaded.'; return }

    flipbookGenerating = true
    flipbookError      = null
    flipbookDone       = false
    flipbookPath       = null

    try {
      const imagePaths = imageStore.images.map((img) => img.path)
      const result = await window.ipcRenderer.invoke(IPC.ATLAS_GENERATE, imagePaths, {
        outputPath: flipbookOutputPath,
        rows, cols, cellWidth, cellHeight, sortBy,
        generateLog,
      }) as string
      flipbookDone = true
      flipbookPath = result
    } catch (err) {
      flipbookError = err instanceof Error ? err.message : String(err)
    } finally {
      flipbookGenerating = false
    }
  }

  function openFlipbookFolder() {
    if (!flipbookPath) return
    const dir = flipbookPath.replace(/[/\\][^/\\]+$/, '')
    window.ipcRenderer.invoke(IPC.SHELL_OPEN_PATH, dir)
  }

  let { selectedNode }: { selectedNode: Node } = $props()

  const params      = $derived(getNodeParams(selectedNode?.data))
  const outputMode  = $derived((params.outputMode as string) ?? 'image')
  const generateLog = $derived(Boolean(params.generateLog ?? false))

  // Detect a connected Folder Path node on the folder-in handle
  const folderEdge = $derived(
    graphStore.edges.find(e => e.target === selectedNode.id && e.targetHandle === 'folder-in') ?? null
  )
  const connectedFolderPath = $derived.by(() => {
    if (!folderEdge) return null
    const src = graphStore.nodes.find(n => n.id === folderEdge.source)
    if (!src) return null
    return (getNodeParams(src.data)?.folderPath as string) ?? null
  })

  async function browseFolder() {
    const folder: string | null = await window.ipcRenderer.invoke(IPC.OPEN_FOLDER_DIALOG)
    if (folder) {
      graphStore.setParam(selectedNode.id, 'outputPath', 'custom')
      graphStore.setParam(selectedNode.id, 'customPath', folder)
    }
  }

  // ── Elapsed timer + ETA ────────────────────────────────────────────────────
  let elapsed = $state(0)   // seconds, ticks while running
  let _ticker: ReturnType<typeof setInterval> | null = null

  $effect(() => {
    if (graphStore.batchRunning) {
      elapsed = graphStore.batchStartTime != null
        ? Math.floor((performance.now() - graphStore.batchStartTime) / 1000)
        : 0
      _ticker = setInterval(() => {
        elapsed = graphStore.batchStartTime != null
          ? Math.floor((performance.now() - graphStore.batchStartTime) / 1000)
          : 0
      }, 1000)
    } else {
      if (_ticker !== null) { clearInterval(_ticker); _ticker = null }
    }
    return () => { if (_ticker !== null) { clearInterval(_ticker); _ticker = null } }
  })

  function fmtTime(ms: number): string {
    const s = Math.round(ms / 1000)
    if (s < 60) return `${s}s`
    return `${Math.floor(s / 60)}m ${s % 60}s`
  }

  // ETA: estimated ms remaining based on per-image rate so far.
  // Requires at least 2 completed images to avoid noise from the first slow image.
  const eta = $derived.by(() => {
    const p = graphStore.batchProgress
    if (!p || !graphStore.batchStartTime || p.completed < 2) return null
    const elapsedMs = performance.now() - graphStore.batchStartTime
    const remaining = (p.total - p.completed) * (elapsedMs / p.completed)
    return remaining
  })

  function fmtEta(ms: number): string {
    const s = Math.round(ms / 1000)
    if (s <= 0) return 'almost done'
    if (s < 60) return `~${s}s left`
    return `~${Math.floor(s / 60)}m ${s % 60}s left`
  }

  async function runBatch() {
    if (graphStore.batchRunning) return

    const outputPath = (params.outputPath as string) ?? 'source'
    const customPath = (params.customPath as string) ?? ''
    const overwrite  = (params.overwrite  as string) ?? 'skip'

    if (connectedFolderPath !== null && !connectedFolderPath.trim()) {
      graphStore.batchError = 'The connected Folder Path node has no folder set.'
      return
    }
    if (connectedFolderPath === null && outputPath === 'custom' && !customPath.trim()) {
      graphStore.batchError = 'Set a custom folder path first.'
      return
    }

    const imagePaths = imageStore.images.map((img) => img.path)
    if (imagePaths.length === 0) {
      graphStore.batchError = 'No images loaded.'
      return
    }

    const sfNodes = $state.snapshot(untrack(() => graphStore.nodes)) as Node[]
    const sfEdges = $state.snapshot(untrack(() => graphStore.edges)) as Edge[]
    const graph: NodeGraph = {
      nodes: sfNodes.map((n) => ({
        id: n.id,
        type: n.type ?? 'process',
        position: n.position,
        data: n.data as NodeGraph['nodes'][number]['data'],
      })),
      edges: sfEdges.map((e) => ({
        id: e.id,
        source: e.source,
        sourceHandle: e.sourceHandle ?? undefined,
        target: e.target,
        targetHandle: e.targetHandle ?? undefined,
      })),
      viewport: { x: 0, y: 0, zoom: 1 },
    }

    const outputDir = connectedFolderPath !== null
      ? connectedFolderPath
      : (outputPath === 'custom' ? customPath : null)

    graphStore.batchRunning   = true
    graphStore.batchProgress  = null
    graphStore.batchError     = null
    graphStore.batchDone      = false
    graphStore.batchSummary   = null
    graphStore.batchStartTime = performance.now()
    graphStore.batchElapsedMs = null

    try {
      const result = await window.ipcRenderer.invoke(IPC.EXECUTE_BATCH, graph, imagePaths, outputDir, overwrite, generateLog) as { processed: number; skipped: number; failed: number }
      graphStore.batchDone        = true
      graphStore.batchElapsedMs   = performance.now() - (graphStore.batchStartTime ?? performance.now())
      graphStore.batchSummary     = { ...result, outputDir }
      graphStore.batchSummaryOpen = true
      if (result.failed > 0) {
        graphStore.batchError = `${result.failed} image${result.failed === 1 ? '' : 's'} failed — check the console for details`
      }
    } catch (err) {
      graphStore.batchError       = (err as Error).message
      graphStore.batchElapsedMs   = performance.now() - (graphStore.batchStartTime ?? performance.now())
      graphStore.batchDone        = true
    } finally {
      graphStore.batchRunning = false
    }
  }
</script>

<!-- Output mode selector -->
<div class="param-row">
  <span class="param-label">Output mode</span>
  <Dropdown
    value={outputMode}
    options={['image', 'text', 'flipbook']}
    labels={['Image', 'Text', 'Flipbook']}
    onchange={(v) => { graphStore.setParam(selectedNode.id, 'outputMode', v) }}
  />
</div>

{#if outputMode === 'text'}
  <InspectorTextOutputNode selectedNode={selectedNode} />

{:else if outputMode === 'flipbook'}
{@const fbParams      = params}
{@const fbOutputPath  = (fbParams.flipbookOutputPath as string) ?? ''}
{@const fbCols        = Number(fbParams.cols       ?? 4)}
{@const fbRows        = Number(fbParams.rows       ?? 4)}
{@const fbCellWidth   = Number(fbParams.cellWidth  ?? 128)}
{@const fbCellHeight  = Number(fbParams.cellHeight ?? 128)}
{@const fbSortBy      = (fbParams.sortBy as string) ?? 'import_order'}
{@const fbAtlasW      = fbCols * fbCellWidth}
{@const fbAtlasH      = fbRows * fbCellHeight}
{@const fbCellCount   = fbCols * fbRows}
{@const fbImgCount    = imageStore.images.length}
{@const fbTruncated   = fbImgCount > fbCellCount}
{@const fbUnfilled    = fbCellCount > fbImgCount}

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
        graphStore.setParam(selectedNode.id, 'flipbookOutputPath', (e.target as HTMLInputElement).value)
        flipbookDone = false
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
      value={fbRows}
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
      value={fbCellWidth}
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
      value={fbCellHeight}
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
    <input type="checkbox" checked={generateLog}
      onchange={(e) => graphStore.setParam(selectedNode.id, 'generateLog', (e.target as HTMLInputElement).checked)} />
    <span>Generate .log file</span>
  </label>
</div>

<!-- Generate button -->
{#if !IS_ELECTRON}
  <p class="web-note">Flipbook generation requires the desktop app.</p>
{/if}
<div class="gen-section" class:hidden={!IS_ELECTRON}>
  <button
    class="gen-btn"
    class:running={flipbookGenerating}
    onclick={generateFlipbook}
    disabled={flipbookGenerating}
  >
    {flipbookGenerating ? 'Generating…' : 'Generate Flipbook'}
  </button>

  {#if flipbookDone && !flipbookGenerating && flipbookPath}
    <button class="open-btn" onclick={openFlipbookFolder}>Open Output Folder</button>
  {/if}

  {#if flipbookError}
    <span class="gen-error">{flipbookError}</span>
  {/if}
</div>

{:else}

<!-- Output path -->
{#if folderEdge}
  <div class="param-row">
    <span class="param-label">Output path</span>
    <span class="connected-note">Using connected folder path</span>
  </div>
{:else}
  <div class="param-row">
    <span class="param-label">Output path</span>
    <Dropdown
      value={(params.outputPath as string) ?? 'source'}
      options={['source', 'custom']}
      labels={['Same as source', 'Custom folder']}
      onchange={(v) => { graphStore.setParam(selectedNode.id, 'outputPath', v) }}
    />
  </div>
  {#if (params.outputPath ?? 'source') === 'custom'}
    <div class="param-row">
      <span class="param-label">Folder</span>
      <div class="path-row">
        <input
          type="text"
          class="text-input path-input"
          value={(params.customPath as string) ?? ''}
          placeholder="Enter folder path…"
          oninput={(e) => graphStore.setParam(selectedNode.id, 'customPath', (e.target as HTMLInputElement).value)}
        />
        {#if IS_ELECTRON}<button class="browse-btn" onclick={browseFolder} title="Browse…">…</button>{/if}
      </div>
    </div>
  {/if}
{/if}
<!-- Overwrite mode -->
<div class="param-row">
  <span class="param-label">Overwrite mode</span>
  <Dropdown
    value={(params.overwrite as string) ?? 'skip'}
    options={['skip', 'overwrite']}
    labels={['Skip existing', 'Overwrite']}
    onchange={(v) => { graphStore.setParam(selectedNode.id, 'overwrite', v) }}
  />
</div>

<!-- Output log -->
<div class="param-row">
  <span class="param-label">Output log</span>
  <label class="log-toggle">
    <input type="checkbox" checked={generateLog}
      onchange={(e) => graphStore.setParam(selectedNode.id, 'generateLog', (e.target as HTMLInputElement).checked)} />
    <span>Generate .log file</span>
  </label>
</div>

<!-- Run Process (desktop only) -->
{#if !IS_ELECTRON}
  <p class="web-note">Processing requires the desktop app.</p>
{/if}
<div class="run-section" class:hidden={!IS_ELECTRON}>
  <button
    class="run-btn"
    class:running={graphStore.batchRunning}
    onclick={runBatch}
    disabled={graphStore.batchRunning}
  >
    {graphStore.batchRunning ? 'Processing…' : 'Run Process'}
  </button>

  {#if graphStore.batchRunning && graphStore.batchProgress}
    {@const p = graphStore.batchProgress}
    <div class="batch-progress">
      <div class="batch-bar-track">
        <div class="batch-bar-fill" style="width: {(p.completed / p.total) * 100}%"></div>
      </div>
      <div class="batch-status-row">
        <span class="batch-count">{p.completed} / {p.total}</span>
        <span class="batch-timer">{eta != null ? fmtEta(eta) : `${elapsed}s`}</span>
      </div>
      <span class="batch-filename">{p.currentFile}</span>
    </div>
  {:else if graphStore.batchRunning}
    <div class="batch-progress">
      <div class="batch-bar-track"><div class="batch-bar-fill" style="width: 0%"></div></div>
      <div class="batch-status-row">
        <span class="batch-count">Starting…</span>
        <span class="batch-timer">{elapsed}s</span>
      </div>
    </div>
  {/if}

  {#if graphStore.batchDone && !graphStore.batchRunning && graphStore.batchSummary}
    <button class="summary-btn" class:has-errors={!!graphStore.batchError} onclick={() => { graphStore.batchSummaryOpen = true }}>
      {graphStore.batchError ? 'View Summary (errors)' : 'View Summary'}
    </button>
  {/if}

  {#if graphStore.batchError}
    <span class="batch-error">{graphStore.batchError}</span>
  {/if}
</div>

{/if}

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
  .num-input::-webkit-outer-spin-button { -webkit-appearance: none; }

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

  /* ── Log toggle ── */
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

  .connected-note {
    font-family: var(--font-mono);
    font-size: 11px;
    color: #86efac;
    font-style: italic;
  }

  .hidden { display: none; }
  .web-note {
    margin: 10px 12px 0;
    font-size: 11px;
    color: var(--text-bright);
    opacity: 0.6;
    font-style: italic;
  }

  .param-row {
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding: 7px 12px;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 25%, transparent);
  }

  .param-label {
    display: flex;
    align-items: center;
    gap: 6px;
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

  .path-input {
    flex: 1;
    min-width: 0;
  }

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

  .browse-btn:hover { border-color: var(--accent); color: var(--text); }

  .run-section {
    padding: 10px 12px 12px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .run-btn {
    width: 100%;
    padding: 7px 0;
    background: #1e4d99;   /* ~8:1 contrast with white text */
    border: none;
    border-radius: 4px;
    color: #fff;
    font-family: var(--font-ui);
    font-size: 13px;
    font-weight: 600;
    line-height: 1.4;
    text-align: center;
    cursor: pointer;
    outline: none;
    transition: opacity 0.15s;
  }

  .run-btn:hover:not(:disabled) { opacity: 0.85; }
  .run-btn:disabled { opacity: 0.5; cursor: default; }
  .run-btn.running { opacity: 0.6; }

  .batch-progress {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .batch-bar-track {
    height: 3px;
    border-radius: 2px;
    background: color-mix(in srgb, var(--border) 60%, transparent);
    overflow: hidden;
  }

  .batch-bar-fill {
    height: 100%;
    background: var(--accent);
    border-radius: 2px;
    transition: width 0.2s;
  }

  .batch-status-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 6px;
  }

  .batch-timer {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text);
    flex-shrink: 0;
  }

  .batch-count {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-bright);
  }

  .batch-filename {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .summary-btn {
    width: 100%;
    padding: 5px 0;
    background: color-mix(in srgb, var(--accent) 12%, var(--panel-header-bg));
    border: 2px solid color-mix(in srgb, var(--accent) 40%, transparent);
    border-radius: 4px;
    color: var(--accent);
    font-family: var(--font-ui);
    font-size: 13px;
    line-height: 1.4;
    text-align: center;
    cursor: pointer;
    outline: none;
    transition: opacity 0.15s;
  }

  .summary-btn:hover { opacity: 0.8; }

  .summary-btn.has-errors {
    background: color-mix(in srgb, #ff7070 10%, var(--panel-header-bg));
    border-color: color-mix(in srgb, #ff7070 35%, transparent);
    color: #ff9090;
  }

  .batch-error {
    font-family: var(--font-mono);
    font-size: 11px;
    color: #ff7070;
    opacity: 0.9;
    white-space: pre-wrap;
    word-break: break-word;
  }
</style>
