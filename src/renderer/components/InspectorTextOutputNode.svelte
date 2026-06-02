<script lang="ts">
  import { untrack } from 'svelte';
  import type { Node, Edge } from '@xyflow/svelte';
  import { graphStore } from '../stores/graph.svelte.js';
  import { imageStore } from '../stores/images.svelte.js';
  import { IPC } from '../../shared/constants.js';
  import type { NodeGraph } from '../../shared/types.js';
  import { getNodeParams } from '../nodeEditor/nodeEditorHelpers.js';
  import { traceInputNodeId } from '../workflowUtils.js';
  import Dropdown from './Dropdown.svelte';

  let { selectedNode }: { selectedNode: Node } = $props();

  // Find which input node's images to use for preview and write
  const tracedInputNodeId = $derived(traceInputNodeId(graphStore.nodes, graphStore.edges, selectedNode.id));
  const activeImages = $derived(tracedInputNodeId ? imageStore.getImages(tracedInputNodeId) : imageStore.images);

  const params = $derived(getNodeParams(selectedNode?.data));

  const outputPath = $derived((params.outputPath as string) ?? '');
  const generateLog = $derived(Boolean(params.generateLog ?? false));
  const usePreviewForProcessing = $derived(Boolean(params.usePreviewForProcessing ?? false));
  const separatorType = $derived((params.separatorType as string) ?? 'comma');
  const customSep = $derived((params.customSeparator as string) ?? '');
  const portIds = $derived((params.portIds as string[]) ?? ['txo-0']);

  // Connected ports = all except the last (ghost) one that also have an incoming edge
  const connectedPortIds = $derived(
    portIds
      .slice(0, -1)
      .filter((pid) => graphStore.edges.some((e) => e.target === selectedNode.id && e.targetHandle === pid))
  );

  // Label of the source node connected to each port
  const portLabels = $derived(
    connectedPortIds.map((portId) => {
      const edge = graphStore.edges.find((e) => e.target === selectedNode.id && e.targetHandle === portId);
      if (!edge) return '(unconnected)';
      const src = graphStore.nodes.find((n) => n.id === edge.source);
      return ((src?.data as Record<string, unknown> | undefined)?.label as string) ?? '(unknown)';
    })
  );

  // ── Param setters ──────────────────────────────────────────────────────────
  function setOutputPath(v: string) {
    graphStore.setParam(selectedNode.id, 'outputPath', v);
  }
  function setSeparatorType(v: string) {
    graphStore.setParam(selectedNode.id, 'separatorType', v);
  }
  function setCustomSep(v: string) {
    graphStore.setParam(selectedNode.id, 'customSeparator', v);
  }

  async function browsePath() {
    const path = (await window.ipcRenderer.invoke(IPC.TEXT_OUTPUT_BROWSE)) as string | null;
    if (path) setOutputPath(path);
  }

  // ── Drag-to-reorder connected port blocks ──────────────────────────────────
  let dragIdx = $state<number | null>(null);
  let dragOverPos = $state<number | null>(null);

  const displayPortIds = $derived.by(() => {
    const indexed = connectedPortIds.map((pid, i) => ({ pid, origIdx: i }));
    if (dragIdx === null || dragOverPos === null || dragIdx === dragOverPos) return indexed;
    const [moved] = indexed.splice(dragIdx, 1);
    indexed.splice(dragOverPos, 0, moved);
    return indexed;
  });

  function onDragStart(i: number, e: DragEvent) {
    dragIdx = i;
    if (e.dataTransfer) {
      e.dataTransfer.effectAllowed = 'move';
      e.dataTransfer.setData('text/plain', String(i));
    }
  }

  function onListDragOver(e: DragEvent) {
    e.preventDefault();
    if (dragIdx === null) return;
    if (e.dataTransfer) e.dataTransfer.dropEffect = 'move';
    const container = e.currentTarget as HTMLElement;
    const items = Array.from(container.children) as HTMLElement[];
    let pos = items.length;
    for (let i = 0; i < items.length; i++) {
      const rect = items[i].getBoundingClientRect();
      if (e.clientY < rect.top + rect.height / 2) {
        pos = i;
        break;
      }
    }
    dragOverPos = pos;
  }

  function onListDrop(e: DragEvent) {
    e.preventDefault();
    if (dragIdx !== null && dragOverPos !== null && dragIdx !== dragOverPos) {
      // Reorder the connected ports within portIds (ghost port stays last)
      const ghost = portIds[portIds.length - 1];
      const rest = portIds.slice(0, -1);
      const next = [...rest];
      const [moved] = next.splice(dragIdx, 1);
      next.splice(dragOverPos, 0, moved);
      graphStore.setParam(selectedNode.id, 'portIds', [...next, ghost]);
    }
    dragIdx = dragOverPos = null;
  }

  function onDragEnd() {
    dragIdx = dragOverPos = null;
  }

  // ── Preview ────────────────────────────────────────────────────────────────
  let previewLines = $state<string[] | null>(null);
  let previewLoading = $state(false);
  let previewTimer: ReturnType<typeof setTimeout> | null = null;

  function serializeGraph(): NodeGraph {
    const sfNodes = $state.snapshot(untrack(() => graphStore.nodes)) as Node[];
    const sfEdges = $state.snapshot(untrack(() => graphStore.edges)) as Edge[];
    return {
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
    };
  }

  const PREVIEW_LIMIT = 10;

  async function runPreview() {
    if (activeImages.length === 0 || connectedPortIds.length === 0) {
      previewLines = null;
      return;
    }
    previewLoading = true;
    try {
      const graph = serializeGraph();
      const lines = (await window.ipcRenderer.invoke(IPC.TEXT_OUTPUT_PREVIEW, {
        graph,
        imagePaths: activeImages.slice(0, PREVIEW_LIMIT).map((img) => img.path),
        nodeId: selectedNode.id,
      })) as string[];
      previewLines = lines;
    } catch {
      previewLines = null;
    } finally {
      previewLoading = false;
    }
  }

  // Re-run preview whenever connected ports, separator, or image list changes
  $effect(() => {
    const _deps = [connectedPortIds.length, separatorType, customSep, activeImages.length];
    void _deps;
    if (previewTimer) clearTimeout(previewTimer);
    previewTimer = setTimeout(runPreview, 350);
    return () => {
      if (previewTimer) clearTimeout(previewTimer);
    };
  });

  const SEPARATOR_OPTIONS = ['space', 'comma', 'tab', 'custom'];
  const SEPARATOR_LABELS = ['Space', 'Comma', 'Tab', 'Custom…'];
