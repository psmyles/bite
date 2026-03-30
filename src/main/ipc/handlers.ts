import { ipcMain, dialog, app, shell } from 'electron'
import type { BrowserWindow } from 'electron'
import { readFileSync, writeFileSync, chmodSync, readdirSync } from 'node:fs'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { spawn } from 'node:child_process'
import { getMagickBinary } from '../pipeline/magick-path.js'
import type { GraphEdge, NodeGraph } from '../../shared/types.js'
import type { NodeRegistry } from '../nodes/registry.js'
import { PipelineExecutor, topoSort } from '../pipeline/executor.js'
import { computeNodeParams, loadImageMeta, loadImageMean, loadImageChannelMean, getSeparator, buildEmptyImageMeta } from '../pipeline/executor-compute.js'
import { IPC } from '../../shared/constants.js'

export function registerRegistryHandlers(
  registry: NodeRegistry,
  getWin: () => BrowserWindow | null
): void {
  ipcMain.handle(IPC.REGISTRY_GET_ALL, () => registry.getAll())

  registry.onChange((defs) => {
    getWin()?.webContents.send(IPC.REGISTRY_UPDATED, defs)
  })
}

export function registerPipelineHandlers(
  registry: NodeRegistry,
  executor: PipelineExecutor,
  getWin: () => BrowserWindow | null
): void {
  ipcMain.handle(IPC.LOAD_IMAGES, async (_e, paths: string[]) => {
    return Promise.all(paths.map((p) => executor.loadImage(p)))
  })

  ipcMain.handle(IPC.LOAD_IMAGES_WITH_THUMBNAILS, async (_e, paths: string[], size: number) => {
    return Promise.all(paths.map((p) => executor.loadImageWithThumbnail(p, size)))
  })

  // ── Streaming import: N concurrent workers, one push per result ──────────
  let _streamCancelled = false

  ipcMain.handle(IPC.LOAD_IMAGES_STREAMING_CANCEL, () => { _streamCancelled = true })

  ipcMain.handle(IPC.LOAD_IMAGES_STREAMING_START, async (_e, paths: string[], size: number) => {
    _streamCancelled = false
    const win = getWin()
    const concurrency = os.cpus().length
    // Batch multiple images per magick spawn to amortize process-spawn overhead.
    // Each worker picks a chunk of BATCH_SIZE images and runs them in one spawn.
    const BATCH_SIZE = 8
    let idx = 0

    async function worker(): Promise<void> {
      while (true) {
        if (_streamCancelled) break
        // Atomically collect the next batch (sync, no interleaving with other workers)
        const batch: string[] = []
        while (batch.length < BATCH_SIZE && idx < paths.length) batch.push(paths[idx++])
        if (batch.length === 0) break
        try {
          const results = await executor.loadImageWithThumbnailBatch(batch, size)
          for (const result of results) {
            if (!_streamCancelled) win?.webContents.send(IPC.LOAD_IMAGES_STREAMING_RESULT, result)
          }
        } catch (err) {
          console.error('[streaming] Batch failed starting at:', batch[0], err)
        }
      }
    }

    await Promise.all(Array.from({ length: concurrency }, worker))
  })

  ipcMain.handle(IPC.GENERATE_THUMBNAIL, async (_e, imagePath: string, size: number) => {
    return executor.generateThumbnail(imagePath, size)
  })

  ipcMain.handle(
    IPC.EXECUTE_PREVIEW,
    async (_e, graph: NodeGraph, imagePath: string, fromNodeId?: string) => {
      return executor.executePreview(graph, imagePath, registry, fromNodeId)
    }
  )

  ipcMain.handle(
    IPC.EXECUTE_BATCH,
    async (_e, graph: NodeGraph, imagePaths: string[], outputDir: string | null, overwrite: 'skip' | 'overwrite') => {
      return await executor.executeBatch(graph, imagePaths, outputDir, overwrite ?? 'skip', registry, (progress) => {
        getWin()?.webContents.send(`${IPC.EXECUTE_BATCH}:progress`, progress)
      })
    }
  )

  ipcMain.handle(IPC.EXPORT_CLI, async (_e, graph: NodeGraph, shellType: 'powershell' | 'bash' | 'cmd') => {
    const filterMap = {
      powershell: { name: 'PowerShell Script', extensions: ['ps1'] },
      bash:       { name: 'Shell Script',       extensions: ['sh']  },
      cmd:        { name: 'Batch File',         extensions: ['bat'] },
    }
    const defaultNames = { powershell: 'imgplex-batch.ps1', bash: 'imgplex-batch.sh', cmd: 'imgplex-batch.bat' }

    const result = await dialog.showSaveDialog(getWin()!, {
      title: 'Export CLI Script',
      defaultPath: defaultNames[shellType],
      filters: [filterMap[shellType], { name: 'All Files', extensions: ['*'] }],
    })
    if (result.canceled || !result.filePath) return null

    // Companion workflow file lives alongside the script with the same base name
    const scriptBase     = path.basename(result.filePath, path.extname(result.filePath))
    const workflowFile   = `${scriptBase}.imgplex`
    const workflowPath   = path.join(path.dirname(result.filePath), workflowFile)

    const scriptContent  = executor.exportCLI(shellType, workflowFile)
    writeFileSync(result.filePath, scriptContent, 'utf-8')
    writeFileSync(workflowPath, JSON.stringify({ version: '1.0', graph }, null, 2), 'utf-8')

    if (shellType === 'bash') {
      try { chmodSync(result.filePath, 0o755) } catch { /* non-fatal on Windows */ }
    }
    return result.filePath
  })
}

