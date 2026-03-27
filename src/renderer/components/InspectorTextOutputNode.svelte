<script lang="ts">
  import { untrack } from 'svelte'
  import type { Node, Edge } from '@xyflow/svelte'
  import { graphStore } from '../stores/graph.svelte.js'
  import { imageStore } from '../stores/images.svelte.js'
  import { IPC } from '../../shared/constants.js'
  import type { NodeGraph } from '../../shared/types.js'
  import { getNodeParams } from '../nodeEditor/nodeEditorHelpers.js'
  import Dropdown from './Dropdown.svelte'

  let { selectedNode }: { selectedNode: Node } = $props()

  const params = $derived(getNodeParams(selectedNode?.data))

  const outputPath    = $derived((params.outputPath    as string) ?? '')
  const separatorType = $derived((params.separatorType as string) ?? 'comma')
  const customSep     = $derived((params.customSeparator as string) ?? '')
  const portIds       = $derived((params.portIds as string[]) ?? ['txo-0'])

  // Connected ports = all except the last (ghost) one that also have an incoming edge
  const connectedPortIds = $derived(
    portIds.slice(0, -1).filter((pid) =>
      graphStore.edges.some((e) => e.target === selectedNode.id && e.targetHandle === pid)
    )
  )

  // Label of the source node connected to each port
  const portLabels = $derived(
    connectedPortIds.map((portId) => {
      const edge = graphStore.edges.find((e) => e.target === selectedNode.id && e.targetHandle === portId)
      if (!edge) return '(unconnected)'
      const src = graphStore.nodes.find((n) => n.id === edge.source)
      return (src?.data as Record<string, unknown> | undefined)?.label as string ?? '(unknown)'
    })
  )

  // ── Param setters ──────────────────────────────────────────────────────────
  function setOutputPath(v: string) { graphStore.setParam(selectedNode.id, 'outputPath', v) }
  function setSeparatorType(v: string) { graphStore.setParam(selectedNode.id, 'separatorType', v) }
  function setCustomSep(v: string) { graphStore.setParam(selectedNode.id, 'customSeparator', v) }

  async function browsePath() {
    const path = await window.ipcRenderer.invoke(IPC.TEXT_OUTPUT_BROWSE) as string | null
    if (path) setOutputPath(path)
  }

  // ── Drag-to-reorder connected port blocks ──────────────────────────────────
  let dragIdx     = $state<number | null>(null)
  let dragOverPos = $state<number | null>(null)

  const displayPortIds = $derived.by(() => {
    const indexed = connectedPortIds.map((pid, i) => ({ pid, origIdx: i }))
    if (dragIdx === null || dragOverPos === null || dragIdx === dragOverPos) return indexed
    const [moved] = indexed.splice(dragIdx, 1)
    indexed.splice(dragOverPos, 0, moved)
    return indexed
  })

  function onDragStart(i: number, e: DragEvent) {
    dragIdx = i
    if (e.dataTransfer) { e.dataTransfer.effectAllowed = 'move'; e.dataTransfer.setData('text/plain', String(i)) }
  }

  function onListDragOver(e: DragEvent) {
    e.preventDefault()
    if (dragIdx === null) return
    if (e.dataTransfer) e.dataTransfer.dropEffect = 'move'
    const container = e.currentTarget as HTMLElement
    const items = Array.from(container.children) as HTMLElement[]
    let pos = items.length
    for (let i = 0; i < items.length; i++) {
      const rect = items[i].getBoundingClientRect()
      if (e.clientY < rect.top + rect.height / 2) { pos = i; break }
    }
    dragOverPos = pos
  }

  function onListDrop(e: DragEvent) {
    e.preventDefault()
    if (dragIdx !== null && dragOverPos !== null && dragIdx !== dragOverPos) {
      // Reorder the connected ports within portIds (ghost port stays last)
      const ghost = portIds[portIds.length - 1]
      const rest  = portIds.slice(0, -1)
      const next  = [...rest]
      const [moved] = next.splice(dragIdx, 1)
      next.splice(dragOverPos, 0, moved)
      graphStore.setParam(selectedNode.id, 'portIds', [...next, ghost])
    }
    dragIdx = dragOverPos = null
  }

  function onDragEnd() { dragIdx = dragOverPos = null }

  // ── Preview ────────────────────────────────────────────────────────────────
  let previewLines   = $state<string[] | null>(null)
  let previewLoading = $state(false)
  let previewTimer: ReturnType<typeof setTimeout> | null = null

  function serializeGraph(): NodeGraph {
    const sfNodes = $state.snapshot(untrack(() => graphStore.nodes)) as Node[]
    const sfEdges = $state.snapshot(untrack(() => graphStore.edges)) as Edge[]
    return {
      nodes: sfNodes.map((n) => ({
        id: n.id, type: n.type ?? 'process', position: n.position,
        data: n.data as NodeGraph['nodes'][number]['data'],
      })),
      edges: sfEdges.map((e) => ({
        id: e.id, source: e.source, sourceHandle: e.sourceHandle ?? undefined,
        target: e.target, targetHandle: e.targetHandle ?? undefined,
      })),
      viewport: { x: 0, y: 0, zoom: 1 },
    }
  }

  const PREVIEW_LIMIT = 10

  async function runPreview() {
    if (imageStore.images.length === 0 || connectedPortIds.length === 0) {
      previewLines = null
      return
    }
    previewLoading = true
    try {
      const graph = serializeGraph()
      const lines = await window.ipcRenderer.invoke(IPC.TEXT_OUTPUT_PREVIEW, {
        graph,
        imagePaths: imageStore.images.slice(0, PREVIEW_LIMIT).map((img) => img.path),
        nodeId: selectedNode.id,
      }) as string[]
      previewLines = lines
    } catch {
      previewLines = null
    } finally {
      previewLoading = false
    }
  }

  // Re-run preview whenever connected ports, separator, or image list changes
  $effect(() => {
    const _deps = [connectedPortIds.length, separatorType, customSep, imageStore.images.length]
    void _deps
    if (previewTimer) clearTimeout(previewTimer)
    previewTimer = setTimeout(runPreview, 350)
    return () => { if (previewTimer) clearTimeout(previewTimer) }
  })

  // ── Write output ───────────────────────────────────────────────────────────
  let writeRunning   = $state(false)
  let writeProgress  = $state<{ done: number; total: number } | null>(null)
  let writeElapsed   = $state(0)
  let writeError     = $state<string | null>(null)
  let writeSuccess   = $state(false)
  let lastWritten    = $state<string | null>(null)
  let _writeStartTime = 0
  let _elapsedTimer: ReturnType<typeof setInterval> | null = null

  function folderOf(filePath: string): string {
    const i = Math.max(filePath.lastIndexOf('/'), filePath.lastIndexOf('\\'))
    return i >= 0 ? filePath.slice(0, i) : filePath
  }

  async function openFolder() {
    if (lastWritten) await window.ipcRenderer.invoke(IPC.SHELL_OPEN_PATH, folderOf(lastWritten))
  }

  function startElapsedTimer() {
    _writeStartTime = performance.now()
    writeElapsed = 0
    _elapsedTimer = setInterval(() => { writeElapsed = performance.now() - _writeStartTime }, 100)
  }

  function stopElapsedTimer() {
    if (_elapsedTimer) { clearInterval(_elapsedTimer); _elapsedTimer = null }
  }

  async function handleWrite() {
    if (writeRunning) return
    writeError   = null
    writeSuccess = false

    if (!outputPath) { writeError = 'Set an output path first.'; return }
    if (imageStore.images.length === 0) { writeError = 'No images loaded.'; return }
    if (connectedPortIds.length === 0)  { writeError = 'Connect at least one input port.'; return }

    const total = imageStore.images.length
    writeRunning  = true
    writeProgress = { done: 0, total }
    startElapsedTimer()

    function onProgress(_e: unknown, prog: { done: number; total: number }) {
      writeProgress = prog
    }
    window.ipcRenderer.on(IPC.TEXT_OUTPUT_WRITE_PROGRESS, onProgress)

    try {
      const graph = serializeGraph()
      const result = await window.ipcRenderer.invoke(IPC.TEXT_OUTPUT_WRITE, {
        graph,
        imagePaths: imageStore.images.map((img) => img.path),
        nodeId: selectedNode.id,
      }) as string
      lastWritten  = result
      writeSuccess = true
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
      if (msg !== 'CANCELLED') writeError = msg
    } finally {
      writeRunning  = false
      writeProgress = null
      stopElapsedTimer()
      window.ipcRenderer.off(IPC.TEXT_OUTPUT_WRITE_PROGRESS, onProgress)
    }
  }

  async function cancelWrite() {
    await window.ipcRenderer.invoke(IPC.TEXT_OUTPUT_WRITE_CANCEL)
  }

  const SEPARATOR_OPTIONS = ['space', 'comma', 'tab', 'custom']
  const SEPARATOR_LABELS  = ['Space', 'Comma', 'Tab', 'Custom…']
