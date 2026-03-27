<script lang="ts">
  interface Props {
    message: string
    onConfirm: () => void
    onCancel: () => void
  }
  let { message, onConfirm, onCancel }: Props = $props()

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') onCancel()
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="backdrop">
  <div class="modal" role="alertdialog" aria-modal="true" aria-label="Confirm">
    <div class="modal-header">
      <span class="modal-title">imgplex</span>
    </div>
    <div class="modal-body">
      <p class="message">{message}</p>
      <div class="buttons">
        <!-- Cancel is first in DOM so it receives autofocus — it is the safe default -->
        <button class="btn btn-primary" autofocus onclick={onCancel}>Cancel</button>
        <button class="btn btn-ghost" onclick={onConfirm}>OK</button>
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
    z-index: 3000;
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
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  .message {
    font-family: var(--font-ui);
    font-size: var(--font-size-base);
    color: var(--text);
    line-height: 1.5;
    margin: 0;
  }

  .buttons {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }

  .buttons .btn {
    flex: 1;
  }

  .btn {
    font-family: var(--font-ui);
    font-size: var(--font-size-base);
    padding: 5px 16px;
    border-radius: 4px;
    border: 2px solid transparent;
    cursor: pointer;
    text-align: center;
    line-height: 1.4;
    transition: background 0.12s, border-color 0.12s, color 0.12s;
  }

  /* Cancel — visually primary so it draws the eye */
  .btn-primary {
    background: var(--accent);
    color: #000;
    border-color: var(--accent);
  }

  .btn-primary:hover {
    filter: brightness(1.12);
  }

  /* OK — ghost/secondary so it reads as the less-recommended action */
  .btn-ghost {
    background: transparent;
    color: var(--text);
    border-color: var(--ctx-border);
  }

  .btn-ghost:hover {
    background: var(--ctx-item-hover-bg);
    color: var(--text-bright);
  }
</style>