function scanFolder(root: string, recursive: boolean, extSet: Set<string>): string[] {
  const found: string[] = []
  function scan(dir: string): void {
    let entries
    try { entries = readdirSync(dir, { withFileTypes: true }) } catch { return }
    for (const entry of entries) {
      const full = path.join(dir, entry.name)
      if (entry.isDirectory()) {
        if (recursive) scan(full)
      } else {
        const ext = path.extname(entry.name).slice(1).toLowerCase()
        if (extSet.has(ext)) found.push(full)
      }
    }
  }
  scan(root)
  return found
}

const IMAGE_EXTENSIONS = [
  // Common web / display formats
  'jpg', 'jpeg', 'png', 'gif', 'webp', 'avif', 'svg', 'svgz', 'ico', 'bmp',
  // TIFF family
  'tif', 'tiff',
  // HEIF / Apple
  'heic', 'heif',
  // JPEG variants
  'jp2', 'j2k', 'jpf', 'jpx', 'jxl',
  // Professional / compositing
  'psd', 'psb', 'exr', 'hdr', 'dpx', 'cin',
  // Camera RAW
  'cr2', 'cr3', 'nef', 'nrw', 'arw', 'dng', 'orf', 'raf', 'rw2', 'pef', 'srw',
  'x3f', '3fr', 'kdc', 'mrw', 'erf', 'rwl',
  // Legacy / misc raster
  'tga', 'pcx', 'ppm', 'pgm', 'pbm', 'pnm', 'sgi', 'rgb', 'rgba',
  'miff', 'mng', 'jng', 'xbm', 'xpm', 'xwd', 'sun', 'iff', 'lbm',
  'wbmp', 'pict', 'pct', 'dds', 'fits', 'fts',
]

export function registerWorkflowHandlers(getWin: () => BrowserWindow | null): void {
  ipcMain.handle(IPC.WORKFLOW_SAVE, async (_e, graph: unknown, filePath: string | null) => {
    let targetPath = filePath ?? null
    if (!targetPath) {
      const result = await dialog.showSaveDialog(getWin()!, {
        filters: [{ name: 'imgplex Workflow', extensions: ['imgplex'] }],
        defaultPath: 'workflow.imgplex',
      })
      if (result.canceled || !result.filePath) return null
      targetPath = result.filePath
    }
    writeFileSync(targetPath, JSON.stringify({ version: '1.0', graph }, null, 2), 'utf-8')
    return targetPath
  })

  ipcMain.handle(IPC.WORKFLOW_LOAD, async () => {
    const result = await dialog.showOpenDialog(getWin()!, {
      properties: ['openFile'],
      filters: [
        { name: 'imgplex Workflow', extensions: ['imgplex'] },
        { name: 'All Files', extensions: ['*'] },
      ],
    })
    if (result.canceled || result.filePaths.length === 0) return null
    const filePath = result.filePaths[0]
    const raw = readFileSync(filePath, 'utf-8')
    const data = JSON.parse(raw) as Record<string, unknown>
    if (!data || typeof data !== 'object' || !data.graph) {
      throw new Error('Invalid workflow file: missing graph data')
    }
    return { graph: data.graph, filePath }
  })

  ipcMain.handle(IPC.WORKFLOW_OPEN_PATH, (_e, filePath: string) => {
    const raw = readFileSync(filePath, 'utf-8')
    const data = JSON.parse(raw) as Record<string, unknown>
    if (!data || typeof data !== 'object' || !data.graph) {
      throw new Error('Invalid workflow file: missing graph data')
    }
    return { graph: data.graph, filePath }
  })

  let isQuitting = false
  ipcMain.handle(IPC.APP_QUIT, () => {
    isQuitting = true
    app.quit()
  })

  // Intercept the window X button — ask renderer to confirm dirty state first
  const setupCloseInterception = () => {
    const win = getWin()
    if (!win) return
    win.on('close', (e) => {
      if (!isQuitting) {
        e.preventDefault()
        win.webContents.send(IPC.MENU_EXIT)
      }
    })
  }
  // The window may not exist yet; call after a tick so createWindow() has run
  setImmediate(setupCloseInterception)
}

