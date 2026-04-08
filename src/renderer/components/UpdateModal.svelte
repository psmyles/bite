<script lang="ts">
  import { IS_ELECTRON } from '../platform.js'
  import { IPC } from '../../shared/constants.js'

  export type UpdateState =
    | { status: 'checking' }
    | { status: 'update';  version: string; body: string; url: string }
    | { status: 'latest';  version: string; body: string; url: string }
    | { status: 'error' }

  interface Props {
    state:   UpdateState
    onClose: () => void
  }
  let { state, onClose }: Props = $props()

  function onBackdropClick(e: MouseEvent) {
    if (state.status === 'checking') return  // prevent close while loading
    if (e.target === e.currentTarget) onClose()
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && state.status !== 'checking') onClose()
  }

  function openReleasePage() {
    if (state.status !== 'update' && state.status !== 'latest') return
    const url = state.url
    if (IS_ELECTRON) window.ipcRenderer.invoke(IPC.SHELL_OPEN_EXTERNAL, url)
    else window.open(url, '_blank')
    onClose()
  }

  const titles: Record<UpdateState['status'], string> = {
    checking: 'Checking for Updates',
    update:   'Update Available',
    latest:   'You\'re up to date',
    error:    'Update Check Failed',
  }
</script>

<svelte:window onkeydown={onKeydown} />

<!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
<div class="backdrop" onclick={onBackdropClick}>
  <div class="modal" role="dialog" aria-modal="true" aria-label={titles[state.status]}>
    <div class="modal-header">
      <span class="modal-title">{titles[state.status]}</span>
      {#if state.status !== 'checking'}
        <button class="close-btn" onclick={onClose} aria-label="Close">✕</button>
      {/if}
    </div>

    <div class="modal-body">

      {#if state.status === 'checking'}
        <div class="checking-row">
          <span class="spinner"></span>
          <span class="checking-text">Contacting GitHub…</span>
        </div>

      {:else if state.status === 'update'}
        <p class="tagline">
          A new version is available: <span class="version">{state.version}</span>
        </p>
        <div class="actions">
          <button class="btn-update" onclick={openReleasePage}>Update</button>
          <button class="btn-later" onclick={onClose}>Later</button>
        </div>
        {#if state.body}
          <div class="release-notes">
            <p class="notes-label">Release notes</p>
            <pre class="notes-body">{state.body}</pre>
          </div>
        {/if}

      {:else if state.status === 'latest'}
        <p class="tagline">
          imgplex <span class="version">{state.version}</span> is the latest version.
        </p>
        {#if state.body}
          <div class="release-notes">
            <p class="notes-label">Release notes</p>
            <pre class="notes-body">{state.body}</pre>
          </div>
        {/if}

      {:else}
        <p class="tagline error-text">
          Could not reach GitHub. Check your internet connection and try again.
        </p>
      {/if}

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
    width: 420px;
    max-height: 80vh;
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
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 16px;
    overflow-y: auto;
  }

  .checking-row {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .checking-text {
    font-family: var(--font-ui);
    font-size: var(--font-size-base);
    color: var(--text);
  }

  .spinner {
    width: 14px;
    height: 14px;
    border: 2px solid var(--ctx-border);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
    flex-shrink: 0;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .tagline {
    font-family: var(--font-ui);
    font-size: var(--font-size-base);
    color: var(--text);
    line-height: 1.5;
    margin: 0;
  }

  .error-text {
    color: var(--text-muted);
  }

  .version {
    color: var(--accent);
    font-family: var(--font-mono);
    font-weight: 600;
  }

  .actions {
    display: flex;
    gap: 8px;
  }

  .btn-update {
    background: var(--accent);
    border: none;
    color: #fff;
    font-family: var(--font-ui);
    font-size: var(--font-size-base);
    font-weight: 600;
    padding: 6px 18px;
    border-radius: 4px;
    cursor: pointer;
    transition: opacity 0.12s;
  }

  .btn-update:hover {
    opacity: 0.85;
  }

  .btn-later {
    background: none;
    border: 1px solid var(--ctx-border);
    color: var(--text);
    font-family: var(--font-ui);
    font-size: var(--font-size-base);
    padding: 6px 18px;
    border-radius: 4px;
    cursor: pointer;
    transition: background 0.12s, color 0.12s;
  }

  .btn-later:hover {
    background: var(--ctx-item-hover-bg);
    color: var(--text-bright);
  }

  .release-notes {
    display: flex;
    flex-direction: column;
    gap: 8px;
    border-top: 1px solid var(--ctx-separator);
    padding-top: 14px;
  }

  .notes-label {
    font-family: var(--font-ui);
    font-size: 11px;
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    margin: 0;
  }

  .notes-body {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text);
    line-height: 1.6;
    white-space: pre-wrap;
    word-break: break-word;
    margin: 0;
    max-height: 260px;
    overflow-y: auto;
  }
</style>
