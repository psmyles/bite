<script lang="ts">
  import { imageStore } from '../stores/images.svelte.js'
  import { onMount } from 'svelte'

  const progress = $derived(imageStore.importProgress)
  const done     = $derived(imageStore.importDone)
  const pct      = $derived(progress ? Math.round(progress.done / progress.total * 100) : 100)

  let elapsed = $state(0)
  let timer: ReturnType<typeof setInterval> | null = null

  onMount(() => {
    timer = setInterval(() => { if (progress) elapsed += 100 }, 100)
    return () => { if (timer) clearInterval(timer) }
  })

  function formatDuration(ms: number): string {
    const totalSec = ms / 1000
    if (totalSec < 60) return `${totalSec.toFixed(1)} sec`
    const minutes = Math.floor(totalSec / 60)
    const secs    = (totalSec - minutes * 60).toFixed(1)
    return `${minutes} ${minutes === 1 ? 'minute' : 'minutes'} ${secs} sec`
  }

  const doneLabel = $derived(
    imageStore.lastImportMs !== null
      ? `Imported ${imageStore.lastImportCount} ${imageStore.lastImportCount === 1 ? 'image' : 'images'} in ${formatDuration(imageStore.lastImportMs)}`
      : ''
  )
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="backdrop">
  <div class="modal" role="dialog" aria-modal="true"
    aria-label={done ? 'Import Complete' : 'Importing Images'}>
    <div class="modal-header">
      <span class="modal-title">{done ? 'Import Complete' : 'Importing Images'}</span>
    </div>

    {#if done}
      <div class="modal-body done-body">
        <div class="done-icon">✓</div>
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
        <button class="ok-btn" onclick={() => imageStore.dismissImport()}>OK</button>
      {:else}
        <button class="cancel-btn" onclick={() => imageStore.cancelImport()}>Cancel Import</button>
      {/if}
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
    color: #81c784;
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
    font-size: 13px;
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
    background: #4caf50;
    border-radius: 3px;
    transition: width 0.15s ease-out;
  }

  .pct-label {
    font-family: var(--font-mono);
    font-size: 11px;
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
    font-size: 28px;
    color: #81c784;
    line-height: 1;
  }

  .done-label {
    font-family: var(--font-mono);
    font-size: 13px;
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

  .cancel-btn {
    padding: 6px 14px;
    background: color-mix(in srgb, #c0392b 14%, var(--ctx-bg));
    border: 2px solid color-mix(in srgb, #c0392b 50%, transparent);
    border-radius: 4px;
    color: #ff9090;
    font-family: var(--font-ui);
    font-size: 12px;
    cursor: pointer;
    outline: none;
    transition: background 0.12s, border-color 0.12s;
  }

  .cancel-btn:hover {
    background: color-mix(in srgb, #c0392b 24%, var(--ctx-bg));
    border-color: color-mix(in srgb, #c0392b 75%, transparent);
  }

  .ok-btn {
    padding: 6px 20px;
    background: color-mix(in srgb, #4caf50 14%, var(--ctx-bg));
    border: 2px solid color-mix(in srgb, #4caf50 50%, transparent);
    border-radius: 4px;
    color: #81c784;
    font-family: var(--font-ui);
    font-size: 12px;
    cursor: pointer;
    outline: none;
    transition: background 0.12s, border-color 0.12s;
  }

  .ok-btn:hover {
    background: color-mix(in srgb, #4caf50 24%, var(--ctx-bg));
    border-color: color-mix(in srgb, #4caf50 75%, transparent);
  }
</style>