</script>

<div class="txo-inspector">

  <!-- ── Output Path ─────────────────────────────────────────────────── -->
  <div class="section">
    <div class="section-title">Output File</div>
    <div class="path-row">
      <input
        class="path-input"
        type="text"
        value={outputPath}
        oninput={(e) => setOutputPath((e.target as HTMLInputElement).value)}
        placeholder="path/to/output.txt"
        spellcheck="false"
      />
      <button class="io-btn" onclick={browsePath}>Browse</button>
    </div>
  </div>

  <!-- ── Separator ──────────────────────────────────────────────────── -->
  <div class="section">
    <div class="section-title">Separator</div>
    <Dropdown
      options={SEPARATOR_OPTIONS}
      labels={SEPARATOR_LABELS}
      value={separatorType}
      onchange={setSeparatorType}
    />
    {#if separatorType === 'custom'}
      <input
        class="custom-sep-input"
        type="text"
        value={customSep}
        oninput={(e) => setCustomSep((e.target as HTMLInputElement).value)}
        placeholder="separator…"
        spellcheck="false"
      />
    {/if}
  </div>

  <!-- ── Port Order ─────────────────────────────────────────────────── -->
  <div class="section">
    <div class="section-title">Port Order</div>

    {#if connectedPortIds.length === 0}
      <div class="empty-hint">Connect nodes to the Text Output's input ports.</div>
    {:else}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="port-list" ondragover={onListDragOver} ondrop={onListDrop}>
        {#each displayPortIds as { pid, origIdx } (pid)}
          {@const isDragging = dragIdx === origIdx}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="port-row"
            class:dragging={isDragging}
            draggable={true}
            ondragstart={(e) => onDragStart(origIdx, e)}
            ondragend={onDragEnd}
          >
            <span class="drag-handle" title="Drag to reorder">
              <svg width="6" height="10" viewBox="0 0 6 10" fill="currentColor">
                <circle cx="1.5" cy="1.5" r="1.2"/><circle cx="4.5" cy="1.5" r="1.2"/>
                <circle cx="1.5" cy="5"   r="1.2"/><circle cx="4.5" cy="5"   r="1.2"/>
                <circle cx="1.5" cy="8.5" r="1.2"/><circle cx="4.5" cy="8.5" r="1.2"/>
              </svg>
            </span>
            <span class="port-label">{portLabels[origIdx] ?? pid}</span>
          </div>
        {/each}
      </div>
    {/if}
  </div>

  <!-- ── Write Button ───────────────────────────────────────────────── -->
  <div class="section write-section">
    <button
      class="write-btn"
      class:running={writeRunning}
      onclick={handleWrite}
      disabled={writeRunning}
    >
      {writeRunning ? 'Writing…' : 'Write Output'}
    </button>

    {#if writeError}
      <div class="status-msg status-msg--error">{writeError}</div>
    {:else if writeSuccess && lastWritten}
      <div class="status-msg status-msg--ok">Written: {lastWritten}</div>
      <button class="io-btn open-folder-btn" onclick={openFolder}>Open Folder</button>
    {/if}
  </div>

  <!-- ── Preview ────────────────────────────────────────────────────── -->
  <div class="section preview-section">
    <div class="section-title">
      {#if previewLines !== null}
        Preview — {imageStore.images.length > PREVIEW_LIMIT
          ? `first ${PREVIEW_LIMIT} of ${imageStore.images.length} files`
          : `${previewLines.length} line${previewLines.length !== 1 ? 's' : ''}`}
      {:else if previewLoading}
        Preview…
      {:else if imageStore.images.length === 0}
        Preview (no files loaded)
      {:else}
        Preview (connect a port)
      {/if}
    </div>

    {#if previewLines !== null && previewLines.length > 0}
      <div class="preview-list">
        {#each previewLines as line}
          <div class="preview-line">{line}</div>
        {/each}
      </div>
    {:else if previewLoading}
      <div class="preview-hint">Computing…</div>
    {:else}
      <div class="preview-hint">
        {imageStore.images.length === 0
          ? 'Load images to see a preview.'
          : 'Connect at least one port to see a preview.'}
      </div>
    {/if}
  </div>

</div>

{#if writeProgress}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="write-backdrop">
    <div class="write-modal" role="dialog" aria-modal="true" aria-label="Writing Output">
      <div class="write-modal-header">
        <span class="write-modal-title">Writing Output</span>
      </div>
      <div class="write-modal-body">
        <div class="write-count-row">
          <span class="write-count-done">{writeProgress.done}</span>
          <span class="write-count-sep">/</span>
          <span class="write-count-total">{writeProgress.total}</span>
          <span class="write-count-label">images</span>
        </div>
        <div class="write-progress-track">
          <div
            class="write-progress-fill"
            style="width: {Math.round(writeProgress.done / writeProgress.total * 100)}%"
          ></div>
        </div>
        <div class="write-pct-label">
          {Math.round(writeProgress.done / writeProgress.total * 100)}%
          &nbsp;·&nbsp;
          {(writeElapsed / 1000).toFixed(1)}s
        </div>
      </div>
      <div class="write-modal-footer">
        <button class="write-cancel-btn" onclick={cancelWrite}>Cancel</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .txo-inspector {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  .section {
    padding: 10px 12px;
    border-bottom: 1px solid var(--node-border, rgba(255,255,255,0.07));
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .section-title {
    font-family: var(--font-mono);
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-bright);
    opacity: 0.5;
    margin-bottom: 2px;
  }

  /* ── Path row ── */
  .path-row {
    display: flex;
    gap: 6px;
    align-items: center;
  }

  .path-input {
    flex: 1;
    background: var(--input-bg, rgba(255,255,255,0.06));
    border: 1px solid var(--input-border, rgba(255,255,255,0.12));
    border-radius: 3px;
    color: var(--text);
    font-family: var(--font-mono);
    font-size: 11px;
    padding: 4px 7px;
    outline: none;
    min-width: 0;
    transition: border-color 0.12s;
  }
  .path-input:focus { border-color: var(--accent); }

  .io-btn {
    flex-shrink: 0;
    background: #2e2e2e;
    border: 2px solid var(--border);
    border-radius: 4px;
    color: var(--text);
    font-family: var(--font-ui);
    font-size: 13px;
    line-height: 1.4;
    padding: 5px 10px;
    cursor: pointer;
    text-align: center;
    transition: border-color 0.1s, background 0.1s, color 0.1s;
  }
  .io-btn:hover { background: #383838; border-color: var(--accent); color: #ffffff; }

  /* ── Custom separator ── */
  .custom-sep-input {
    background: var(--input-bg, rgba(255,255,255,0.06));
    border: 1px solid var(--input-border, rgba(255,255,255,0.12));
    border-radius: 3px;
    color: var(--text);
    font-family: var(--font-mono);
    font-size: 11px;
    padding: 4px 7px;
    outline: none;
    width: 100%;
    box-sizing: border-box;
    transition: border-color 0.12s;
  }
  .custom-sep-input:focus { border-color: var(--accent); }

  /* ── Port list ── */
  .empty-hint {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-bright);
    opacity: 0.35;
    text-align: center;
    padding: 4px 0;
    font-style: italic;
  }

  .port-list { display: flex; flex-direction: column; gap: 3px; }

  .port-row {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 3px 4px 2px;
    border-radius: 3px;
    border: 1px solid transparent;
    transition: background 0.1s, border-color 0.1s;
  }
  .port-row:hover { background: rgba(255,255,255,0.04); }
  .port-row.dragging { opacity: 0.5; border-color: var(--accent); }

  .drag-handle {
    color: var(--text-bright);
    opacity: 0.3;
    cursor: grab;
    flex-shrink: 0;
    padding: 0 2px;
    line-height: 0;
  }
  .drag-handle:hover { opacity: 0.7; }

  .port-label {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
    flex: 1;
  }

  /* ── Preview ── */
  .preview-section {
    gap: 4px;
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }

  .preview-list {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    scrollbar-width: thin;
    scrollbar-color: transparent transparent;
    transition: scrollbar-color 0.2s;
  }
  .preview-list:hover { scrollbar-color: var(--scrollbar-thumb) transparent; }

  .preview-line {
    font-family: var(--font-mono);
    font-size: 11px;
    line-height: 1.6;
    color: var(--accent);
    padding: 1px 2px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    flex-shrink: 0;
  }

  .preview-hint {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-bright);
    opacity: 0.3;
    text-align: center;
    padding: 4px 0 2px;
    font-style: italic;
  }

  /* ── Write section ── */
  .write-section { gap: 8px; }

  .write-btn {
    width: 100%;
    background: #2e2e2e;
    border: 2px solid var(--border);
    border-radius: 4px;
    color: var(--text);
    font-family: var(--font-ui);
    font-size: 13px;
    line-height: 1.4;
    padding: 7px 12px;
    cursor: pointer;
    text-align: center;
    transition: border-color 0.1s, background 0.1s, color 0.1s;
  }
  .write-btn:hover:not(:disabled) { background: #383838; border-color: var(--accent); color: #ffffff; }
  .write-btn:disabled { opacity: 0.5; cursor: default; }
  .write-btn.running { border-color: var(--accent); opacity: 0.7; }

  .status-msg {
    font-family: var(--font-mono);
    font-size: 11px;
    word-break: break-all;
  }
  .status-msg--error { color: #f87171; }
  .status-msg--ok    { color: #86efac; }

  .open-folder-btn { width: 100%; }

  /* ── Write progress modal ── */
  .write-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.65);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 2000;
  }

  .write-modal {
    background: var(--ctx-bg);
    border: 2px solid var(--ctx-border);
    border-radius: var(--panel-radius);
    box-shadow: var(--ctx-shadow);
    width: 300px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .write-modal-header {
    display: flex;
    align-items: center;
    padding: 10px 14px 9px;
    border-bottom: 2px solid var(--ctx-border);
  }

  .write-modal-title {
    font-family: var(--font-ui);
    font-size: var(--font-size-base);
    font-weight: 600;
    color: var(--text-bright);
    letter-spacing: 0.04em;
  }

  .write-modal-body {
    padding: 20px 22px 16px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .write-count-row {
    display: flex;
    align-items: baseline;
    gap: 5px;
  }

  .write-count-done {
    font-family: var(--font-mono);
    font-size: 26px;
    font-weight: 600;
    color: #81c784;
    line-height: 1;
  }

  .write-count-sep {
    font-family: var(--font-mono);
    font-size: 16px;
    color: var(--text);
  }

  .write-count-total {
    font-family: var(--font-mono);
    font-size: 18px;
    color: var(--text-bright);
    line-height: 1;
  }

  .write-count-label {
    font-family: var(--font-ui);
    font-size: 13px;
    color: var(--text);
    margin-left: 4px;
  }

  .write-progress-track {
    height: 5px;
    background: var(--border);
    border-radius: 3px;
    overflow: hidden;
  }

  .write-progress-fill {
    height: 100%;
    background: #4caf50;
    border-radius: 3px;
    transition: width 0.1s ease-out;
  }

  .write-pct-label {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text);
    text-align: right;
  }

  .write-modal-footer {
    padding: 10px 14px;
    border-top: 2px solid var(--ctx-border);
    display: flex;
    justify-content: flex-end;
  }

  .write-cancel-btn {
    padding: 5px 12px;
    background: color-mix(in srgb, #c0392b 14%, var(--ctx-bg));
    border: 2px solid color-mix(in srgb, #c0392b 50%, transparent);
    border-radius: 4px;
    color: #ff9090;
    font-family: var(--font-ui);
    font-size: 12px;
    cursor: pointer;
    outline: none;
    transition: background 0.12s, border-color 0.12s;
  }

  .write-cancel-btn:hover {
    background: color-mix(in srgb, #c0392b 24%, var(--ctx-bg));
    border-color: color-mix(in srgb, #c0392b 75%, transparent);
  }
</style>
