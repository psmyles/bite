<script lang="ts">
  import type { Node } from '@xyflow/svelte';
  import { graphStore } from '../stores/graph.svelte.js';
  import { IPC } from '../../shared/constants.js';
  import Dropdown from './Dropdown.svelte';
  import { IS_ELECTRON } from '../platform.js';
  import { getNodeParams } from '../nodeEditor/nodeEditorHelpers.js';

  let { selectedNode }: { selectedNode: Node } = $props();

  const params = $derived(getNodeParams(selectedNode?.data));
  const hasSetInput = $derived(graphStore.nodes.some((n) => n.type === 'setInputNode'));

  // Detect a connected Folder Path node on the folder-in handle
  const folderEdge = $derived(
    graphStore.edges.find((e) => e.target === selectedNode.id && e.targetHandle === 'folder-in') ?? null
  );
  const connectedFolderPath = $derived.by(() => {
    if (!folderEdge) return null;
    const src = graphStore.nodes.find((n) => n.id === folderEdge.source);
    if (!src) return null;
    return (getNodeParams(src.data)?.folderPath as string) ?? null;
  });

  async function browseFolder() {
    const folder: string | null = await window.ipcRenderer.invoke(IPC.OPEN_FOLDER_DIALOG);
    if (folder) {
      graphStore.setParam(selectedNode.id, 'outputPath', 'custom');
      graphStore.setParam(selectedNode.id, 'customPath', folder);
    }
  }
</script>

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
      onchange={(v) => graphStore.setParam(selectedNode.id, 'outputPath', v)}
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
        {#if IS_ELECTRON}<button class="btn btn--neutral" onclick={browseFolder} title="Browse…">…</button>{/if}
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
    onchange={(v) => graphStore.setParam(selectedNode.id, 'overwrite', v)}
  />
</div>

<!-- Set naming (only when a Set Input node is in the graph) -->
{#if hasSetInput}
  <div class="section-divider"></div>
  <div class="section-label">Set naming</div>
  <div class="param-row two-col">
    <div class="field">
      <span class="param-label">Prefix</span>
      <input
        type="text"
        class="text-input"
        value={(params.setOutputPrefix as string) ?? ''}
        placeholder="e.g. T_"
        oninput={(e) => graphStore.setParam(selectedNode.id, 'setOutputPrefix', (e.target as HTMLInputElement).value)}
      />
    </div>
    <div class="field">
      <span class="param-label">Suffix</span>
      <input
        type="text"
        class="text-input"
        value={(params.setOutputSuffix as string) ?? ''}
        placeholder="e.g. _ORM"
        oninput={(e) => graphStore.setParam(selectedNode.id, 'setOutputSuffix', (e.target as HTMLInputElement).value)}
      />
    </div>
  </div>
{/if}

<!-- Output log -->
<div class="param-row">
  <span class="param-label">Output log</span>
  <label class="log-toggle">
    <input
      type="checkbox"
      checked={Boolean(params.generateLog ?? false)}
      onchange={(e) => graphStore.setParam(selectedNode.id, 'generateLog', (e.target as HTMLInputElement).checked)}
    />
    <span>Generate .log file</span>
  </label>
</div>

<style>
  .section-divider {
    height: 1px;
    background: var(--ctx-separator);
    margin: 6px 0;
  }

  .section-label {
    font-family: var(--font-ui);
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.07em;
    color: var(--text-muted);
    padding: 0 12px 4px;
  }

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

  .connected-note {
    font-family: var(--font-mono);
    font-size: 11px;
    color: #86efac;
    font-style: italic;
  }

  /* These classes match the global param-row styling from Inspector.svelte's content area */
  :global(.param-row) {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 5px 12px;
    gap: 8px;
    min-height: 32px;
  }

  :global(.param-label) {
    font-family: var(--font-ui);
    font-size: 12px;
    color: var(--text-bright);
    white-space: nowrap;
    flex-shrink: 0;
  }

  :global(.text-input) {
    flex: 1;
    min-width: 0;
  }

  :global(.path-row) {
    display: flex;
    gap: 4px;
    flex: 1;
    min-width: 0;
  }

  :global(.path-input) {
    flex: 1;
    min-width: 0;
  }

</style>