export function registerDialogHandlers(getWin: () => BrowserWindow | null): void {
  ipcMain.handle(IPC.OPEN_IMAGES_DIALOG, async () => {
    const result = await dialog.showOpenDialog(getWin()!, {
      properties: ['openFile', 'multiSelections'],
      filters: [
        { name: 'All Supported Images', extensions: IMAGE_EXTENSIONS },
        { name: 'All Files', extensions: ['*'] },
      ],
    })
    return result.canceled ? [] : result.filePaths
  })

  ipcMain.handle(IPC.OPEN_FOLDER_DIALOG, async () => {
    const result = await dialog.showOpenDialog(getWin()!, {
      properties: ['openDirectory', 'createDirectory'],
    })
    return result.canceled ? null : result.filePaths[0]
  })

  ipcMain.handle(IPC.SCAN_FOLDER_DIALOG, async (
    _e,
    opts: { recursive: boolean; extensions: string[] },
  ) => {
    const result = await dialog.showOpenDialog(getWin()!, {
      properties: ['openDirectory'],
      buttonLabel: 'Select Folder',
    })
    if (result.canceled || !result.filePaths[0]) return []
    return scanFolder(result.filePaths[0], opts.recursive, new Set(opts.extensions.map(e => e.toLowerCase())))
  })
}

export function registerScanHandlers(): void {
  ipcMain.handle(IPC.SCAN_FOLDER, (
    _e,
    opts: { folderPath: string; recursive: boolean; extensions: string[] },
  ) => {
    return scanFolder(opts.folderPath, opts.recursive, new Set(opts.extensions.map(e => e.toLowerCase())))
  })
}

export function registerShellHandlers(): void {
  ipcMain.handle(IPC.SHELL_OPEN_EXTERNAL, (_e, url: string) => shell.openExternal(url))
  ipcMain.handle(IPC.SHELL_OPEN_PATH, (_e, folderPath: string) => shell.openPath(folderPath))
}

// ─── Text Output node ──────────────────────────────────────────────────────────

function valueToString(val: unknown): string {
  if (val === null || val === undefined) return ''
  if (typeof val === 'boolean') return val ? 'true' : 'false'
  if (typeof val === 'number') {
    return Number.isInteger(val) ? String(val) : parseFloat(val.toFixed(4)).toString()
  }
  if (Array.isArray(val)) {
    return (val as number[]).map((n) => parseFloat(Number(n).toFixed(4)).toString()).join(', ')
  }
  return String(val)
}

// prop_name and prop_path only need path.basename / the path itself — no magick spawn needed.
const PROP_PATH_ONLY = new Set(['prop_name', 'prop_path'])

interface ResolveContext {
  sorted: ReturnType<typeof topoSort>
  needsMagickMeta: boolean
  nodeMap: Map<string, ReturnType<typeof topoSort>[number]>
  edgesByTarget: Map<string, GraphEdge[]>
}

function buildResolveContext(graph: NodeGraph, registry: NodeRegistry): ResolveContext {
  const sorted = topoSort(graph.nodes, graph.edges)
  const needsMagickMeta = sorted.some((n) => {
    const exec = registry.get(n.data.definitionId)?.executor
    return exec?.startsWith('prop_') && !PROP_PATH_ONLY.has(exec)
  })
  const nodeMap = new Map(graph.nodes.map(n => [n.id, n]))
  const edgesByTarget = new Map<string, typeof graph.edges>()
  for (const edge of graph.edges) {
    const list = edgesByTarget.get(edge.target)
    if (list) list.push(edge)
    else edgesByTarget.set(edge.target, [edge])
  }
  return { sorted, needsMagickMeta, nodeMap, edgesByTarget }
}

