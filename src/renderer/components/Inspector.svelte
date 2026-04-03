<script lang="ts">
  import { graphStore } from '../stores/graph.svelte.js'
  import type { NodeDefinition } from '../../shared/types.js'
  import InspectorInputNode from './InspectorInputNode.svelte'
  import InspectorOutputNode from './InspectorOutputNode.svelte'
  import InspectorParamEditor from './InspectorParamEditor.svelte'
  import InspectorCommentNode from './InspectorCommentNode.svelte'
  import InspectorRenameNode from './InspectorRenameNode.svelte'
  import InspectorResizeNode from './InspectorResizeNode.svelte'
  import InspectorFolderPathNode from './InspectorFolderPathNode.svelte'
  import { getNodeParams } from '../nodeEditor/nodeEditorHelpers.js'

  let { definitions }: { definitions: NodeDefinition[] } = $props()

  const selectedNode = $derived(graphStore.selectedNode)
  const nodeType     = $derived(selectedNode?.type ?? null)
  const nodeData     = $derived(selectedNode?.data as Record<string, unknown> | undefined)

  const definition = $derived(
    selectedNode
      ? definitions.find((d) => d.id === nodeData?.definitionId) ?? null
      : null
  )

  const outputNodeMode = $derived(
    nodeType === 'outputNode'
      ? ((nodeData?.params as Record<string, unknown> | undefined)?.outputMode as string) ?? 'image'
      : null
  )
</script>

<div class="inspector">
  <div class="panel-header">
    <span>Inspector</span>
    {#if definition}
      <span class="node-label">{definition.label}</span>
    {:else if nodeType === 'inputNode'}
      <span class="node-label">{nodeData?.label as string}</span>
    {:else if nodeType === 'outputNode'}
      <span class="node-label">
        {outputNodeMode === 'text' ? 'Output — Text' : outputNodeMode === 'flipbook' ? 'Output — Flipbook' : 'Output — Image'}
      </span>
    {:else if nodeType === 'commentNode'}
      <span class="node-label">Comment</span>
    {:else if nodeType === 'folderPathNode'}
      <span class="node-label">Folder Path</span>
    {/if}
  </div>

  <div class="content" class:fill={nodeType === 'inputNode' || outputNodeMode === 'text'}>
    {#if !selectedNode}
      <span class="empty-hint">Select a node to edit its parameters.</span>

    {:else if nodeType === 'inputNode'}
      <InspectorInputNode />

    {:else if nodeType === 'outputNode'}
      <InspectorOutputNode selectedNode={selectedNode} />

    {:else if nodeType === 'commentNode'}
      <InspectorCommentNode selectedNode={selectedNode} />

    {:else if nodeType === 'folderPathNode'}
      <InspectorFolderPathNode selectedNode={selectedNode} />

    {:else if !definition}
      <span class="empty-hint">No definition found for this node.</span>
    {:else if definition.id === 'rename'}
      <InspectorRenameNode {definition} selectedNode={selectedNode} />
    {:else if definition.id === 'resize'}
      <InspectorResizeNode selectedNode={selectedNode} />
    {:else if definition.params.filter((p) => !p.portOnly).length === 0}
      <span class="empty-hint">This node has no parameters.</span>
    {:else}
      <InspectorParamEditor {definition} selectedNode={selectedNode} />
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
    font-size: 11px;
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

  .content:hover { scrollbar-color: var(--scrollbar-thumb) transparent; }

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
