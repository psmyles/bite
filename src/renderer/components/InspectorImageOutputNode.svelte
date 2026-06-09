<script lang="ts">
  import type { Node } from '@xyflow/svelte';
  import { graphStore } from '../stores/graph.svelte.js';
  import { IPC } from '../../shared/constants.js';
  import Dropdown from './Dropdown.svelte';
  import { IS_ELECTRON } from '../platform.js';
  import { getNodeParams } from '../nodeEditor/nodeEditorHelpers.js';
  import { hasSetInputInChain } from '../workflowUtils.js';

  let { selectedNode }: { selectedNode: Node } = $props();

  const params = $derived(getNodeParams(selectedNode?.data));
  const hasSetInput = $derived(hasSetInputInChain(graphStore.nodes, graphStore.edges, selectedNode.id));

  const cliName = $derived((params.cliName as string) ?? '');
  const cliNameConflict = $derived.by(() => {
    const WORKFLOW_TYPES = new Set(['inputNode', 'imageOutputNode', 'textOutputNode', 'flipbookOutputNode']);
    return (
      cliName.length > 0 &&
      graphStore.nodes.some(
        (n) =>
          n.id !== selectedNode.id &&
          WORKFLOW_TYPES.has(n.type ?? '') &&
          ((n.data as Record<string, unknown>)?.params as Record<string, unknown>)?.cliName === cliName
      )
    );
  });

  function sanitizeCliName(raw: string): string {
    return raw
      .toLowerCase()
      .replace(/\s+/g, '-')
      .replace(/[^a-z0-9-]/g, '');
  }

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

<div class="iio-inspector">
  <!-- ── CLI Name ──────────────────────────────────────────────── -->
  <div class="section">
    <div class="section-title">CLI Name</div>
    <input
      type="text"
      class="text-input"
      value={cliName}
      placeholder="e.g. output-image-1"
      oninput={(e) =>
        graphStore.setParam(selectedNode.id, 'cliName', sanitizeCliName((e.target as HTMLInputElement).value))}
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

  <!-- ── Output Path ─────────────────────────────────────────── -->
  <div class="section">
    <div class="section-title">Output Path</div>
    {#if folderEdge}
      <span class="connected-note">Using connected folder path</span>
    {:else}
      <Dropdown
        value={(params.outputPath as string) ?? 'source'}
        options={['source', 'custom']}
        labels={['Same as source', 'Custom folder']}
        onchange={(v) => graphStore.setParam(selectedNode.id, 'outputPath', v)}
      />
      {#if (params.outputPath ?? 'source') === 'custom'}
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
      {/if}
    {/if}
  </div>

  <!-- ── Overwrite ───────────────────────────────────────────── -->
  <div class="section">
    <div class="section-title">Overwrite</div>
    <Dropdown
      value={(params.overwrite as string) ?? 'skip'}
      options={['skip', 'overwrite']}
      labels={['Skip existing', 'Overwrite']}
      onchange={(v) => graphStore.setParam(selectedNode.id, 'overwrite', v)}
    />
  </div>

  <!-- ── Set Naming ──────────────────────────────────────────── -->
  {#if hasSetInput}
    <div class="section">
      <div class="section-title">Set Naming</div>
      <div class="two-col">
        <div class="field">
          <span class="field-label">Prefix</span>
          <input
            type="text"
            class="text-input"
            value={(params.setOutputPrefix as string) ?? ''}
            placeholder="e.g. T_"
            oninput={(e) =>
              graphStore.setParam(selectedNode.id, 'setOutputPrefix', (e.target as HTMLInputElement).value)}
          />
        </div>
        <div class="field">
          <span class="field-label">Suffix</span>
          <input
            type="text"
            class="text-input"
            value={(params.setOutputSuffix as string) ?? ''}
            placeholder="e.g. _ORM"
            oninput={(e) =>
              graphStore.setParam(selectedNode.id, 'setOutputSuffix', (e.target as HTMLInputElement).value)}
          />
        </div>
      </div>
    </div>
  {/if}

  <!-- ── Output Log ──────────────────────────────────────────── -->
  <div class="section">
    <div class="section-title">Output Log</div>
    <label class="log-toggle">
      <span>Generate .log file</span>
      <input
        type="checkbox"
        checked={Boolean(params.generateLog ?? false)}
        onchange={(e) => graphStore.setParam(selectedNode.id, 'generateLog', (e.target as HTMLInputElement).checked)}
      />
    </label>
  </div>
</div>

<style>
  .iio-inspector {
    display: flex;
    flex-direction: column;
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

  .connected-note {
    font-family: var(--font-mono);
    font-size: var(--font-size-xs);
    color: var(--color-success-text);
    font-style: italic;
  }

  .path-row {
    display: flex;
    gap: 6px;
    align-items: center;
  }

  .path-input {
    flex: 1;
  }
</style>
