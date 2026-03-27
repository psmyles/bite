<script lang="ts">
  import { imageStore } from '../stores/images.svelte.js'
  import { IPC } from '../../shared/constants.js'

  // ── Format groups ─────────────────────────────────────────────────────────
  const FORMAT_GROUPS = [
    { label: 'PNG',  exts: ['png'] },
    { label: 'JPEG', exts: ['jpg', 'jpeg'] },
    { label: 'WEBP', exts: ['webp'] },
    { label: 'AVIF', exts: ['avif'] },
    { label: 'TIFF', exts: ['tif', 'tiff'] },
    { label: 'BMP',  exts: ['bmp'] },
    { label: 'TGA',  exts: ['tga'] },
    { label: 'PSD',  exts: ['psd', 'psb'] },
    { label: 'EXR',  exts: ['exr', 'hdr'] },
    { label: 'RAW',  exts: ['cr2','cr3','nef','nrw','arw','dng','orf','raf','rw2','pef','srw'] },
  ] as const

  // ── Folder import state ───────────────────────────────────────────────────
  let recursive      = $state(false)
  let activeFormats  = $state(new Set(['PNG', 'JPEG', 'WEBP', 'TIFF']))
  let folderPath     = $state<string | null>(null)
  let matchingPaths  = $state<string[]>([])
  let selecting      = $state(false)   // folder dialog open
  let counting       = $state(false)   // re-scanning after filter change

  const importing = $derived(imageStore.importProgress !== null)

  function activeExtensions() {
    return FORMAT_GROUPS
      .filter(g => activeFormats.has(g.label))
      .flatMap(g => [...g.exts])
  }

  function toggleFormat(label: string) {
    const next = new Set(activeFormats)
    if (next.has(label)) { if (next.size > 1) next.delete(label) }
    else next.add(label)
    activeFormats = next
  }

  async function chooseFolder() {
    selecting = true
    try {
      const picked: string | null = await window.ipcRenderer.invoke(IPC.OPEN_FOLDER_DIALOG)
      if (picked) folderPath = picked
    } finally {
      selecting = false
    }
  }

  // Re-scan whenever folder, recursion flag, or active formats change
  $effect(() => {
    const fp = folderPath
    const rec = recursive
    const exts = activeExtensions()   // depends on activeFormats
    if (!fp) { matchingPaths = []; return }
    counting = true
    window.ipcRenderer.invoke(IPC.SCAN_FOLDER, { folderPath: fp, recursive: rec, extensions: exts })
      .then((paths: string[]) => { matchingPaths = paths })
      .finally(() => { counting = false })
  })

  async function importFromFolder() {
    if (matchingPaths.length === 0) return
    await imageStore.add(matchingPaths)
  }

  function folderDisplayName(p: string) {
    return p.replace(/\\/g, '/').split('/').pop() ?? p
  }
</script>

