<script lang="ts">
  import { untrack } from 'svelte';
  import type { Node, Edge } from '@xyflow/svelte';
  import { graphStore } from '../stores/graph.svelte.js';
  import { imageStore } from '../stores/images.svelte.js';
  import { IPC } from '../../shared/constants.js';
  import type { NodeGraph, NodeDefinition } from '../../shared/types.js';
  import { getNodeParams } from '../nodeEditor/nodeEditorHelpers.js';
  import { traceInputNodeId } from '../workflowUtils.js';
  import RunWorkflowDialog from './RunWorkflowDialog.svelte';

  let { definitions }: { definitions: NodeDefinition[] } = $props();
  void definitions; // reserved for future use (e.g. building complete graph)

  // ── Output node status ──────────────────────────────────────────────────────
  export type OutputNodeStatus = {
    nodeId: string;
    label: string;
    type: 'imageOutputNode';
    valid: boolean;
    reasons: string[];
  };

  const outputNodeStatuses = $derived.by((): OutputNodeStatus[] => {
    return graphStore.nodes
      .filter((n) => n.type === 'imageOutputNode')
      .map((n) => {
        const params = getNodeParams(n.data);
        const reasons: string[] = [];

        const hasImageWire = graphStore.edges.some((e) => e.target === n.id && e.targetHandle === 'in-0');
        if (!hasImageWire) reasons.push('No image input connected');

        const outputPath = (params.outputPath as string) ?? 'source';
        if (outputPath === 'custom') {
          // Check if driven by a folder path node
          const folderEdge = graphStore.edges.find((e) => e.target === n.id && e.targetHandle === 'folder-in');
          if (folderEdge) {
            const src = graphStore.nodes.find((nd) => nd.id === folderEdge.source);
            const fp = (getNodeParams(src?.data)?.folderPath as string) ?? '';
            if (!fp.trim()) reasons.push('Connected folder path node has no folder set');
          } else {
            const customPath = (params.customPath as string) ?? '';
            if (!customPath.trim()) reasons.push('Custom output folder is empty');
          }
        }

        const inputNodeId = traceInputNodeId(graphStore.nodes, graphStore.edges, n.id);
        if (hasImageWire && !inputNodeId) reasons.push('Cannot trace back to an Input node');

        const imageCount = inputNodeId ? imageStore.getImages(inputNodeId).length : 0;
        if (reasons.length === 0 && imageCount === 0) reasons.push('No images loaded for connected Input node');

        return {
          nodeId: n.id,
          label: ((n.data as Record<string, unknown>)?.label as string) ?? 'Image Output',
          type: 'imageOutputNode',
          valid: reasons.length === 0,
          reasons,
        };
      });
  });

  const validCount = $derived(outputNodeStatuses.filter((s) => s.valid).length);
  const totalCount = $derived(outputNodeStatuses.length);
  const canRun = $derived(validCount > 0 && !graphStore.batchRunning);

  // ── Dialog ──────────────────────────────────────────────────────────────────
  let showDialog = $state(false);

  // ── Elapsed timer ──────────────────────────────────────────────────────────
  let elapsed = $state(0);
  let _ticker: ReturnType<typeof setInterval> | null = null;
  $effect(() => {
    if (graphStore.batchRunning) {
      elapsed =
        graphStore.batchStartTime != null ? Math.floor((performance.now() - graphStore.batchStartTime) / 1000) : 0;
      _ticker = setInterval(() => {
        elapsed =
          graphStore.batchStartTime != null ? Math.floor((performance.now() - graphStore.batchStartTime) / 1000) : 0;
      }, 1000);
    } else {
      if (_ticker !== null) {
        clearInterval(_ticker);
        _ticker = null;
      }
    }
    return () => {
      if (_ticker !== null) {
        clearInterval(_ticker);
        _ticker = null;
      }
    };
  });

  // ── Graph serialization ─────────────────────────────────────────────────────
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

  // ── Run ─────────────────────────────────────────────────────────────────────
  async function executeValidNodes(validStatuses: OutputNodeStatus[]) {
    const graph = serializeGraph();

    for (let i = 0; i < validStatuses.length; i++) {
      const status = validStatuses[i];
      const node = graphStore.nodes.find((n) => n.id === status.nodeId);
      if (!node) continue;

      const params = getNodeParams(node.data);
      const inputNodeId = traceInputNodeId(graphStore.nodes, graphStore.edges, status.nodeId);
      if (!inputNodeId) continue;

      const imagePaths = imageStore.getImages(inputNodeId).map((img) => img.path);
      const outputPath = (params.outputPath as string) ?? 'source';
      const folderEdge = graphStore.edges.find((e) => e.target === status.nodeId && e.targetHandle === 'folder-in');
      let outputDir: string | null = null;
      if (folderEdge) {
        const src = graphStore.nodes.find((nd) => nd.id === folderEdge.source);
        outputDir = (getNodeParams(src?.data)?.folderPath as string) ?? null;
      } else if (outputPath === 'custom') {
        outputDir = (params.customPath as string) ?? null;
      }
      const overwrite = ((params.overwrite as string) ?? 'skip') as 'skip' | 'overwrite';
      const generateLog = Boolean(params.generateLog ?? false);

      graphStore.batchRunning = true;
      graphStore.batchProgress = null;
      graphStore.batchError = null;
      graphStore.batchDone = false;
      graphStore.batchStartTime = performance.now();
      graphStore.batchElapsedMs = null;
      graphStore.batchSummary = null;

      try {
        const result = (await window.ipcRenderer.invoke(
          IPC.EXECUTE_BATCH,
          graph,
          status.nodeId,
          inputNodeId,
          imagePaths,
          outputDir,
          overwrite,
          generateLog
        )) as { processed: number; skipped: number; failed: number; errors?: string[]; outputDir: string | null };
        graphStore.batchElapsedMs = performance.now() - (graphStore.batchStartTime ?? performance.now());
        graphStore.batchDone = true;
        graphStore.batchSummary = { ...result };
        graphStore.batchSummaryOpen = true;
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        if (msg !== 'CANCELLED') graphStore.batchError = msg;
      } finally {
        graphStore.batchRunning = false;
      }

      // Stop if cancelled
      if (!graphStore.batchDone && graphStore.batchError === null) break;
    }
  }

  function handleRun() {
    const valid = outputNodeStatuses.filter((s) => s.valid);
    const invalid = outputNodeStatuses.filter((s) => !s.valid);

    if (valid.length === 0) return; // button should be disabled, but guard anyway
    if (invalid.length === 0) {
      // All valid — run immediately
      void executeValidNodes(valid);
    } else {
      // Some invalid — show dialog
      showDialog = true;
    }
  }

  function handleDialogRun(statuses: OutputNodeStatus[]) {
    showDialog = false;
    void executeValidNodes(statuses.filter((s) => s.valid));
  }

  function cancelBatch() {
    window.ipcRenderer.invoke(IPC.EXECUTE_BATCH_CANCEL);
  }

  function fmtTime(s: number): string {
    if (s < 60) return `${s}s`;
    return `${Math.floor(s / 60)}m ${s % 60}s`;
  }

  const tooltipText = $derived(
    totalCount === 0
      ? 'No Image Output nodes in graph'
      : validCount === 0
        ? `0 of ${totalCount} output node${totalCount !== 1 ? 's' : ''} ready`
        : `${validCount} of ${totalCount} output node${totalCount !== 1 ? 's' : ''} ready`
  );
