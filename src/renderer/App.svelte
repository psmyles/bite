<script lang="ts">
  import NodeLibrary from './components/NodeLibrary.svelte'
  import NodeEditor from './nodeEditor/NodeEditor.svelte'
  import Inspector from './components/Inspector.svelte'
  import Preview from './components/Preview.svelte'
  import Filmstrip from './components/Filmstrip.svelte'
  import type { NodeDefinition, NodeGraph, GraphNode, GraphEdge } from '../shared/types.js'
  import { IPC } from '../shared/constants.js'
  import { graphStore } from './stores/graph.svelte.js'
  import { expandNodeData, sortNodesGroupFirst } from './nodeEditor/nodeEditorHelpers.js'
  import { IS_ELECTRON } from './platform.js'
  import MenuBar from './components/MenuBar.svelte'
  import CreditsModal from './components/CreditsModal.svelte'
  import AboutModal from './components/AboutModal.svelte'
  import BatchSummaryModal from './components/BatchSummaryModal.svelte'
  import ConfirmModal from './components/ConfirmModal.svelte'
  import ImportProgressModal from './components/ImportProgressModal.svelte'
  import { imageStore } from './stores/images.svelte.js'

  // ── Node definitions — loaded once here, passed to NodeLibrary + NodeEditor ──
  let definitions: NodeDefinition[] = $state([])
  $effect(() => {
    window.ipcRenderer.invoke(IPC.REGISTRY_GET_ALL)
      .then((defs: NodeDefinition[]) => { definitions = defs })
      .catch((err: unknown) => { console.error('[App] Failed to load node registry:', err) })
    const onUpdated = (_e: unknown, defs: NodeDefinition[]) => { definitions = defs }
    window.ipcRenderer.on(IPC.REGISTRY_UPDATED, onUpdated)
    return () => window.ipcRenderer.off(IPC.REGISTRY_UPDATED, onUpdated)
  })

  // ── Batch progress listener — registered here so it persists across Inspector remounts ──
  $effect(() => {
    type P = { completed: number; total: number; currentFile: string }
    const onProgress = (_e: unknown, p: P) => { graphStore.batchProgress = p }
    window.ipcRenderer.on(`${IPC.EXECUTE_BATCH}:progress`, onProgress)
    return () => window.ipcRenderer.off(`${IPC.EXECUTE_BATCH}:progress`, onProgress)
  })

  // ── Workflow helpers ───────────────────────────────────────────────────────

  function buildNodeGraph(): NodeGraph {
    // Sort: group nodes before their children so load order is correct
    const sortedNodes = sortNodesGroupFirst(graphStore.nodes)
    return {
      nodes: sortedNodes.map((n) => {
        const d = n.data as Record<string, unknown>
        // Slim node data: only persist what can't be re-derived from the definition.
        // workflow-input has no user state; workflow-output and regular nodes save params only.
        const slimData: GraphNode['data'] =
          n.id === 'workflow-input'
            ? { label: 'Input', definitionId: '', params: {} }
            : n.id === 'workflow-output'
              ? { label: 'Output', definitionId: '', params: (d.params ?? {}) as Record<string, unknown> }
              : { label: String(d.label ?? ''), definitionId: String(d.definitionId ?? ''), params: (d.params ?? {}) as Record<string, unknown> }
        return {
          id: n.id,
          type: n.type ?? 'process',
          position: n.position,
          ...(n.width  != null ? { width:  n.width  } : {}),
          ...(n.height != null ? { height: n.height } : {}),
          ...(n.parentId         ? { parentId: n.parentId }      : {}),
          ...(n.extent           ? { extent: 'parent' as const } : {}),
          data: slimData,
        }
      }),
      edges: graphStore.edges.map((e) => ({
        id: e.id,
        source: e.source,
        sourceHandle: e.sourceHandle ?? undefined,
        target: e.target,
        targetHandle: e.targetHandle ?? undefined,
      })) as GraphEdge[],
      viewport: graphStore.viewport,
    }
  }

  let confirmState = $state<{ message: string; resolve: (ok: boolean) => void } | null>(null)

  function confirmLoseChanges(action: string): Promise<boolean> {
    if (!graphStore.isDirty) return Promise.resolve(true)
    return new Promise((resolve) => {
      confirmState = { message: `You have unsaved changes. ${action} anyway?`, resolve }
    })
  }

  function resolveConfirm(ok: boolean) {
    confirmState?.resolve(ok)
    confirmState = null
  }

  async function handleNew() {
    if (!await confirmLoseChanges('Start a new workflow')) return
    graphStore.resetToSeed()
  }

  function applyWorkflow(graph: NodeGraph, filePath: string) {
    const sortedSaved = sortNodesGroupFirst(graph.nodes)
    graphStore.nodes = sortedSaved.map((n) => {
      const saved = n.data as Record<string, unknown>
      const savedParams = (saved.params ?? {}) as Record<string, unknown>
      let data: Record<string, unknown>
      if (n.id === 'workflow-input') {
        data = { label: 'Input', inputs: [], outputs: ['image'] }
      } else if (n.id === 'workflow-output') {
        data = { label: 'Output', inputs: ['image'], outputs: [],
          params: {
            outputPath: 'source', customPath: '', overwrite: 'skip',
            outputMode: 'image',
            portIds: ['txo-0'], nextPortIndex: 1,
            separatorType: 'comma', customSeparator: '',
            ...savedParams,
          } }
      } else if (n.type === 'group') {
        data = { label: String(saved.label ?? 'Group'), definitionId: '', params: savedParams }
      } else {
        const defId = String(saved.definitionId ?? '')
        const def = definitions.find((d) => d.id === defId)
        data = def ? expandNodeData(def, savedParams) : (saved as Record<string, unknown>)
      }
      return {
        id: n.id,
        type: n.type ?? 'process',
        position: n.position,
        ...(n.width  != null ? { width:  n.width  } : {}),
        ...(n.height != null ? { height: n.height } : {}),
        ...(n.parentId         ? { parentId: n.parentId }      : {}),
        ...(n.extent           ? { extent: 'parent' as const } : {}),
        ...(n.type === 'group' ? { zIndex: 0 }                 : {}),
        data,
        ...(n.id === 'workflow-input' || n.id === 'workflow-output' ? { deletable: false } : {}),
      }
    })
    graphStore.edges = graph.edges
    graphStore.selectedNodeId = null
    graphStore.previewNodeId = null
    graphStore.propValues = {}
    graphStore.pendingViewport = graph.viewport
    graphStore.markClean(filePath)
  }

  async function handleOpenWorkflow() {
    if (!await confirmLoseChanges('Open a different workflow')) return
    let result: { graph: NodeGraph; filePath: string } | null
    try {
      result = await window.ipcRenderer.invoke(IPC.WORKFLOW_LOAD) as typeof result
    } catch (err) {
      alert(`Failed to open workflow:\n${err instanceof Error ? err.message : String(err)}`)
      return
    }
    if (!result) return
    applyWorkflow(result.graph, result.filePath)
  }

  async function handleOpenFilePath(filePath: string) {
    if (!await confirmLoseChanges('Open a different workflow')) return
    let result: { graph: NodeGraph; filePath: string } | null
    try {
      result = await window.ipcRenderer.invoke(IPC.WORKFLOW_OPEN_PATH, filePath) as typeof result
    } catch (err) {
      alert(`Failed to open workflow:\n${err instanceof Error ? err.message : String(err)}`)
      return
    }
    if (!result) return
    applyWorkflow(result.graph, result.filePath)
  }

  async function handleSaveWorkflow() {
    // JSON round-trip strips Svelte 5 reactive proxies — IPC structured clone
    // cannot serialize Proxy objects and throws a silent unhandled rejection.
    const graph = JSON.parse(JSON.stringify(buildNodeGraph())) as NodeGraph
    const savedPath = await window.ipcRenderer.invoke(
      IPC.WORKFLOW_SAVE, graph, graphStore.currentFilePath,
    ) as string | null
    if (savedPath) graphStore.markClean(savedPath)
  }

  async function handleSaveWorkflowAs() {
    const graph = JSON.parse(JSON.stringify(buildNodeGraph())) as NodeGraph
    // Pass null to always force the Save dialog
    const savedPath = await window.ipcRenderer.invoke(
      IPC.WORKFLOW_SAVE, graph, null,
    ) as string | null
    if (savedPath) graphStore.markClean(savedPath)
  }

  async function handleExit() {
    if (!await confirmLoseChanges('Exit')) return
    await window.ipcRenderer.invoke(IPC.APP_QUIT)
  }

  let showAbout = $state(false)
  function handleAbout() { showAbout = true }

  let showCredits = $state(false)
  function handleCredits() { showCredits = true }

  async function handleExportCLI(shellType: 'powershell' | 'bash' | 'cmd') {
    if (!IS_ELECTRON) {
      alert('CLI export requires the desktop app.')
      return
    }
    const graph = JSON.parse(JSON.stringify(buildNodeGraph())) as NodeGraph
    try {
      await window.ipcRenderer.invoke(IPC.EXPORT_CLI, graph, shellType)
    } catch (err) {
      alert(`Failed to export CLI script:\n${err instanceof Error ? err.message : String(err)}`)
    }
  }

  // ── Menu IPC listeners ─────────────────────────────────────────────────────
  $effect(() => {
    const onNew           = () => handleNew()
    const onOpen          = () => handleOpenWorkflow()
    const onSave          = () => handleSaveWorkflow()
    const onSaveAs        = () => handleSaveWorkflowAs()
    const onExit          = () => handleExit()
    const onAbout         = () => handleAbout()
    const onExportCliPS   = () => handleExportCLI('powershell')
    const onExportCliBash = () => handleExportCLI('bash')
    const onExportCliCmd  = () => handleExportCLI('cmd')
    const onOpenFilePath  = (_e: unknown, fp: string) => handleOpenFilePath(fp)
    window.ipcRenderer.on(IPC.MENU_NEW,              onNew)
    window.ipcRenderer.on(IPC.MENU_OPEN_WORKFLOW,    onOpen)
    window.ipcRenderer.on(IPC.MENU_SAVE_WORKFLOW,    onSave)
    window.ipcRenderer.on(IPC.MENU_SAVE_WORKFLOW_AS, onSaveAs)
    window.ipcRenderer.on(IPC.MENU_EXIT,             onExit)
    window.ipcRenderer.on(IPC.MENU_ABOUT,            onAbout)
    window.ipcRenderer.on(IPC.MENU_CREDITS,          handleCredits)
    window.ipcRenderer.on(IPC.MENU_EXPORT_CLI_PS,   onExportCliPS)
    window.ipcRenderer.on(IPC.MENU_EXPORT_CLI_BASH, onExportCliBash)
    window.ipcRenderer.on(IPC.MENU_EXPORT_CLI_CMD,  onExportCliCmd)
    window.ipcRenderer.on(IPC.OPEN_FILE_PATH,        onOpenFilePath)
    return () => {
      window.ipcRenderer.off(IPC.MENU_NEW,              onNew)
      window.ipcRenderer.off(IPC.MENU_OPEN_WORKFLOW,    onOpen)
      window.ipcRenderer.off(IPC.MENU_SAVE_WORKFLOW,    onSave)
      window.ipcRenderer.off(IPC.MENU_SAVE_WORKFLOW_AS, onSaveAs)
      window.ipcRenderer.off(IPC.MENU_EXIT,             onExit)
      window.ipcRenderer.off(IPC.MENU_ABOUT,            onAbout)
      window.ipcRenderer.off(IPC.MENU_CREDITS,          handleCredits)
      window.ipcRenderer.off(IPC.MENU_EXPORT_CLI_PS,   onExportCliPS)
      window.ipcRenderer.off(IPC.MENU_EXPORT_CLI_BASH, onExportCliBash)
      window.ipcRenderer.off(IPC.MENU_EXPORT_CLI_CMD,  onExportCliCmd)
      window.ipcRenderer.off(IPC.OPEN_FILE_PATH,        onOpenFilePath)
    }
  })

  // ── Window title: "[*][filename —] imgplex" ────────────────────────────────
  $effect(() => {
    const filePath = graphStore.currentFilePath
    const dirty    = graphStore.isDirty
    const fileName = filePath ? filePath.split(/[\\/]/).pop()!.replace(/\.[^.]+$/, '') : 'Untitled'
    const prefix   = dirty ? '*' : ''
    document.title = `${prefix}${fileName} - imgplex`
  })

  // Read unitless panel size tokens from theme.css
  function cssNum(prop: string): number {
    return parseFloat(getComputedStyle(document.documentElement).getPropertyValue(prop))
  }

  // Panel dimensions (px) — defaults and constraints come from theme.css
  let leftWidth       = $state(cssNum('--left-panel-default'))
  let rightWidth      = $state(cssNum('--right-panel-default'))
  let filmstripHeight = $state(cssNum('--filmstrip-default'))

  // Inspector/preview split — stored as a fraction (0–1) of the right column's usable height
  // so it scales correctly when the window is resized.
  const shellPadding = cssNum('--shell-padding')
  const handleSize   = cssNum('--panel-gap')
  // Use the shell element's actual height (excludes menu bar in browser mode)
  let shellHeight = $state(window.innerHeight)
  const rightColAvail  = $derived(shellHeight - 2 * shellPadding - handleSize)
  let inspectorPct     = $state(cssNum('--inspector-default'))
  const inspectorHeight = $derived(Math.round(inspectorPct * rightColAvail))

  type Handle = 'left' | 'right' | 'bottom' | 'inspector'

  function startDrag(handle: Handle, e: MouseEvent) {
    e.preventDefault()
    const startX  = e.clientX
    const startY  = e.clientY
    const w0Left  = leftWidth
    const w0Right = rightWidth
    const h0Film  = filmstripHeight
    const p0Insp  = inspectorPct

    function onMove(ev: MouseEvent) {
      const dx = ev.clientX - startX
      const dy = ev.clientY - startY
      if (handle === 'left')      leftWidth       = Math.max(cssNum('--left-panel-min'),  Math.min(cssNum('--left-panel-max'),  w0Left  + dx))
      if (handle === 'right')     rightWidth      = Math.max(cssNum('--right-panel-min'), Math.min(cssNum('--right-panel-max'), w0Right - dx))
      if (handle === 'bottom')    filmstripHeight = Math.max(cssNum('--filmstrip-min'),    Math.min(cssNum('--filmstrip-max'),   h0Film  - dy))
      if (handle === 'inspector') inspectorPct    = Math.max(cssNum('--inspector-min'),    Math.min(cssNum('--inspector-max'),   p0Insp  + dy / rightColAvail))
    }

    function onUp() {
      window.removeEventListener('mousemove', onMove)
      window.removeEventListener('mouseup', onUp)
    }

    window.addEventListener('mousemove', onMove)
    window.addEventListener('mouseup', onUp)
  }