</script>

<div class="txo-inspector">
  <!-- ── Output Path ─────────────────────────────────────────────────── -->
  <div class="section">
    <div class="section-title">Output File</div>
    <div class="path-row">
      <input
        class="text-input path-input"
        type="text"
        value={outputPath}
        oninput={(e) => setOutputPath((e.target as HTMLInputElement).value)}
        placeholder="path/to/output.txt"
        spellcheck="false"
      />
      <button class="btn btn--neutral" onclick={browsePath}>Browse</button>
    </div>
  </div>

  <!-- ── Separator ──────────────────────────────────────────────────── -->
  <div class="section">
    <div class="section-title">Separator</div>
    <Dropdown options={SEPARATOR_OPTIONS} labels={SEPARATOR_LABELS} value={separatorType} onchange={setSeparatorType} />
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
                <circle cx="1.5" cy="1.5" r="1.2" /><circle cx="4.5" cy="1.5" r="1.2" />
                <circle cx="1.5" cy="5" r="1.2" /><circle cx="4.5" cy="5" r="1.2" />
                <circle cx="1.5" cy="8.5" r="1.2" /><circle cx="4.5" cy="8.5" r="1.2" />
              </svg>
            </span>
            <span class="port-label">{portLabels[origIdx] ?? pid}</span>
          </div>
        {/each}
      </div>
    {/if}
  </div>

  <!-- ── Output Log ─────────────────────────────────────────────────── -->
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

  <!-- ── Processing Source ─────────────────────────────────────────── -->
  <div class="section">
    <div class="section-title">Processing Source</div>
    <label class="log-toggle">
      <span>Use preview image for processing</span>
      <input
        type="checkbox"
        checked={usePreviewForProcessing}
        onchange={(e) =>
          graphStore.setParam(selectedNode.id, 'usePreviewForProcessing', (e.target as HTMLInputElement).checked)}
      />
    </label>
  </div>

  <!-- ── Preview ────────────────────────────────────────────────────── -->
  <div class="section preview-section">
    <div class="section-title">
      {#if previewLines !== null}
        Preview — {activeImages.length > PREVIEW_LIMIT
          ? `first ${PREVIEW_LIMIT} of ${activeImages.length} files`
          : `${previewLines.length} line${previewLines.length !== 1 ? 's' : ''}`}
      {:else if previewLoading}
        Preview…
      {:else if activeImages.length === 0}
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
        {activeImages.length === 0 ? 'Load images to see a preview.' : 'Connect at least one port to see a preview.'}
      </div>
    {/if}
  </div>
</div>

<style>
  .txo-inspector {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

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

  /* ── Path row ── */
  .path-row {
    display: flex;
    gap: 6px;
    align-items: center;
  }

  .path-input {
    flex: 1;
    min-width: 0;
  }

  /* ── Custom separator ── */
  .custom-sep-input {
    background: var(--input-bg);
    border: 1px solid var(--input-border);
    border-radius: 3px;
    color: var(--text);
    font-family: var(--font-mono);
    font-size: var(--font-size-xs);
    padding: 4px 7px;
    outline: none;
    width: 100%;
    box-sizing: border-box;
    transition: border-color 0.12s;
  }
  .custom-sep-input:focus {
    border-color: var(--accent);
  }

  /* ── Port list ── */
  .empty-hint {
    font-family: var(--font-mono);
    font-size: var(--font-size-xs);
    color: var(--text-bright);
    opacity: 0.35;
    text-align: center;
    padding: 4px 0;
    font-style: italic;
  }

  .port-list {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .port-row {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 3px 4px 2px;
    border-radius: 3px;
    border: 1px solid transparent;
    transition:
      background 0.1s,
      border-color 0.1s;
  }
  .port-row:hover {
    background: var(--item-hover-bg);
  }
  .port-row.dragging {
    opacity: 0.5;
    border-color: var(--accent);
  }

  .drag-handle {
    color: var(--text-bright);
    opacity: 0.3;
    cursor: grab;
    flex-shrink: 0;
    padding: 0 2px;
    line-height: 0;
  }
  .drag-handle:hover {
    opacity: 0.7;
  }

  .port-label {
    font-family: var(--font-mono);
    font-size: var(--font-size-xs);
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
  .preview-list:hover {
    scrollbar-color: var(--scrollbar-thumb) transparent;
  }

  .preview-line {
    font-family: var(--font-mono);
    font-size: var(--font-size-xs);
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
    font-size: var(--font-size-xs);
    color: var(--text-bright);
    opacity: 0.3;
    text-align: center;
    padding: 4px 0 2px;
    font-style: italic;
  }

  /* ── Log toggle ── */
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
</style>
