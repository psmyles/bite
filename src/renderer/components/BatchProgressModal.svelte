<script lang="ts">
  import { graphStore } from '../stores/graph.svelte.js';
  import { IPC } from '../../shared/constants.js';

  const batchRunning = $derived(graphStore.batchRunning);
  const batchProgress = $derived(graphStore.batchProgress);
  const batchError = $derived(graphStore.batchError);
  const pct = $derived(batchProgress ? Math.round((batchProgress.completed / batchProgress.total) * 100) : 0);

  let elapsed = $state(0);
  let _ticker: ReturnType<typeof setInterval> | null = null;

  $effect(() => {
    if (batchRunning) {
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

  function cancel() {
    window.ipcRenderer.invoke(IPC.EXECUTE_BATCH_CANCEL);
  }

  function fmtTime(s: number): string {
    if (s < 60) return `${s}s`;
    return `${Math.floor(s / 60)}m ${s % 60}s`;
  }
</script>

<div class="backdrop">
  <div class="modal" role="dialog" aria-modal="true" aria-label={batchError ? 'Workflow Error' : 'Running Workflow'}>
    <div class="modal-header">
      <span class="modal-title">{batchError ? 'Workflow Error' : 'Running Workflow…'}</span>
    </div>

    {#if batchError}
      <div class="modal-body">
        <div class="error-text">{batchError}</div>
      </div>
    {:else}
      <div class="modal-body">
        {#if batchProgress}
          <div class="count-row">
            <span class="count-done">{batchProgress.completed}</span>
            <span class="count-sep">/</span>
            <span class="count-total">{batchProgress.total}</span>
            <span class="count-label">images</span>
          </div>
          <div class="progress-track">
            <div class="progress-fill" style="width: {pct}%"></div>
          </div>
          <div class="pct-label">{pct}% &nbsp;·&nbsp; {fmtTime(elapsed)}</div>
        {:else}
          <div class="starting-label">Starting… {fmtTime(elapsed)}</div>
        {/if}
      </div>
    {/if}

    <div class="modal-footer">
      {#if batchError}
        <button class="btn btn--neutral" onclick={() => (graphStore.batchError = null)}>Close</button>
      {:else}
        <button class="btn btn--danger" onclick={cancel}>Cancel</button>
      {/if}
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

  .modal {
    background: var(--ctx-bg);
    border: 2px solid var(--ctx-border);
    border-radius: var(--panel-radius);
    box-shadow: var(--ctx-shadow);
    width: 340px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .modal-header {
    display: flex;
    align-items: center;
    padding: 12px 14px 11px;
    border-bottom: 2px solid var(--ctx-border);
    flex-shrink: 0;
  }

  .modal-title {
    font-family: var(--font-ui);
    font-size: var(--font-size-base);
    font-weight: 600;
    color: var(--text-bright);
    letter-spacing: 0.04em;
  }

  .modal-body {
    padding: 24px 24px 20px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .count-row {
    display: flex;
    align-items: baseline;
    gap: 5px;
  }

  .count-done {
    font-family: var(--font-mono);
    font-size: 28px;
    font-weight: 600;
    color: var(--color-success-muted);
    line-height: 1;
  }

  .count-sep {
    font-family: var(--font-mono);
    font-size: 18px;
    color: var(--text);
  }

  .count-total {
    font-family: var(--font-mono);
    font-size: 20px;
    color: var(--text-bright);
    line-height: 1;
  }

  .count-label {
    font-family: var(--font-ui);
    font-size: var(--font-size-base);
    color: var(--text);
    margin-left: 4px;
  }

  .progress-track {
    height: 6px;
    background: var(--border);
    border-radius: 3px;
    overflow: hidden;
  }

  .progress-fill {
    height: 100%;
    background: var(--color-success);
    border-radius: 3px;
    transition: width 0.15s ease-out;
  }

  .pct-label {
    font-family: var(--font-mono);
    font-size: var(--font-size-xs);
    color: var(--text);
    text-align: right;
  }

  .starting-label {
    font-family: var(--font-mono);
    font-size: var(--font-size-sm);
    color: var(--text);
    text-align: center;
    padding: 8px 0;
  }

  .error-text {
    font-family: var(--font-mono);
    font-size: var(--font-size-xs);
    color: var(--color-error-text);
    line-height: 1.5;
    word-break: break-word;
  }

  .modal-footer {
    padding: 12px 16px;
    border-top: 2px solid var(--ctx-border);
    display: flex;
    justify-content: flex-end;
  }
</style>
