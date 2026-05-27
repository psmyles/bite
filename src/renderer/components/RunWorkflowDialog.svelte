<script lang="ts">
  import type { OutputNodeStatus } from './Toolbar.svelte';

  let {
    statuses,
    onRun,
    onCancel,
  }: {
    statuses: OutputNodeStatus[];
    onRun: (statuses: OutputNodeStatus[]) => void;
    onCancel: () => void;
  } = $props();

  const validCount = $derived(statuses.filter((s) => s.valid).length);
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="backdrop" onclick={onCancel}>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="dialog" role="dialog" aria-modal="true" aria-label="Run Workflow" onclick={(e) => e.stopPropagation()}>
    <div class="dialog-header">
      <span class="dialog-title">Run Workflow</span>
    </div>

    <div class="dialog-body">
      <p class="dialog-desc">
        {validCount} of {statuses.length} output node{statuses.length !== 1 ? 's' : ''} ready to run.
        {#if validCount < statuses.length}
          Invalid nodes will be skipped.
        {/if}
      </p>

      <div class="node-list">
        {#each statuses as status}
          <div class="node-row" class:valid={status.valid} class:invalid={!status.valid}>
            <span class="node-icon">{status.valid ? '✅' : '⚠️'}</span>
            <div class="node-info">
              <span class="node-name">{status.label}</span>
              {#if !status.valid}
                <span class="node-reasons">{status.reasons.join(' · ')}</span>
              {/if}
            </div>
          </div>
        {/each}
      </div>
    </div>

    <div class="dialog-footer">
      <button class="btn-cancel" onclick={onCancel}>Cancel</button>
      <button class="btn-run" disabled={validCount === 0} onclick={() => onRun(statuses)}>
        Run {validCount} node{validCount !== 1 ? 's' : ''}
      </button>
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.65);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 2000;
  }

  .dialog {
    background: var(--ctx-bg);
    border: 2px solid var(--ctx-border);
    border-radius: var(--panel-radius);
    box-shadow: var(--ctx-shadow);
    width: 340px;
    max-height: 80vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .dialog-header {
    display: flex;
    align-items: center;
    padding: 10px 14px 9px;
    border-bottom: 2px solid var(--ctx-border);
    flex-shrink: 0;
  }

  .dialog-title {
    font-family: var(--font-ui);
    font-size: var(--font-size-base);
    font-weight: 600;
    color: var(--text-bright);
    letter-spacing: 0.04em;
  }

  .dialog-body {
    padding: 14px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    overflow-y: auto;
    flex: 1;
    min-height: 0;
  }

  .dialog-desc {
    font-family: var(--font-ui);
    font-size: 12px;
    color: var(--text);
    margin: 0;
  }

  .node-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .node-row {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: 6px 8px;
    border-radius: 4px;
    border: 1px solid transparent;
  }

  .node-row.valid {
    background: color-mix(in srgb, #22c55e 8%, transparent);
    border-color: color-mix(in srgb, #22c55e 25%, transparent);
  }

  .node-row.invalid {
    background: color-mix(in srgb, #f59e0b 8%, transparent);
    border-color: color-mix(in srgb, #f59e0b 25%, transparent);
  }

  .node-icon {
    font-size: 14px;
    flex-shrink: 0;
    line-height: 1.4;
  }

  .node-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .node-name {
    font-family: var(--font-ui);
    font-size: 12px;
    font-weight: 600;
    color: var(--text-bright);
  }

  .node-reasons {
    font-family: var(--font-mono);
    font-size: 10px;
    color: #fbbf24;
    word-break: break-word;
  }

  .dialog-footer {
    padding: 10px 14px;
    border-top: 2px solid var(--ctx-border);
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    flex-shrink: 0;
  }

  .btn-cancel {
    padding: 5px 12px;
    background: transparent;
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text);
    font-family: var(--font-ui);
    font-size: 12px;
    cursor: pointer;
    outline: none;
  }
  .btn-cancel:hover {
    border-color: var(--accent);
  }

  .btn-run {
    padding: 5px 16px;
    background: #7c3aed;
    border: none;
    border-radius: 4px;
    color: #fff;
    font-family: var(--font-ui);
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    outline: none;
    transition: opacity 0.15s;
  }
  .btn-run:hover:not(:disabled) {
    opacity: 0.85;
  }
  .btn-run:disabled {
    opacity: 0.4;
    cursor: default;
  }
</style>
