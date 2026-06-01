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
            <span class="node-icon" class:valid={status.valid} class:invalid={!status.valid}></span>
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
      <button class="btn btn--neutral" onclick={onCancel}>Cancel</button>
      <button class="btn btn--primary" disabled={validCount === 0} onclick={() => onRun(statuses)}>
        Run {validCount} node{validCount !== 1 ? 's' : ''}
      </button>
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: var(--modal-overlay-bg);
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
    font-size: var(--font-size-sm);
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
    background: color-mix(in srgb, var(--color-success) 8%, transparent);
    border-color: color-mix(in srgb, var(--color-success) 25%, transparent);
  }

  .node-row.invalid {
    background: color-mix(in srgb, var(--color-warning) 8%, transparent);
    border-color: color-mix(in srgb, var(--color-warning) 25%, transparent);
  }

  .node-icon {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
    margin-top: 3px;
  }
  .node-icon.valid {
    background: var(--color-success);
  }
  .node-icon.invalid {
    background: var(--color-warning);
    border-radius: 2px;
  }

  .node-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .node-name {
    font-family: var(--font-ui);
    font-size: var(--font-size-sm);
    font-weight: 600;
    color: var(--text-bright);
  }

  .node-reasons {
    font-family: var(--font-mono);
    font-size: var(--font-size-xxs);
    color: var(--color-warning-text);
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

</style>
