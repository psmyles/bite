<script lang="ts">
  import { untrack } from 'svelte';
  import type { Node, Edge } from '@xyflow/svelte';
  import { graphStore } from '../stores/graph.svelte.js';
  import { imageStore } from '../stores/images.svelte.js';
  import { IPC } from '../../shared/constants.js';
  import type { NodeGraph } from '../../shared/types.js';
  import { getNodeParams } from '../nodeEditor/nodeEditorHelpers.js';
  import { traceInputNodeId } from '../workflowUtils.js';
  import RunWorkflowDialog from './RunWorkflowDialog.svelte';

  type OutputNodeStatus = {
    nodeId: string;
    label: string;
    type: 'imageOutputNode' | 'textOutputNode' | 'flipbookOutputNode';
    valid: boolean;
    reasons: string[];
  };

  const outputNodeStatuses = $derived.by((): OutputNodeStatus[] => {
    const outputTypes = new Set(['imageOutputNode', 'textOutputNode', 'flipbookOutputNode']);
    return graphStore.nodes
      .filter((n) => outputTypes.has(n.type ?? ''))
      .map((n) => {
        const params = getNodeParams(n.data);
        const reasons: string[] = [];
        const nodeData = n.data as Record<string, unknown>;
        const type = n.type as 'imageOutputNode' | 'textOutputNode' | 'flipbookOutputNode';

        const hasImageWire = graphStore.edges.some((e) => e.target === n.id && e.targetHandle === 'in-0');
        if (!hasImageWire) reasons.push('No image input connected');

        if (type === 'imageOutputNode') {
          const outputPath = (params.outputPath as string) ?? 'source';
          if (outputPath === 'custom') {
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
        } else if (type === 'textOutputNode') {
          const outputPath = (params.outputPath as string) ?? '';
          if (!outputPath.trim()) reasons.push('Output file path is empty');
        } else if (type === 'flipbookOutputNode') {
          const flipbookOutputPath = (params.flipbookOutputPath as string) ?? '';
          if (!flipbookOutputPath.trim()) reasons.push('Output file path is empty');
        }

        const inputNodeId = traceInputNodeId(graphStore.nodes, graphStore.edges, n.id);
        if (hasImageWire && !inputNodeId) reasons.push('Cannot trace back to an Input node');

        const imageCount = inputNodeId ? imageStore.getImages(inputNodeId).length : 0;
        if (reasons.length === 0 && imageCount === 0) reasons.push('No images loaded for connected Input node');

        const defaultLabel =
          type === 'textOutputNode' ? 'Text Output' : type === 'flipbookOutputNode' ? 'Flipbook Output' : 'Image Output';
        return {
          nodeId: n.id,
          label: (nodeData?.label as string) ?? defaultLabel,
          type,
          valid: reasons.length === 0,
          reasons,
        };
      });
  });

  const validCount = $derived(outputNodeStatuses.filter((s) => s.valid).length);
  const totalCount = $derived(outputNodeStatuses.length);
  const canRun = $derived(validCount > 0 && !graphStore.batchRunning);

  let showDialog = $state(false);

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

      if (!graphStore.batchDone && graphStore.batchError === null) break;
    }
  }

  function handleRun() {
    const valid = outputNodeStatuses.filter((s) => s.valid);
    const invalid = outputNodeStatuses.filter((s) => !s.valid);
    if (valid.length === 0) return;
    if (invalid.length === 0) {
      void executeValidNodes(valid);
    } else {
      showDialog = true;
    }
  }

  function handleDialogRun(statuses: OutputNodeStatus[]) {
    showDialog = false;
    void executeValidNodes(statuses.filter((s) => s.valid));
  }

  // Listen for Ctrl+R from the native menu
  $effect(() => {
    const handler = () => handleRun();
    window.ipcRenderer.on(IPC.MENU_RUN_WORKFLOW, handler);
    return () => window.ipcRenderer.off(IPC.MENU_RUN_WORKFLOW, handler);
  });

  const tooltipText = $derived.by(() => {
    if (totalCount === 0) return 'No output nodes in graph';
    if (validCount === 0) {
      const allReasons = outputNodeStatuses.flatMap((s) => s.reasons);
      const unique = [...new Set(allReasons)];
      return unique.length > 0 ? unique.join(' · ') : `0 of ${totalCount} output node${totalCount !== 1 ? 's' : ''} ready`;
    }
    return `${validCount} of ${totalCount} output node${totalCount !== 1 ? 's' : ''} ready`;
  });
</script>

<button class="btn btn--primary btn--full" disabled={!canRun} onclick={handleRun} title={tooltipText}>
  Run Workflow
</button>

{#if showDialog}
  <RunWorkflowDialog statuses={outputNodeStatuses} onRun={handleDialogRun} onCancel={() => (showDialog = false)} />
{/if}