</script>

<div class="app-root">

{#if !IS_ELECTRON}
  <MenuBar
    onNew={handleNew}
    onOpen={handleOpenWorkflow}
    onSave={handleSaveWorkflow}
    onSaveAs={handleSaveWorkflowAs}
    onAbout={handleAbout}
    onCredits={handleCredits}
    title={document.title}
  />
{/if}

<div class="shell" bind:clientHeight={shellHeight}>

  <!-- ── Main area: (library | canvas) over filmstrip ── -->
  <div class="main-area">

    <div class="top-row">
      <!-- Left: Node Library -->
      <div class="left-col" style="width: {leftWidth}px">
        <NodeLibrary {definitions} />
      </div>

      <!-- Drag handle: left -->
      <div
        class="handle h-col"
        role="separator"
        aria-orientation="vertical"
        tabindex="0"
        onmousedown={(e) => startDrag('left', e)}
      ></div>

      <!-- Center: Node Editor -->
      <div class="center-col">
        <NodeEditor {definitions} />
      </div>
    </div>

    <!-- Drag handle: filmstrip -->
    <div
      class="handle h-row"
      role="separator"
      aria-orientation="horizontal"
      tabindex="0"
      onmousedown={(e) => startDrag('bottom', e)}
    ></div>

    <!-- Bottom: Filmstrip -->
    <div class="filmstrip-row" style="height: {filmstripHeight}px">
      <Filmstrip />
    </div>

  </div>

  <!-- Drag handle: right column -->
  <div
    class="handle h-col"
    role="separator"
    aria-orientation="vertical"
    tabindex="0"
    onmousedown={(e) => startDrag('right', e)}
  ></div>

  <!-- Right column: full height — Inspector (top) + Preview (bottom) -->
  <div class="right-col" style="width: {rightWidth}px">
    <div class="inspector-pane" style="height: {inspectorHeight}px">
      <Inspector {definitions} />
    </div>

    <!-- Drag handle: inspector/preview split -->
    <div
      class="handle h-row"
      role="separator"
      aria-orientation="horizontal"
      tabindex="0"
      onmousedown={(e) => startDrag('inspector', e)}
    ></div>

    <div class="preview-pane">
      <Preview />
    </div>
  </div>

</div>

</div>

{#if showAbout}
  <AboutModal onClose={() => showAbout = false} />
{/if}

{#if showCredits}
  <CreditsModal onClose={() => showCredits = false} />
{/if}

{#if graphStore.batchSummaryOpen && graphStore.batchSummary}
  <BatchSummaryModal onClose={() => { graphStore.batchSummaryOpen = false }} />
{/if}

{#if confirmState}
  <ConfirmModal
    message={confirmState.message}
    onConfirm={() => resolveConfirm(true)}
    onCancel={() => resolveConfirm(false)}
  />
{/if}

{#if imageStore.importProgress || imageStore.importDone}
  <ImportProgressModal />
{/if}

<style>
  .app-root {
    display: flex;
    flex-direction: column;
    width: 100vw;
    height: 100vh;
    overflow: hidden;
  }

  .shell {
    display: flex;
    flex-direction: row;        /* shell is now a row: main-area | handle | right-col */
    flex: 1;
    min-height: 0;
    width: 100%;
    background: var(--gap-color);
    padding: var(--shell-padding);
    overflow: hidden;
    user-select: none;
    box-sizing: border-box;
  }

  /* ── Main area: column of (top-row + filmstrip) ── */
  .main-area {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .top-row {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: row;
  }

  /* ── Panel cards — rounded, clipped, no borders ── */
  .left-col {
    flex-shrink: 0;
    min-width: 0;
    overflow: hidden;
    border-radius: var(--panel-radius);
    background: var(--panel-bg);
  }

  .center-col {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    border-radius: var(--panel-radius);
    background: var(--graph-bg-base-color);
    transform: translateZ(0); /* force GPU compositing layer — fixes subpixel border-radius artifact */
  }

  /* ── Filmstrip row ── */
  .filmstrip-row {
    flex-shrink: 0;
    overflow: hidden;
    border-radius: var(--panel-radius);
    background: var(--panel-bg);
  }

  /* ── Right column — full window height ── */
  .right-col {
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
  }

  .inspector-pane {
    flex-shrink: 0;
    overflow: hidden;
    border-radius: var(--panel-radius);
    background: var(--panel-bg);
  }

  .preview-pane {
    flex: 1;
    min-height: 0;
    overflow: hidden;
    border-radius: var(--panel-radius);
    background: var(--preview-bg);
  }

  /* ── Drag handles — transparent gap zones ── */
  .handle {
    flex-shrink: 0;
    background: transparent;
    z-index: 10;
  }

  .h-col {
    width: var(--handle-size);
    cursor: col-resize;
  }

  .h-row {
    height: var(--handle-size);
    cursor: row-resize;
    width: 100%;
  }
</style>
