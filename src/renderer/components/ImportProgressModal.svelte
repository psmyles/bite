<script lang="ts">
  import { imageStore } from '../stores/images.svelte.js';
  import { onMount } from 'svelte';

  const progress = $derived(imageStore.importProgress);
  const done = $derived(imageStore.importDone);
  const pct = $derived(progress ? Math.round((progress.done / progress.total) * 100) : 100);

  let elapsed = $state(0);
  let timer: ReturnType<typeof setInterval> | null = null;

  onMount(() => {
    timer = setInterval(() => {
      if (progress) elapsed += 100;
    }, 100);
    return () => {
      if (timer) clearInterval(timer);
    };
  });

  function formatDuration(ms: number): string {
    const totalSec = ms / 1000;
    if (totalSec < 60) return `${totalSec.toFixed(1)} sec`;
    const minutes = Math.floor(totalSec / 60);
    const secs = (totalSec - minutes * 60).toFixed(1);
    return `${minutes} ${minutes === 1 ? 'minute' : 'minutes'} ${secs} sec`;
  }

  const doneLabel = $derived(
    imageStore.lastImportMs !== null
      ? `Imported ${imageStore.lastImportCount} ${imageStore.lastImportCount === 1 ? 'image' : 'images'} in ${formatDuration(imageStore.lastImportMs)}`
      : ''
  );
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="backdrop">
  <div class="modal" role="dialog" aria-modal="true" aria-label={done ? 'Import Complete' : 'Importing Images'}>
    <div class="modal-header">
      <span class="modal-title">{done ? 'Import Complete' : 'Importing Images'}</span>
    </div>

    {#if done}
      <div class="modal-body done-body">
        <div class="done-icon"></div>
        <div class="done-label">{doneLabel}</div>
      </div>
    {:else if progress}
      <div class="modal-body">
        <div class="count-row">
          <span class="count-done">{progress.done}</span>
          <span class="count-sep">/</span>
          <span class="count-total">{progress.total}</span>
          <span class="count-label">images</span>
        </div>

        <div class="progress-track">
          <div class="progress-fill" style="width: {pct}%"></div>
        </div>

        <div class="pct-label">{pct}% &nbsp;·&nbsp; {(elapsed / 1000).toFixed(1)}s</div>
      </div>
    {/if}

    <div class="modal-footer">
      {#if done}
        <button class="btn btn--primary" onclick={() => imageStore.dismissImport()}>OK</button>
      {:else}
        <button class="btn btn--danger" onclick={() => imageStore.cancelImport()}>Cancel Import</button>
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

  /* ── In-progress view ── */
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

  /* ── Done view ── */
  .done-body {
    align-items: center;
    padding: 28px 24px 24px;
    gap: 10px;
  }

  .done-icon {
    width: 32px;
    height: 32px;
    border-radius: 50%;
    background: color-mix(in srgb, var(--color-success) 20%, transparent);
    border: 2px solid var(--color-success);
    flex-shrink: 0;
  }

  .done-label {
    font-family: var(--font-mono);
    font-size: var(--font-size-base);
    color: var(--text-bright);
    text-align: center;
  }

  /* ── Footer ── */
  .modal-footer {
    padding: 12px 16px;
    border-top: 2px solid var(--ctx-border);
    display: flex;
    justify-content: flex-end;
  }
</style>
