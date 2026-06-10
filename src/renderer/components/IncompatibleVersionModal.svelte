<script lang="ts">
  import { IPC } from '../../shared/constants.js';
  import { IS_ELECTRON } from '../platform.js';

  let {
    fileVersion,
    onClose,
  }: {
    fileVersion: string | null;
    onClose: () => void;
  } = $props();

  function viewReleases() {
    const url = 'https://github.com/psmyles/imgplex/releases';
    if (IS_ELECTRON) window.ipcRenderer.invoke(IPC.SHELL_OPEN_EXTERNAL, url);
    else window.open(url, '_blank');
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="backdrop" onclick={onClose}>
  <div
    class="dialog"
    role="dialog"
    aria-modal="true"
    aria-label="Incompatible Workflow Version"
    onclick={(e) => e.stopPropagation()}
  >
    <div class="dialog-header">
      <span class="dialog-title">Incompatible Workflow Version</span>
    </div>

    <div class="dialog-body">
      {#if fileVersion}
        <p class="dialog-desc">
          This workflow was created with imgplex <strong>v{fileVersion}</strong>, which is not compatible with the
          current version (<strong>v{__APP_VERSION__}</strong>).
        </p>
      {:else}
        <p class="dialog-desc">
          This workflow was created with an older version of imgplex that does not include version information. It is
          not compatible with the current version (<strong>v{__APP_VERSION__}</strong>).
        </p>
      {/if}
      <p class="dialog-desc">Please download an older compatible version of imgplex from GitHub to open this file.</p>
    </div>

    <div class="dialog-footer">
      <button class="btn btn--neutral" onclick={onClose}>Close</button>
      <button class="btn btn--primary" onclick={viewReleases}>View Older Releases</button>
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
    width: 360px;
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
  }

  .dialog-desc {
    font-family: var(--font-ui);
    font-size: var(--font-size-sm);
    color: var(--text);
    margin: 0;
    line-height: 1.5;
  }

  .dialog-footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 10px 14px;
    border-top: 2px solid var(--ctx-border);
  }
</style>
