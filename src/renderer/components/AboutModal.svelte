<script lang="ts">
  import { IS_ELECTRON } from '../platform.js';
  import { IPC } from '../../shared/constants.js';

  interface Props {
    onClose: () => void;
  }
  let { onClose }: Props = $props();

  interface RuntimeVersions {
    magick: string;
    electron: string;
    chrome: string;
    node: string;
  }

  let runtime = $state<RuntimeVersions | null>(null);

  $effect(() => {
    if (IS_ELECTRON) {
      window.ipcRenderer.invoke(IPC.GET_APP_VERSIONS).then((v) => {
        runtime = v as RuntimeVersions;
      });
    }
  });

  const deps = $derived([
    { name: 'ImageMagick', version: runtime?.magick ?? '…' },
    { name: 'Electron', version: runtime?.electron ?? '…' },
    { name: 'Chromium', version: runtime?.chrome ?? '…' },
    { name: 'Node.js', version: runtime?.node ?? '…' },
    { name: 'Svelte', version: __SVELTE_VERSION__ },
    { name: 'SvelteFlow', version: __XYFLOW_VERSION__ },
  ]);

  function onBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) onClose();
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') onClose();
  }
</script>

<svelte:window onkeydown={onKeydown} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="backdrop" onclick={onBackdropClick}>
  <div class="modal" role="dialog" aria-modal="true" aria-label="About imgplex">
    <div class="modal-header">
      <span class="modal-title">About imgplex</span>
      <button class="close-btn" onclick={onClose} aria-label="Close">
        <svg width="14" height="14" viewBox="0 0 14 14" fill="none"
          ><path d="M2 2l10 10M12 2L2 12" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" /></svg
        >
      </button>
    </div>

    <div class="modal-body">
      <p class="description">A node-based image processing tool powered by ImageMagick.</p>
      <p class="version">Version {__APP_VERSION__}</p>

      <table class="deps">
        <tbody>
          {#each deps as dep (dep.name)}
            <tr>
              <td class="dep-name">{dep.name}</td>
              <td class="dep-version">{dep.version}</td>
            </tr>
          {/each}
        </tbody>
      </table>
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

  .modal-body {
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .description {
    font-family: var(--font-ui);
    font-size: var(--font-size-base);
    color: var(--text);
    line-height: 1.5;
  }

  .version {
    font-family: var(--font-mono);
    font-size: var(--font-size-xs);
    color: var(--accent);
    margin: -8px 0 0;
  }

  .deps {
    width: 100%;
    border-collapse: collapse;
    border-top: 1px solid var(--ctx-border);
    padding-top: 4px;
    margin-top: -4px;
  }

  .deps tr:first-child td {
    padding-top: 12px;
  }

  .deps td {
    font-family: var(--font-mono);
    font-size: var(--font-size-xs);
    padding: 2px 0;
  }

  .dep-name {
    color: var(--text-muted);
    width: 50%;
  }

  .dep-version {
    color: var(--text);
  }
</style>
