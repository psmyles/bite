<script lang="ts">
  interface Props {
    onNew:       () => void
    onOpen:      () => void
    onSave:      () => void
    onSaveAs:    () => void
    onDuplicate: () => void
    onDelete:    () => void
    onAbout:     () => void
    onCredits:   () => void
    title:       string
  }

  let { onNew, onOpen, onSave, onSaveAs, onDuplicate, onDelete, onAbout, onCredits, title }: Props = $props()

  let openMenu: string | null = $state(null)

  function toggle(name: string) {
    openMenu = openMenu === name ? null : name
  }

  function run(fn: () => void) {
    openMenu = null
    fn()
  }

  function onWindowClick(e: MouseEvent) {
    if (!(e.target as Element).closest('.menu-item')) openMenu = null
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') { openMenu = null; return }
    if (!e.ctrlKey && !e.metaKey) return
    if (e.key === 'n') { e.preventDefault(); onNew() }
    if (e.key === 'o') { e.preventDefault(); onOpen() }
    if (e.key === 's' && !e.shiftKey) { e.preventDefault(); onSave() }
    if (e.key === 'S' &&  e.shiftKey) { e.preventDefault(); onSaveAs() }
  }
</script>

<svelte:window onclick={onWindowClick} onkeydown={onKeydown} />

<div class="menubar">
  <span class="app-name">imgplex</span>

  <!-- File menu -->
  <div class="menu-item" class:open={openMenu === 'file'}>
    <button class="menu-trigger" onclick={() => toggle('file')}>File</button>
    {#if openMenu === 'file'}
      <ul class="dropdown">
        <li><button onclick={() => run(onNew)}>   New<span class="shortcut">Ctrl+N</span></button></li>
        <li><button onclick={() => run(onOpen)}>  Open…<span class="shortcut">Ctrl+O</span></button></li>
        <li class="sep"></li>
        <li><button onclick={() => run(onSave)}>  Save<span class="shortcut">Ctrl+S</span></button></li>
        <li><button onclick={() => run(onSaveAs)}>Save As…<span class="shortcut">Ctrl+Shift+S</span></button></li>
      </ul>
    {/if}
  </div>

  <!-- Edit menu -->
  <div class="menu-item" class:open={openMenu === 'edit'}>
    <button class="menu-trigger" onclick={() => toggle('edit')}>Edit</button>
    {#if openMenu === 'edit'}
      <ul class="dropdown">
        <li><button onclick={() => { openMenu = null; document.execCommand('undo') }}>Undo<span class="shortcut">Ctrl+Z</span></button></li>
        <li><button onclick={() => { openMenu = null; document.execCommand('redo') }}>Redo<span class="shortcut">Ctrl+Y</span></button></li>
        <li class="sep"></li>
        <li><button onclick={() => { openMenu = null; document.execCommand('cut') }}>Cut<span class="shortcut">Ctrl+X</span></button></li>
        <li><button onclick={() => { openMenu = null; document.execCommand('copy') }}>Copy<span class="shortcut">Ctrl+C</span></button></li>
        <li><button onclick={() => { openMenu = null; document.execCommand('paste') }}>Paste<span class="shortcut">Ctrl+V</span></button></li>
        <li class="sep"></li>
        <li><button onclick={() => run(onDuplicate)}>Duplicate<span class="shortcut">Ctrl+D</span></button></li>
        <li><button onclick={() => run(onDelete)}>Delete<span class="shortcut">Del</span></button></li>
        <li class="sep"></li>
        <li><button onclick={() => { openMenu = null; document.execCommand('selectAll') }}>Select All<span class="shortcut">Ctrl+A</span></button></li>
      </ul>
    {/if}
  </div>

  <!-- View menu -->
  <div class="menu-item" class:open={openMenu === 'view'}>
    <button class="menu-trigger" onclick={() => toggle('view')}>View</button>
    {#if openMenu === 'view'}
      <ul class="dropdown">
        <li><button onclick={() => { openMenu = null }}>Actual Size<span class="shortcut">Ctrl+0</span></button></li>
        <li><button onclick={() => { openMenu = null }}>Zoom In<span class="shortcut">Ctrl++</span></button></li>
        <li><button onclick={() => { openMenu = null }}>Zoom Out<span class="shortcut">Ctrl+−</span></button></li>
        <li class="sep"></li>
        <li><button onclick={() => { openMenu = null; document.documentElement.requestFullscreen?.() }}>Toggle Full Screen<span class="shortcut">F11</span></button></li>
      </ul>
    {/if}
  </div>

  <!-- Help menu -->
  <div class="menu-item" class:open={openMenu === 'help'}>
    <button class="menu-trigger" onclick={() => toggle('help')}>Help</button>
    {#if openMenu === 'help'}
      <ul class="dropdown">
        <li><button onclick={() => run(onAbout)}>About</button></li>
        <li><button onclick={() => run(onCredits)}>Credits</button></li>
      </ul>
    {/if}
  </div>

  <span class="title">{title}</span>
</div>

<style>
  .menubar {
    display: flex;
    align-items: center;
    height: 30px;
    background: var(--panel-header-bg);
    border-bottom: 1px solid var(--border);
    padding: 0 8px;
    gap: 2px;
    flex-shrink: 0;
    user-select: none;
  }

  .app-name {
    font-family: var(--font-ui);
    font-size: var(--font-size-base);
    font-weight: 700;
    color: var(--text-bright);
    padding: 0 10px 0 4px;
    letter-spacing: 0.05em;
  }

  .title {
    margin-left: auto;
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text);
    opacity: 0.6;
    padding-right: 4px;
  }

  /* ── Menu items ── */
  .menu-item {
    position: relative;
  }

  .menu-trigger {
    background: none;
    border: none;
    color: var(--text);
    font-family: var(--font-ui);
    font-size: var(--font-size-base);
    padding: 0 8px;
    height: 24px;
    border-radius: 4px;
    cursor: pointer;
    white-space: nowrap;
  }

  .menu-trigger:hover,
  .menu-item.open .menu-trigger {
    background: var(--library-item-hover-bg);
    color: var(--text-bright);
  }

  /* ── Dropdown ── */
  .dropdown {
    position: absolute;
    top: calc(100% + 2px);
    left: 0;
    background: var(--ctx-bg);
    border: 1px solid var(--ctx-border);
    border-radius: var(--ctx-radius);
    box-shadow: var(--ctx-shadow);
    list-style: none;
    min-width: 180px;
    padding: 4px 0;
    z-index: 1000;
  }

  .dropdown li button {
    display: flex;
    align-items: center;
    width: 100%;
    background: none;
    border: none;
    color: var(--ctx-text);
    font-family: var(--font-ui);
    font-size: var(--font-size-base);
    padding: 5px 12px;
    cursor: pointer;
    text-align: left;
    gap: 8px;
  }

  .dropdown li button:hover {
    background: var(--ctx-item-hover-bg);
    color: var(--text-bright);
  }

  .shortcut {
    margin-left: auto;
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--ctx-text-muted);
    white-space: nowrap;
  }

  .sep {
    height: 1px;
    background: var(--ctx-separator);
    margin: 4px 0;
  }
</style>
