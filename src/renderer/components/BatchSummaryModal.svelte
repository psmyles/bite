<script lang="ts">
  import { graphStore } from '../stores/graph.svelte.js';
  import { IPC } from '../../shared/constants.js';
  import { IS_ELECTRON } from '../platform.js';

  interface Props {
    onClose: () => void;
  }
  let { onClose }: Props = $props();

  const summary = $derived(graphStore.batchSummary!);
  const elapsedMs = $derived(graphStore.batchElapsedMs);

  function fmtTime(ms: number): string {
    const s = Math.round(ms / 1000);
    if (s < 60) return `${s}s`;
    return `${Math.floor(s / 60)}m ${s % 60}s`;
  }

  function onBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) onClose();
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') onClose();
  }

  function openOutputFolder() {
    const dir = summary.outputDir;
    if (!dir || !IS_ELECTRON) return;
    window.ipcRenderer.invoke(IPC.SHELL_OPEN_PATH, dir);
  }
</script>

<svelte:window onkeydown={onKeydown} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="backdrop" onclick={onBackdropClick}>
  <div class="modal" role="dialog" aria-modal="true" aria-label="Batch Summary">
    <div class="modal-header">
      <span class="modal-title">Batch Complete</span>
      <button class="close-btn" onclick={onClose} aria-label="Close">×</button>
    </div>

    <div class="modal-body">
      <div class="stats">
        <div class="stat">
          <span class="stat-value success">{summary.processed}</span>
          <span class="stat-label">Processed</span>
        </div>
        {#if summary.skipped > 0}
          <div class="stat-divider"></div>
          <div class="stat">
            <span class="stat-value muted">{summary.skipped}</span>
            <span class="stat-label">Skipped</span>
          </div>
        {/if}
        {#if summary.failed > 0}
          <div class="stat-divider"></div>
          <div class="stat">
            <span class="stat-value error">{summary.failed}</span>
            <span class="stat-label">Failed</span>
          </div>
        {/if}
      </div>

      {#if elapsedMs != null}
        <div class="time-row">
          <span class="time-label">Total time</span>
          <span class="time-value">{fmtTime(elapsedMs)}</span>
        </div>
      {/if}

      {#if summary.failed > 0 && summary.errors?.length}
        <div class="error-list">
          {#each summary.errors as err}
            <div class="error-item">{err}</div>
          {/each}
        </div>
      {:else if summary.failed > 0}
        <p class="error-note">Check the DevTools console for per-image error details.</p>
      {/if}
    </div>

    <div class="modal-footer">
      {#if IS_ELECTRON && summary.outputDir}
        <button class="btn btn--neutral" onclick={openOutputFolder}>Open Output Folder</button>
      {/if}
      <button class="btn btn--neutral" onclick={onClose}>Close</button>
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
    width: 320px;
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
    flex: 1;
  }

  .close-btn {
    background: none;
    border: var(--modal-close-btn-border-width) solid var(--border);
    color: var(--text);
    font-size: var(--font-size-base);
    cursor: pointer;
    width: var(--modal-close-btn-size);
    height: var(--modal-close-btn-size);
    padding: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: var(--modal-close-btn-radius);
    line-height: 1;
    flex-shrink: 0;
    transition:
      color 0.12s,
      background 0.12s,
      border-color 0.12s;
  }

  .close-btn:hover {
    color: var(--text-bright);
    background: var(--ctx-item-hover-bg);
    border-color: var(--accent);
  }

  .modal-body {
    padding: 20px 20px 16px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  /* ── Stat row ── */
  .stats {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0;
  }

  .stat {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    flex: 1;
  }

  .stat-divider {
    width: 1px;
    height: 40px;
    background: var(--ctx-border);
    flex-shrink: 0;
  }

  .stat-value {
    font-family: var(--font-mono);
    font-size: 28px;
    font-weight: 600;
    line-height: 1;
  }

  .stat-value.success {
    color: var(--color-success-muted);
  }
  .stat-value.muted {
    color: var(--text);
  }
  .stat-value.error {
    color: var(--color-error-text);
  }

  .stat-label {
    font-family: var(--font-ui);
    font-size: var(--font-size-xs);
    color: var(--text);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  /* ── Time row ── */
  .time-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding-top: 4px;
    border-top: 2px solid var(--ctx-border);
  }

  .time-label {
    font-family: var(--font-ui);
    font-size: var(--font-size-sm);
    color: var(--text);
  }

  .time-value {
    font-family: var(--font-mono);
    font-size: var(--font-size-sm);
    color: var(--text-bright);
  }

  .error-note {
    font-family: var(--font-ui);
    font-size: var(--font-size-xs);
    color: var(--color-error-text);
    line-height: 1.4;
  }

  .error-list {
    display: flex;
    flex-direction: column;
    gap: 3px;
    max-height: 120px;
    overflow-y: auto;
    border: 1px solid var(--color-error-border);
    border-radius: 4px;
    padding: 6px 8px;
    background: var(--color-error-bg);
  }

  .error-item {
    font-family: var(--font-mono);
    font-size: var(--font-size-xxs);
    color: var(--color-error-text);
    line-height: 1.4;
    word-break: break-all;
  }

  /* ── Footer ── */
  .modal-footer {
    display: flex;
    gap: 8px;
    padding: 12px 16px;
    border-top: 2px solid var(--ctx-border);
    justify-content: flex-end;
  }
</style>
