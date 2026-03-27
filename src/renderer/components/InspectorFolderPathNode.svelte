<script lang="ts">
  import type { Node } from '@xyflow/svelte'
  import { graphStore } from '../stores/graph.svelte.js'
  import { IPC } from '../../shared/constants.js'
  import { IS_ELECTRON } from '../platform.js'
  import { getNodeParams } from '../nodeEditor/nodeEditorHelpers.js'

  let { selectedNode }: { selectedNode: Node } = $props()

  const params     = $derived(getNodeParams(selectedNode?.data))
  const folderPath = $derived((params.folderPath as string) ?? '')

  async function browseFolder() {
    const folder: string | null = await window.ipcRenderer.invoke(IPC.OPEN_FOLDER_DIALOG)
    if (folder) graphStore.setParam(selectedNode.id, 'folderPath', folder)
  }
</script>

<div class="param-row">
  <span class="param-label">Folder</span>
  <div class="path-row">
    <input
      type="text"
      class="text-input path-input"
      value={folderPath}
      placeholder="Enter folder path…"
      oninput={(e) => graphStore.setParam(selectedNode.id, 'folderPath', (e.target as HTMLInputElement).value)}
    />
    {#if IS_ELECTRON}
      <button class="browse-btn" onclick={browseFolder} title="Browse…">…</button>
    {/if}
  </div>
</div>

<style>
  .param-row {
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding: 7px 12px;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 25%, transparent);
  }

  .param-label {
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

  .path-input { flex: 1; min-width: 0; }

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
</style>
