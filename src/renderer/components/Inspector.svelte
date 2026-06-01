<script lang="ts">
  import { graphStore } from '../stores/graph.svelte.js';
  import type { NodeDefinition } from '../../shared/types.js';
  import InspectorInputNode from './InspectorInputNode.svelte';
  import InspectorImageOutputNode from './InspectorImageOutputNode.svelte';
  import InspectorTextOutputNode from './InspectorTextOutputNode.svelte';
  import InspectorFlipbookOutputNode from './InspectorFlipbookOutputNode.svelte';
  import InspectorParamEditor from './InspectorParamEditor.svelte';
  import InspectorCommentNode from './InspectorCommentNode.svelte';
  import InspectorRenameNode from './InspectorRenameNode.svelte';
  import InspectorResizeNode from './InspectorResizeNode.svelte';
  import InspectorFolderPathNode from './InspectorFolderPathNode.svelte';
  import InspectorSetInputNode from './InspectorSetInputNode.svelte';
  let { definitions }: { definitions: NodeDefinition[] } = $props();

  const selectedNode = $derived(graphStore.selectedNode);
  const nodeType = $derived(selectedNode?.type ?? null);
  const nodeData = $derived(selectedNode?.data as Record<string, unknown> | undefined);

  const definition = $derived(selectedNode ? (definitions.find((d) => d.id === nodeData?.definitionId) ?? null) : null);

  function nodeLabel(): string {
    if (nodeType === 'inputNode') return (nodeData?.label as string | undefined) ?? 'Input';
    if (nodeType === 'imageOutputNode') return 'Image Output';
    if (nodeType === 'textOutputNode') return 'Text Output';
    if (nodeType === 'flipbookOutputNode') return 'Flipbook Output';
    if (nodeType === 'commentNode') return 'Comment';
    if (nodeType === 'folderPathNode') return 'Folder Path';
    if (nodeType === 'setInputNode') return 'Process As Set';
    if (definition) return definition.label;
    return '';
  }
</script>

<div class="inspector">
  <div class="panel-header">
    <span>Inspector</span>
    {#if nodeType}
      <span class="node-label">{nodeLabel()}</span>
    {/if}
  </div>

  <div class="content" class:fill={nodeType === 'inputNode' || nodeType === 'textOutputNode'}>
    {#if !selectedNode}
      <span class="empty-hint">Select a node to edit its parameters.</span>
    {:else if nodeType === 'inputNode'}
      <InspectorInputNode nodeId={selectedNode.id} />
    {:else if nodeType === 'imageOutputNode'}
      <InspectorImageOutputNode {selectedNode} />
    {:else if nodeType === 'textOutputNode'}
      <InspectorTextOutputNode {selectedNode} />
    {:else if nodeType === 'flipbookOutputNode'}
      <InspectorFlipbookOutputNode {selectedNode} />
    {:else if nodeType === 'commentNode'}
      <InspectorCommentNode {selectedNode} />
    {:else if nodeType === 'folderPathNode'}
      <InspectorFolderPathNode {selectedNode} />
    {:else if nodeType === 'setInputNode'}
      <InspectorSetInputNode {selectedNode} />
    {:else if !definition}
      <span class="empty-hint">No definition found for this node.</span>
    {:else if definition.id === 'rename'}
      <InspectorRenameNode {definition} {selectedNode} />
    {:else if definition.id === 'resize'}
      <InspectorResizeNode {selectedNode} />
    {:else if definition.params.filter((p) => !p.portOnly).length === 0}
      <span class="empty-hint">This node has no parameters.</span>
    {:else}
      <InspectorParamEditor {definition} {selectedNode} />
    {/if}
  </div>
</div>

<style>
  .inspector {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    background: var(--panel-bg);
    overflow: hidden;
  }

  /* ── Header ── */
  .panel-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 10px 0 12px;
    height: 36px;
    flex-shrink: 0;
    font-family: var(--text-panel-header-family);
    font-size: var(--text-panel-header-size);
    font-weight: var(--text-panel-header-weight);
    text-transform: var(--text-panel-header-transform);
    letter-spacing: var(--text-panel-header-spacing);
    color: var(--text-bright);
    background: var(--panel-header-bg);
  }

  .node-label {
    font-family: var(--font-mono);
    font-size: var(--font-size-xs);
    color: var(--text-bright);
    opacity: 0.5;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 60%;
    text-align: right;
  }

  /* ── Scrollable content ── */
  .content {
    flex: 1;
    overflow-y: auto;
    padding: 10px 0;
    scrollbar-width: thin;
    scrollbar-color: transparent transparent;
    transition: scrollbar-color 0.2s;
  }

  .content:hover {
    scrollbar-color: var(--scrollbar-thumb) transparent;
  }

  .content.fill {
    overflow: hidden;
    padding: 0;
    display: flex;
    flex-direction: column;
  }

  .empty-hint {
    display: block;
    padding: 14px 12px;
    font-family: var(--text-hint-family);
    font-size: var(--text-hint-size);
    font-weight: var(--text-hint-weight);
    color: var(--text-bright);
    opacity: 0.5;
  }
</style>