/** Resolve param values for all non-image nodes in the graph for a single image. */
async function resolveParamsForImage(
  graph: NodeGraph,
  imagePath: string,
  registry: NodeRegistry,
  ctx?: ResolveContext
): Promise<Map<string, Record<string, unknown>>> {
  const { sorted, needsMagickMeta, nodeMap, edgesByTarget } = ctx ?? buildResolveContext(graph, registry)
  const resolvedParams = new Map<string, Record<string, unknown>>()
  // Track image output slots blocked by a gate whose condition is false: "nodeId:handleId"
  const blockedImageSlots = new Set<string>()

  // mean_value handles its own image read via loadImageChannelMean — it does NOT use meta.
  // Excluded from needsMagickMeta so images blocked by a gate skip loadImageMeta entirely.
  const meta = needsMagickMeta
    ? await loadImageMeta(imagePath)
    : buildEmptyImageMeta(imagePath)

  for (const node of sorted) {
    const def = registry.get(node.data.definitionId)
    if (!def) continue

    const inEdges = edgesByTarget.get(node.id) ?? []
    const rawParams: Record<string, unknown> = { ...(node.data.params ?? {}) }
    for (const edge of inEdges) {
      const th = edge.targetHandle ?? ''
      const sh = edge.sourceHandle ?? ''
      if (th.startsWith('param-in-') && sh.startsWith('param-out-')) {
        const srcResolved = resolvedParams.get(edge.source)
        if (srcResolved) {
          const sourceParam = sh.slice('param-out-'.length)
          if (sourceParam in srcResolved) rawParams[th.slice('param-in-'.length)] = srcResolved[sourceParam]
        }
      }
    }

    // Check whether this node's image input arrives from a blocked gate output
    const imageInputBlocked = inEdges.some(
      (e) => !e.targetHandle?.startsWith('param-') && blockedImageSlots.has(`${e.source}:${e.sourceHandle ?? 'out-0'}`)
    )

    // Gate: rawParams.condition is already resolved from param-wire edges above,
    // so this handles both wired conditions (upstream value propagated) and
    // static defaults (node's own param value). False → block image output.
    if (def.executor === 'gate') {
      if (!rawParams.condition) blockedImageSlots.add(`${node.id}:out-0`)
      resolvedParams.set(node.id, rawParams)
      continue
    }

    // If image input is blocked, propagate blocking to this node's image outputs and skip
    if (imageInputBlocked) {
      for (let i = 0; i < def.outputs.length; i++) {
        if (def.outputs[i].type === 'image' || def.outputs[i].type === 'mask') {
          blockedImageSlots.add(`${node.id}:out-${i}`)
        }
      }
      resolvedParams.set(node.id, rawParams)
      continue
    }

    const isImageNode =
      def.inputs.some((p) => p.type === 'image' || p.type === 'mask') ||
      def.outputs.some((p) => p.type === 'image' || p.type === 'mask')

    if (def.executor === 'mean_value') {
      try {
        // Trace in-0 back to channel_split to get the correct channel index
        const imgInEdge = inEdges.find(e => e.targetHandle === 'in-0')
        let value: number
        if (imgInEdge) {
          const srcNode = nodeMap.get(imgInEdge.source)
          const srcDef = registry.get(srcNode?.data.definitionId ?? '')
          const channelIdx = parseInt((imgInEdge.sourceHandle ?? '').replace('out-', ''), 10)
          if (srcDef?.executor === 'channel_split' && !isNaN(channelIdx)) {
            value = await loadImageChannelMean(imagePath, channelIdx)
          } else {
            value = await loadImageMean(imagePath)
          }
        } else {
          value = await loadImageMean(imagePath)
        }
        resolvedParams.set(node.id, { ...rawParams, value })
      } catch {
        resolvedParams.set(node.id, rawParams)
      }
      continue
    }

    const params = computeNodeParams(isImageNode ? undefined : def.executor, rawParams, meta)
    resolvedParams.set(node.id, params)
  }

  return resolvedParams
}

