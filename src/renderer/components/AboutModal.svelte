<script lang="ts">
  interface Props { onClose: () => void }
  let { onClose }: Props = $props()

  function onBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) onClose()
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') onClose()
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

      <p class="version">Version {__APP_VERSION__}</p>
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

  .version {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--accent);
    margin: -8px 0 0;
  }
</style>
