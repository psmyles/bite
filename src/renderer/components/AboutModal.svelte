<script lang="ts">
  import { IS_ELECTRON } from '../platform.js'
  import { IPC } from '../../shared/constants.js'

  interface Props { onClose: () => void }
  let { onClose }: Props = $props()

  function onBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) onClose()
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') onClose()
  }

  function openUrl(url: string) {
    if (IS_ELECTRON) {
      window.ipcRenderer.invoke(IPC.SHELL_OPEN_EXTERNAL, url)
    } else {
      window.open(url, '_blank')
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="backdrop" onclick={onBackdropClick}>
  <div class="modal" role="dialog" aria-modal="true" aria-label="About imgplex">
    <div class="modal-header">
      <span class="modal-title">About imgplex</span>
      <button class="close-btn" onclick={onClose} aria-label="Close">✕</button>
    </div>

    <div class="modal-body">
      <p class="description">
        A node-based image processing tool powered by ImageMagick.
      </p>

      <div class="link-row">
        <button class="link-btn" onclick={() => openUrl('https://github.com/psmyles/imgplex')}>
          GitHub repository
        </button>
      </div>
      <div class="link-row">
        <button class="link-btn" onclick={() => openUrl('https://github.com/psmyles/imgplex/issues/new')}>
          Report a bug
        </button>
      </div>
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
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
    border: none;
    color: var(--text);
    font-size: 13px;
    cursor: pointer;
    padding: 2px 4px;
    border-radius: 3px;
    line-height: 1;
    transition: color 0.12s, background 0.12s;
  }

  .close-btn:hover {
    color: var(--text-bright);
    background: var(--ctx-item-hover-bg);
  }

  .modal-body {
    padding: 20px 20px 20px;
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

  .link-row {
    display: flex;
    gap: 12px;
  }

  .link-btn {
    background: none;
    border: none;
    padding: 0;
    font-family: var(--font-ui);
    font-size: var(--font-size-base);
    color: var(--ctx-text);
    cursor: pointer;
    text-align: left;
    transition: color 0.12s;
  }

  .link-btn:hover {
    color: var(--text-bright);
    text-decoration: underline;
  }
</style>