/** Compute the output lines for a Text Output node without writing anything. */
async function computeTextOutputLines(
  graph: NodeGraph,
  imagePaths: string[],
  nodeId: string,
  registry: NodeRegistry
): Promise<string[]> {
  const txNode = graph.nodes.find((n) => n.id === nodeId)
  if (!txNode) throw new Error('Text output node not found in graph.')

  const p = (txNode.data.params ?? {}) as Record<string, unknown>
  const separatorType = (p.separatorType as string) ?? 'comma'
  const customSep     = (p.customSeparator as string) ?? ''
  const portIds       = (p.portIds as string[]) ?? []

  if (imagePaths.length === 0) return []

  const connectedPorts = portIds.slice(0, -1).filter((pid) =>
    graph.edges.some((e) => e.target === nodeId && e.targetHandle === pid)
  )
  if (connectedPorts.length === 0) return []

  const portSources = connectedPorts.map((portId) => {
    const edge = graph.edges.find((e) => e.target === nodeId && e.targetHandle === portId)
    if (!edge || !edge.sourceHandle?.startsWith('param-out-')) return null
    return {
      sourceNodeId:   edge.source,
      sourceParamKey: edge.sourceHandle.slice('param-out-'.length),
    }
  })

  const conditionEdge = graph.edges.find(
    (e) => e.target === nodeId && e.targetHandle === 'txo-condition' && e.sourceHandle?.startsWith('param-out-')
  )
  const conditionSource = conditionEdge
    ? { sourceNodeId: conditionEdge.source, sourceParamKey: conditionEdge.sourceHandle?.slice('param-out-'.length) ?? '' }
    : null

  const sep = getSeparator(separatorType, customSep)

  const ctx = buildResolveContext(graph, registry)
  const allResolved = await Promise.all(
    imagePaths.map((imagePath) => resolveParamsForImage(graph, imagePath, registry, ctx))
  )

  const lines: string[] = []
  for (const resolvedParams of allResolved) {
    if (conditionSource) {
      const condResolved = resolvedParams.get(conditionSource.sourceNodeId)
      const condVal = condResolved?.[conditionSource.sourceParamKey]
      if (!condVal) continue
    }
    const values = portSources.map((ps) => {
      if (!ps) return ''
      const resolved = resolvedParams.get(ps.sourceNodeId)
      return valueToString(resolved?.[ps.sourceParamKey])
    })
    lines.push(values.join(sep))
  }

  return lines
}

export function registerTextOutputHandlers(
  registry: NodeRegistry,
  getWin: () => BrowserWindow | null
): void {
  ipcMain.handle(IPC.TEXT_OUTPUT_BROWSE, async () => {
    const result = await dialog.showSaveDialog(getWin()!, {
      title: 'Choose Output File',
      filters: [{ name: 'Text File', extensions: ['txt'] }, { name: 'All Files', extensions: ['*'] }],
      defaultPath: 'output.txt',
    })
    return result.canceled ? null : result.filePath
  })

  ipcMain.handle(
    IPC.TEXT_OUTPUT_PREVIEW,
    async (_e, { graph, imagePaths, nodeId }: { graph: NodeGraph; imagePaths: string[]; nodeId: string }) => {
      return computeTextOutputLines(graph, imagePaths, nodeId, registry)
    }
  )

  let _writeCancelled = false
  ipcMain.handle(IPC.TEXT_OUTPUT_WRITE_CANCEL, () => { _writeCancelled = true })

  ipcMain.handle(
    IPC.TEXT_OUTPUT_WRITE,
    async (
      _e,
      { graph, imagePaths, nodeId }: { graph: NodeGraph; imagePaths: string[]; nodeId: string }
    ) => {
      const txNode = graph.nodes.find((n) => n.id === nodeId)
      if (!txNode) throw new Error('Text output node not found in graph.')

      const p = (txNode.data.params ?? {}) as Record<string, unknown>
      const outputPath = (p.outputPath as string) ?? ''

      if (!outputPath) throw new Error('No output path set on the Text Output node.')
      if (imagePaths.length === 0) throw new Error('No images are loaded.')

      const separatorType = (p.separatorType as string) ?? 'comma'
      const customSep     = (p.customSeparator as string) ?? ''
      const portIds       = (p.portIds as string[]) ?? []

      const connectedPorts = portIds.slice(0, -1).filter((pid) =>
        graph.edges.some((e) => e.target === nodeId && e.targetHandle === pid)
      )
      if (connectedPorts.length === 0) throw new Error('No input ports are connected.')

      const portSources = connectedPorts.map((portId) => {
        const edge = graph.edges.find((e) => e.target === nodeId && e.targetHandle === portId)
        if (!edge || !edge.sourceHandle?.startsWith('param-out-')) return null
        return { sourceNodeId: edge.source, sourceParamKey: edge.sourceHandle.slice('param-out-'.length) }
      })

      const conditionEdge = graph.edges.find(
        (e) => e.target === nodeId && e.targetHandle === 'txo-condition' && e.sourceHandle?.startsWith('param-out-')
      )
      const conditionSource = conditionEdge
        ? { sourceNodeId: conditionEdge.source, sourceParamKey: conditionEdge.sourceHandle?.slice('param-out-'.length) ?? '' }
        : null

      const sep = getSeparator(separatorType, customSep)

      _writeCancelled = false
      const win = getWin()
      const lines: string[] = []
      const ctx = buildResolveContext(graph, registry)

      for (let i = 0; i < imagePaths.length; i++) {
        if (_writeCancelled) break
        const resolvedParams = await resolveParamsForImage(graph, imagePaths[i], registry, ctx)

        if (conditionSource) {
          const condVal = resolvedParams.get(conditionSource.sourceNodeId)?.[conditionSource.sourceParamKey]
          if (!condVal) {
            win?.webContents.send(IPC.TEXT_OUTPUT_WRITE_PROGRESS, { done: i + 1, total: imagePaths.length })
            continue
          }
        }

        const values = portSources.map((ps) => {
          if (!ps) return ''
          const resolved = resolvedParams.get(ps.sourceNodeId)
          return valueToString(resolved?.[ps.sourceParamKey])
        })
        lines.push(values.join(sep))
        win?.webContents.send(IPC.TEXT_OUTPUT_WRITE_PROGRESS, { done: i + 1, total: imagePaths.length })
      }

      if (_writeCancelled) throw new Error('CANCELLED')

      if (lines.length === 0) throw new Error('No lines were written — all images were filtered out by the condition.')
      if (!lines.some((l) => l.trim() !== '')) throw new Error('All values resolved to empty — file not written.')

      let filePath = outputPath
      if (!filePath.toLowerCase().endsWith('.txt')) filePath += '.txt'

      await fs.promises.writeFile(filePath, lines.join('\n') + '\n', 'utf-8')
      return filePath
    }
  )
}