</script>

<div class="toolbar">
  {#if graphStore.batchRunning}
    <!-- Running state -->
    <div class="running-info">
      <span class="running-label">Running… {fmtTime(elapsed)}</span>
      {#if graphStore.batchProgress}
        {@const p = graphStore.batchProgress}
        <div class="progress-track">
          <div class="progress-fill" style="width: {Math.round((p.completed / p.total) * 100)}%"></div>
        </div>
        <span class="progress-text">{p.completed}/{p.total}</span>
      {/if}
    </div>
    <button class="btn btn--danger" onclick={cancelBatch}>Cancel</button>
  {:else}
    <!-- Idle state -->
    <button class="btn btn--primary run-center" disabled={!canRun} onclick={handleRun} title={tooltipText}> ▶ Run Workflow </button>
    {#if graphStore.batchError}
      <span class="error-label" title={graphStore.batchError}
        >Error: {graphStore.batchError.slice(0, 60)}{graphStore.batchError.length > 60 ? '…' : ''}</span
      >
    {/if}
  {/if}
</div>

{#if showDialog}
  <RunWorkflowDialog statuses={outputNodeStatuses} onRun={handleDialogRun} onCancel={() => (showDialog = false)} />
{/if}

<style>
  .toolbar {
    height: 38px;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 10px;
    background: var(--panel-header-bg);
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .run-center {
    margin: 0 auto;
  }

  .running-info {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
    min-width: 0;
  }

  .running-label {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-bright);
    white-space: nowrap;
    flex-shrink: 0;
  }

  .progress-track {
    flex: 1;
    max-width: 160px;
    height: 4px;
    background: var(--border);
    border-radius: 2px;
    overflow: hidden;
  }

  .progress-fill {
    height: 100%;
    background: #22c55e;
    border-radius: 2px;
    transition: width 0.1s ease-out;
  }

  .progress-text {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-bright);
    opacity: 0.6;
    white-space: nowrap;
    flex-shrink: 0;
  }

  .error-label {
    font-family: var(--font-mono);
    font-size: 11px;
    color: #f87171;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
    min-width: 0;
  }
</style>
