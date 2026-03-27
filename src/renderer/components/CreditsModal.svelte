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

  const deps = [
    { name: 'Electron',              license: 'MIT',              url: 'https://www.electronjs.org' },
    { name: 'Svelte',                license: 'MIT',              url: 'https://svelte.dev' },
    { name: 'Svelte Flow (@xyflow)', license: 'MIT',              url: 'https://svelteflow.dev' },
    { name: 'Vite',                  license: 'MIT',              url: 'https://vitejs.dev' },
    { name: 'TypeScript',            license: 'Apache 2.0',       url: 'https://www.typescriptlang.org' },
    { name: 'ImageMagick',           license: 'ImageMagick',      url: 'https://imagemagick.org' },
    { name: 'electron-builder',      license: 'MIT',              url: 'https://www.electron.build' },
    { name: '@yao-pkg/pkg',          license: 'MIT',              url: 'https://github.com/yao-pkg/pkg' },
    { name: 'vite-plugin-electron',  license: 'MIT',              url: 'https://github.com/electron-vite/vite-plugin-electron' },
  ]

  const fonts = [
    { name: 'JetBrains Mono',        license: 'SIL OFL 1.1',     url: 'https://www.jetbrains.com/lp/mono/' },
    { name: 'Atkinson Hyperlegible', license: 'SIL OFL 1.1',     url: 'https://www.brailleinstitute.org/freefont/' },
  ]
</script>

<svelte:window onkeydown={onKeydown} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="backdrop" onclick={onBackdropClick}>
  <div class="modal" role="dialog" aria-modal="true" aria-label="Credits">
    <div class="modal-header">
      <span class="modal-title">Credits</span>
      <button class="close-btn" onclick={onClose} aria-label="Close">✕</button>
    </div>

    <div class="modal-body">
      <section>
        <h2 class="section-heading">Open Source Libraries</h2>
        <table>
          <tbody>
            {#each deps as dep}
              <tr>
                <td class="dep-name">
                  <button class="link-btn" onclick={() => openUrl(dep.url)}>{dep.name}</button>
                </td>
                <td class="dep-license">{dep.license}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </section>

      <section>
        <h2 class="section-heading">Fonts</h2>
        <table>
          <tbody>
            {#each fonts as font}
              <tr>
                <td class="dep-name">
                  <button class="link-btn" onclick={() => openUrl(font.url)}>{font.name}</button>
                </td>
                <td class="dep-license">{font.license}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </section>
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
    overflow-y: auto;
    padding: 14px 16px 16px;
    display: flex;
    flex-direction: column;
    gap: 18px;
    scrollbar-width: thin;
    scrollbar-color: transparent transparent;
    transition: scrollbar-color 0.2s;
  }

  .modal-body:hover {
    scrollbar-color: var(--scrollbar-thumb) transparent;
  }

  .modal-body::-webkit-scrollbar { width: var(--scrollbar-width); }
  .modal-body::-webkit-scrollbar-track { background: transparent; }
  .modal-body::-webkit-scrollbar-thumb { background: transparent; border-radius: 3px; }
  .modal-body:hover::-webkit-scrollbar-thumb { background: var(--scrollbar-thumb); }
  .modal-body::-webkit-scrollbar-thumb:hover { background: var(--scrollbar-thumb-hover); }
  .modal-body::-webkit-scrollbar-button { display: none; }

  section { display: flex; flex-direction: column; gap: 8px; }

  .section-heading {
    font-family: var(--font-ui);
    font-size: 11px;
    font-weight: 700;
    color: #6fb8cc;   /* muted teal — 7.6:1 on --ctx-bg (#1c1c1c) */
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }

  table {
    width: 100%;
    border-collapse: collapse;
  }

  tr + tr td {
    border-top: 1px solid var(--ctx-border);
  }

  td {
    padding: 7px 4px;
    font-family: var(--font-ui);
    font-size: var(--font-size-base);
    vertical-align: middle;
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

  .dep-license {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--ctx-text-muted);
    text-align: right;
    white-space: nowrap;
  }
</style>