// ── Atlas handlers ─────────────────────────────────────────────────────────────

interface AtlasConfig {
  outputPath: string
  rows: number
  cols: number
  cellWidth: number
  cellHeight: number
  sortBy: string
}

export function registerAtlasHandlers(getWin: () => BrowserWindow | null): void {
  // Browse for output file path
  ipcMain.handle(IPC.ATLAS_BROWSE, async () => {
    const result = await dialog.showSaveDialog(getWin()!, {
      title: 'Save Atlas As',
      filters: [
        { name: 'PNG Image', extensions: ['png'] },
        { name: 'WebP Image', extensions: ['webp'] },
        { name: 'JPEG Image', extensions: ['jpg', 'jpeg'] },
        { name: 'All Files', extensions: ['*'] },
      ],
      defaultPath: 'atlas.png',
    })
    return result.canceled ? null : result.filePath
  })

  // Generate the atlas using magick montage
  ipcMain.handle(IPC.ATLAS_GENERATE, async (
    _e,
    imagePaths: string[],
    config: AtlasConfig,
  ) => {
    const { outputPath, rows, cols, cellWidth, cellHeight, sortBy } = config

    if (!outputPath?.trim()) throw new Error('No output file path specified.')
    if (imagePaths.length === 0) throw new Error('No images loaded.')

    // Sort images
    const sorted = sortBy === 'name'
      ? [...imagePaths].sort((a, b) =>
          path.basename(a).localeCompare(path.basename(b), undefined, { numeric: true, sensitivity: 'base' })
        )
      : imagePaths

    // Truncate to grid capacity
    const selected = sorted.slice(0, rows * cols)

    const magick = getMagickBinary()
    // -geometry WxH!+0+0 : force exact cell size (stretch), 0px border between tiles
    // -background none   : transparent fill for unfilled cells (PNG/WebP)
    // -tile COLSxROWS    : fixed grid layout
    const args = [
      'montage',
      ...selected,
      '-tile',       `${cols}x${rows}`,
      '-geometry',   `${cellWidth}x${cellHeight}!+0+0`,
      '-background', 'none',
      outputPath,
    ]

    await new Promise<void>((resolve, reject) => {
      const child = spawn(magick, args, { stdio: ['ignore', 'ignore', 'pipe'] })
      let stderr = ''
      child.stderr?.on('data', (d: Buffer) => { stderr += d.toString() })
      child.on('close', (code) => {
        if (code === 0) resolve()
        else reject(new Error(`magick montage exited with code ${code}: ${stderr.slice(0, 300).trim()}`))
      })
      child.on('error', (err: Error) => reject(new Error(`Failed to launch magick: ${err.message}`)))
    })

    return outputPath
  })
}