<div class="input-inspector">
  {#if imageStore.images.length === 0}
    <!-- ── Folder picker (shown when filmstrip is empty) ─────────────────── -->
    <div class="folder-section">

      <!-- Folder import group -->
      <div class="import-group">
        <span class="group-label">Folder</span>
        <div class="group-body">

          <!-- Step 1: pick a folder -->
          <button class="io-btn" onclick={chooseFolder} disabled={selecting}>
            {selecting ? 'Choosing…' : folderPath ? 'Change Folder…' : 'Select Input Folder…'}
          </button>

          {#if folderPath}
            <!-- Selected folder path -->
            <div class="folder-path" title={folderPath}>
              <span class="folder-path-name">{folderDisplayName(folderPath)}</span>
              <span class="folder-path-full">{folderPath}</span>
            </div>

            <!-- Filters -->
            <label class="checkbox-label">
              <input type="checkbox" bind:checked={recursive} />
              <span>Include subfolders</span>
            </label>
            <div class="format-section">
              <span class="section-label">File formats</span>
              <div class="chip-grid">
                {#each FORMAT_GROUPS as g}
                  <button
                    class="chip"
                    class:active={activeFormats.has(g.label)}
                    onclick={() => toggleFormat(g.label)}
                  >{g.label}</button>
                {/each}
              </div>
            </div>

            <!-- Match count -->
            <div class="match-count" class:counting>
              {#if counting}
                <span>Scanning…</span>
              {:else}
                <span>{matchingPaths.length} {matchingPaths.length === 1 ? 'file' : 'files'} found</span>
              {/if}
            </div>

            <!-- Step 2: import -->
            <button
              class="io-btn io-btn--import"
              onclick={importFromFolder}
              disabled={importing || counting || matchingPaths.length === 0}
            >
              {importing ? 'Importing…' : `Import ${matchingPaths.length} ${matchingPaths.length === 1 ? 'Image' : 'Images'}`}
            </button>

          {/if}

        </div>
      </div>

      <!-- Individual images group -->
      <div class="import-group">
        <span class="group-label">Individual Images</span>
        <div class="group-body">
          <button class="io-btn" onclick={() => imageStore.openDialog()}>
            Add Individual Images…
          </button>
        </div>
      </div>

      <span class="empty-hint">or drop images onto the filmstrip</span>
    </div>

  {:else}
    <!-- ── Loaded image list ──────────────────────────────────────────────── -->
    <div class="input-top">
      <button class="io-btn" onclick={() => imageStore.openDialog()}>Add Images…</button>
      <button class="io-btn danger" onclick={() => imageStore.clear()}>Clear All</button>
    </div>
    <div class="file-list-wrap">
      {#each imageStore.images as img, i}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="file-entry"
          class:active={i === imageStore.selectedIndex}
          onclick={() => imageStore.select(i)}
          title={img.path}
        >
          <span class="file-entry-name">{img.name}</span>
          <span class="file-entry-fmt">{img.format?.toLowerCase()}</span>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .input-inspector {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
  }

  /* ── Shared button ──────────────────────────────────────────────────────── */
  .io-btn {
    width: 100%;
    background: #2e2e2e;
    border: 2px solid var(--border);
    border-radius: 4px;
    color: var(--text-bright);
    font-family: var(--font-ui);
    font-size: 13px;
    line-height: 1.4;
    padding: 5px 8px;
    cursor: pointer;
    text-align: left;
    outline: none;
    transition: border-color 0.1s, background 0.1s, color 0.1s;
  }

  .io-btn:hover:not(:disabled) { background: #383838; border-color: var(--accent); color: #ffffff; }
  .io-btn:disabled { opacity: 0.45; cursor: default; }

  .io-btn.danger:hover:not(:disabled) {
    border-color: #c0392b;
    color: #ff7f7f;
  }

  /* ── Folder section ─────────────────────────────────────────────────────── */
  .folder-section {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 10px 12px;
  }

  .import-group {
    display: flex;
    flex-direction: column;
    gap: 0;
    border: 1px solid var(--border);
    border-radius: 4px;
    overflow: hidden;
  }

  .group-label {
    font-family: var(--font-ui);
    font-size: 11px;
    font-weight: 600;
    color: var(--text);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    padding: 4px 8px;
    background: var(--panel-header-bg);
    border-bottom: 1px solid var(--border);
  }

  .group-body {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 8px;
  }

  .folder-path {
    display: flex;
    flex-direction: column;
    gap: 1px;
    padding: 5px 7px;
    background: var(--search-bg);
    border: 1px solid var(--border);
    border-radius: 3px;
    min-width: 0;
  }

  .folder-path-name {
    font-family: var(--font-ui);
    font-size: 12px;
    font-weight: 600;
    color: var(--text-bright);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .folder-path-full {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .match-count {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text);
    padding: 2px 0;
  }

  .match-count.counting {
    opacity: 0.5;
  }

  .io-btn--import {
    background: color-mix(in srgb, #4caf50 14%, var(--panel-header-bg));
    border-color: color-mix(in srgb, #4caf50 45%, transparent);
    color: #81c784;
  }

  .io-btn--import:hover:not(:disabled) {
    background: color-mix(in srgb, #4caf50 22%, var(--panel-header-bg));
    border-color: color-mix(in srgb, #4caf50 70%, transparent);
    color: #a5d6a7;
  }

  .io-btn--import:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .checkbox-label {
    display: flex;
    align-items: center;
    gap: 6px;
    cursor: pointer;
    font-family: var(--font-ui);
    font-size: 12px;
    color: var(--text-bright);
    user-select: none;
  }

  .checkbox-label input[type='checkbox'] {
    accent-color: var(--accent);
    width: 13px;
    height: 13px;
    margin: 0;
    cursor: pointer;
  }

  .format-section {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .section-label {
    font-family: var(--font-ui);
    font-size: 11px;
    color: var(--text-bright);
    opacity: 0.6;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .chip-grid {
    display: grid;
    grid-template-columns: repeat(5, 1fr);
    gap: 4px;
  }

  .chip {
    font-family: var(--font-mono);
    font-size: 11px;
    padding: 3px 0;
    border-radius: 3px;
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text-bright);
    cursor: pointer;
    text-align: center;
    outline: none;
    transition: background 0.1s, border-color 0.1s, color 0.1s;
  }

  .chip:hover { border-color: var(--accent); color: var(--text); }

  .chip.active {
    background: color-mix(in srgb, #4caf50 18%, transparent);
    border-color: color-mix(in srgb, #4caf50 60%, transparent);
    color: #81c784;
  }

  .empty-hint {
    font-family: var(--text-hint-family);
    font-size: var(--text-hint-size);
    color: var(--text);
    text-align: center;
    padding: 2px 0 4px;
  }

  /* ── Loaded list ────────────────────────────────────────────────────────── */
  .input-top {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px 12px 4px;
    flex-shrink: 0;
  }

  .file-list-wrap {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 4px 0 2px;
    scrollbar-width: thin;
    scrollbar-color: transparent transparent;
    transition: scrollbar-color 0.2s;
  }

  .file-list-wrap:hover { scrollbar-color: var(--scrollbar-thumb) transparent; }

  .file-entry {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
    height: 24px;
    padding: 0 12px;
    cursor: pointer;
    transition: background 0.1s;
  }

  .file-entry:hover  { background: color-mix(in srgb, var(--accent) 10%, transparent); }
  .file-entry.active { background: color-mix(in srgb, var(--accent) 18%, transparent); }

  .file-entry-name {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-bright);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    flex: 1;
    min-width: 0;
  }

  .file-entry.active .file-entry-name { color: var(--text); }

  .file-entry-fmt {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-bright);
    opacity: 0.4;
    flex-shrink: 0;
    text-transform: uppercase;
  }
</style>
