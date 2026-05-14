// Graph resolution, topological sort, and ImageMagick execution
import { spawn } from 'node:child_process'
import crypto from 'node:crypto'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'

import type { GraphEdge, GraphNode, ImageInfo, NodeGraph, Progress } from '../../shared/types.js'
import { PREVIEW_MAX_EDGE_PX } from '../../shared/constants.js'
import type { NodeRegistry } from '../nodes/registry.js'
import { buildCommandArgs, buildCommandArgsFromJs } from './command-builder.js'
import { getExecutor } from './executorRegistry.js'
import './imageNodeExecutors.js'
import { PreviewCache } from './cache.js'
import { computeNodeParams, loadImageMeta, loadImageMean, loadImageChannelMean, loadMultipleChannelMeans, getSeparator, buildEmptyImageMeta, type ImageMeta } from './executor-compute.js'
import { getMagickBinary } from './magick-path.js'
import { cliScriptPS, cliScriptBash, cliScriptCmd } from './executor-cli.js'
import { computeNewName, type RenameParams } from '../../shared/renameUtils.js'
import { timings } from './timing.js'

// ─── Temp directory ───────────────────────────────────────────────────────────

const TEMP_DIR = path.join(os.tmpdir(), 'imgplex-preview')
fs.mkdirSync(TEMP_DIR, { recursive: true })

// ─── Topological sort (Kahn's algorithm) ─────────────────────────────────────

export function topoSort(nodes: GraphNode[], edges: GraphEdge[]): GraphNode[] {
  const inDegree = new Map<string, number>()
  const adj = new Map<string, string[]>()

  for (const node of nodes) {
    inDegree.set(node.id, 0)
    adj.set(node.id, [])
  }
  for (const edge of edges) {
    adj.get(edge.source)?.push(edge.target)
    inDegree.set(edge.target, (inDegree.get(edge.target) ?? 0) + 1)
  }

  const queue = [...inDegree.entries()].filter(([, d]) => d === 0).map(([id]) => id)
  const nodeMap = new Map(nodes.map((n) => [n.id, n]))
  const sorted: GraphNode[] = []

  while (queue.length > 0) {
    const id = queue.shift()!
    const node = nodeMap.get(id)
    if (node) sorted.push(node)
    for (const neighbor of adj.get(id) ?? []) {
      const deg = (inDegree.get(neighbor) ?? 0) - 1
      inDegree.set(neighbor, deg)
      if (deg === 0) queue.push(neighbor)
    }
  }

  return sorted
}

// ─── Set batch utilities ───────────────────────────────────────────────────────

/** Group image paths by {prefix}{middle}{suffix} naming convention. */
export function groupBySetPattern(
  imagePaths: string[],
  prefix: string,
  suffixes: string[]
): Map<string, Record<string, string>> {
  const map = new Map<string, Record<string, string>>()
  for (const imgPath of imagePaths) {
    const ext  = path.extname(imgPath)
    const base = path.basename(imgPath, ext)
    if (prefix && !base.startsWith(prefix)) continue
    const rest = base.slice(prefix.length)
    for (const s of suffixes) {
      if (!s) continue
      if (rest.endsWith(s)) {
        const mid = rest.slice(0, rest.length - s.length)
        if (!map.has(mid)) map.set(mid, {})
        map.get(mid)![s] = imgPath
        break
      }
    }
  }
  return map
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

function spawnMagick(args: string[], bucket?: { add(ms: number): void }): Promise<void> {
  const t0 = bucket ? Date.now() : 0
  return new Promise((resolve, reject) => {
    const proc = spawn(getMagickBinary(), args)
    const stderr: string[] = []
    proc.stderr.on('data', (chunk: Buffer) => stderr.push(chunk.toString()))
    proc.on('close', (code) => {
      if (bucket) bucket.add(Date.now() - t0)
      if (code === 0) resolve()
      else reject(new Error(`magick exited ${code}: ${stderr.join('').trim()}`))
    })
    proc.on('error', (err) => {
      if (bucket) bucket.add(Date.now() - t0)
      reject(new Error(`Failed to spawn magick: ${err.message}. Is ImageMagick v7 in PATH?`))
    })
  })
}

function shortHash(data: string): string {
  return crypto.createHash('md5').update(data).digest('hex').slice(0, 8)
}

async function fileToDataUrl(filePath: string): Promise<string> {
  const data = await fs.promises.readFile(filePath)
  const ext = path.extname(filePath).slice(1).toLowerCase()
  const mime =
    ext === 'jpg' || ext === 'jpeg'
      ? 'image/jpeg'
      : ext === 'webp'
        ? 'image/webp'
        : 'image/png'
  return `data:${mime};base64,${data.toString('base64')}`
}

// ─── Native image header parser ───────────────────────────────────────────────
// Parses JPEG/PNG/WEBP dimensions from the file header without spawning magick.
// Returns null for unrecognized formats or truncated/corrupt headers.

const JPEG_SOF_MARKERS = new Set([
  0xC0, 0xC1, 0xC2, 0xC3, 0xC5, 0xC6, 0xC7, 0xC9, 0xCA, 0xCB, 0xCD, 0xCE, 0xCF,
])
const TGA_VALID_TYPES = new Set([0, 1, 2, 3, 9, 10, 11])
const HEADER_READ_BYTES = 131072 // 128 KB — covers EXIF/APP markers before SOF in virtually all JPEGs

interface HeaderDims {
  width: number
  height: number
  isJpeg: boolean
  format: string  // inferred from header/extension
}

async function readHeaderDimensions(filePath: string): Promise<HeaderDims | null> {
  let fd: fs.promises.FileHandle | null = null
  try {
    fd = await fs.promises.open(filePath, 'r')
    const buf = Buffer.alloc(HEADER_READ_BYTES)
    const { bytesRead } = await fd.read(buf, 0, HEADER_READ_BYTES, 0)
    if (bytesRead < 12) return null

    // ── PNG ──────────────────────────────────────────────────────────────────
    if (buf[0] === 0x89 && buf[1] === 0x50 && buf[2] === 0x4E && buf[3] === 0x47) {
      if (bytesRead < 24) return null
      return { width: buf.readUInt32BE(16), height: buf.readUInt32BE(20), isJpeg: false, format: 'PNG' }
    }

    // ── BMP ──────────────────────────────────────────────────────────────────
    if (buf[0] === 0x42 && buf[1] === 0x4D) { // "BM"
      if (bytesRead < 26) return null
      const w = Math.abs(buf.readInt32LE(18))
      const h = Math.abs(buf.readInt32LE(22))
      if (w > 0 && h > 0) return { width: w, height: h, isJpeg: false, format: 'BMP' }
      return null
    }

    // ── WEBP ─────────────────────────────────────────────────────────────────
    if (buf[0] === 0x52 && buf[1] === 0x49 && buf[2] === 0x46 && buf[3] === 0x46 &&
        buf[8] === 0x57 && buf[9] === 0x45 && buf[10] === 0x42 && buf[11] === 0x50) {
      if (bytesRead < 30) return null
      const chunkType = buf.subarray(12, 16).toString('ascii')
      if (chunkType === 'VP8 ') {
        if (buf[23] === 0x9D && buf[24] === 0x01 && buf[25] === 0x2A) {
          const w = (buf[26] | (buf[27] << 8)) & 0x3FFF
          const h = (buf[28] | (buf[29] << 8)) & 0x3FFF
          if (w > 0 && h > 0) return { width: w, height: h, isJpeg: false, format: 'WEBP' }
        }
        return null
      }
      if (chunkType === 'VP8L') {
        if (bytesRead < 25 || buf[20] !== 0x2F) return null
        const val = buf.readUInt32LE(21)
        return { width: (val & 0x3FFF) + 1, height: ((val >> 14) & 0x3FFF) + 1, isJpeg: false, format: 'WEBP' }
      }
      if (chunkType === 'VP8X') {
        const w = (buf[24] | (buf[25] << 8) | (buf[26] << 16)) + 1
        const h = (buf[27] | (buf[28] << 8) | (buf[29] << 16)) + 1
        return { width: w, height: h, isJpeg: false, format: 'WEBP' }
      }
      return null
    }

    // ── JPEG ─────────────────────────────────────────────────────────────────
    if (buf[0] === 0xFF && buf[1] === 0xD8) {
      let pos = 2
      while (pos + 1 < bytesRead) {
        if (buf[pos] !== 0xFF) break
        const marker = buf[pos + 1]
        pos += 2
        if (marker === 0xD8 || marker === 0xD9 || (marker >= 0xD0 && marker <= 0xD7)) continue
        if (marker === 0xDA) break
        if (pos + 1 >= bytesRead) break
        const length = buf.readUInt16BE(pos)
        if (length < 2) break
        if (JPEG_SOF_MARKERS.has(marker) && pos + 6 < bytesRead) {
          const h = buf.readUInt16BE(pos + 3)
          const w = buf.readUInt16BE(pos + 5)
          if (w > 0 && h > 0) return { width: w, height: h, isJpeg: true, format: 'JPEG' }
        }
        pos += length
      }
      return null
    }

    // ── TGA ──────────────────────────────────────────────────────────────────
    // No universal magic bytes — detect by extension + header validation
    const extLower = path.extname(filePath).toLowerCase()
    if (extLower === '.tga' || extLower === '.targa') {
      if (bytesRead < 16) return null
      if (!TGA_VALID_TYPES.has(buf[2])) return null // image type must be valid
      const w = buf.readUInt16LE(12)
      const h = buf.readUInt16LE(14)
      if (w > 0 && h > 0) return { width: w, height: h, isJpeg: false, format: 'TGA' }
      return null
    }

    return null // unrecognized format
  } catch {
    return null
  } finally {
    await fd?.close().catch(() => {})
  }
}

// ─── PipelineExecutor ─────────────────────────────────────────────────────────

export class PipelineExecutor {
  private previewCache = new PreviewCache()
  private _metaCache   = new Map<string, { width: number; height: number; format: string }>()
  private _batchCancelled = false

  cancelBatch(): void { this._batchCancelled = true }

  // ── Image metadata ──────────────────────────────────────────────────────────

  async loadImage(imagePath: string): Promise<ImageInfo> {
    const output = await new Promise<string>((resolve, reject) => {
      const proc = spawn(getMagickBinary(), ['identify', '-format', '%w %h %m', imagePath])
      const out: string[] = []
      const err: string[] = []
      proc.stdout.on('data', (c: Buffer) => out.push(c.toString()))
      proc.stderr.on('data', (c: Buffer) => err.push(c.toString()))
      proc.on('close', (code) => {
        if (code === 0) resolve(out.join('').trim())
        else reject(new Error(err.join('').trim()))
      })
      proc.on('error', reject)
    })

    // identify returns one line per frame; take the first
    const firstLine = output.split('\n')[0]
    const [widthStr, heightStr, format] = firstLine.split(' ')
    const stat = await fs.promises.stat(imagePath)

    return {
      path: imagePath,
      name: path.basename(imagePath),
      width:  parseInt(widthStr  ?? '0', 10) || 0,
      height: parseInt(heightStr ?? '0', 10) || 0,
      format: format ?? 'UNKNOWN',
      sizeBytes: stat.size,
    }
  }

  // ── Load image + thumbnail in one magick call ───────────────────────────────

  async loadImageWithThumbnail(
    imagePath: string,
    size: number
  ): Promise<ImageInfo & { thumbnailDataUrl: string }> {
    const hash      = shortHash(imagePath)
    const thumbPath = path.join(TEMP_DIR, `thumb_${hash}_${size}.webp`)

    // Check thumb cache first; only fetch srcStat if thumb actually exists
    const thumbStat = await fs.promises.stat(thumbPath).catch(() => null)
    const cached    = this._metaCache.get(imagePath)

    let needsRegen = !thumbStat
    if (thumbStat) {
      const srcStat = await fs.promises.stat(imagePath).catch(() => null)
      needsRegen = !!(srcStat && thumbStat.mtimeMs < srcStat.mtimeMs)
    }

    let width: number, height: number, format: string

    if (!needsRegen && cached) {
      // Full cache hit — zero magick calls
      ;({ width, height, format } = cached)
      const [fileStat, dataUrl] = await Promise.all([
        fs.promises.stat(imagePath).catch(() => null),
        fileToDataUrl(thumbPath),
      ])
      return {
        path: imagePath, name: path.basename(imagePath),
        width, height, format,
        sizeBytes: fileStat?.size ?? 0,
        thumbnailDataUrl: dataUrl,
      }
    }

    // Cache miss — parse header natively first to enable JPEG DCT size hint
    await fs.promises.mkdir(TEMP_DIR, { recursive: true })

    const hdr = await readHeaderDimensions(imagePath)

    if (hdr) {
      // Fast path: dimensions known from header — spawn magick with optional JPEG size hint
      const magickArgs: string[] = []
      if (hdr.isJpeg) {
        // Tell libjpeg to decode at 1/8 scale; picks smallest factor >= requested size,
        // giving up to ~64× less data to decompress for large images.
        magickArgs.push('-define', `jpeg:size=${size * 2}x${size * 2}`)
      }
      magickArgs.push(
        `${imagePath}[0]`,
        '-thumbnail', `${size}x${size}>`,
        '-quality', '85',
        thumbPath,
      )
      await new Promise<void>((resolve, reject) => {
        const proc = spawn(getMagickBinary(), magickArgs, { env: { ...process.env, MAGICK_THREAD_LIMIT: '1' } })
        const stderr: string[] = []
        proc.stderr.on('data', (d: Buffer) => stderr.push(d.toString()))
        proc.on('close', (code) => {
          if (code === 0) resolve()
          else reject(new Error(`magick exited ${code}: ${stderr.join('').trim()}`))
        })
        proc.on('error', (err) => reject(new Error(`Failed to spawn magick: ${err.message}. Is ImageMagick v7 in PATH?`)))
      })
      width  = hdr.width
      height = hdr.height
      format = hdr.format
    } else {
      // Slow path (RAW, TIFF, PSD, GIF, …): combined -print + thumbnail spawn
      let stdout = ''
      await new Promise<void>((resolve, reject) => {
        const proc = spawn(
          getMagickBinary(),
          [
            `${imagePath}[0]`,
            '-print', '%w %h %m\n',
            '-thumbnail', `${size}x${size}>`,
            '-quality', '85',
            thumbPath,
          ],
          { env: { ...process.env, MAGICK_THREAD_LIMIT: '1' } }
        )
        proc.stdout.on('data', (d: Buffer) => { stdout += d.toString() })
        const stderr: string[] = []
        proc.stderr.on('data', (d: Buffer) => stderr.push(d.toString()))
        proc.on('close', (code) => {
          if (code === 0) resolve()
          else reject(new Error(`magick exited ${code}: ${stderr.join('').trim()}`))
        })
        proc.on('error', (err) => reject(new Error(`Failed to spawn magick: ${err.message}. Is ImageMagick v7 in PATH?`)))
      })
      const [ws, hs, fmt] = stdout.trim().split(' ')
      width  = parseInt(ws, 10) || 0
      height = parseInt(hs, 10) || 0
      format = fmt ?? 'UNKNOWN'
    }

    this._metaCache.set(imagePath, { width, height, format })

    const [fileStat, dataUrl] = await Promise.all([
      fs.promises.stat(imagePath).catch(() => null),
      fileToDataUrl(thumbPath),
    ])

    return {
      path: imagePath,
      name: path.basename(imagePath),
      width, height, format,
      sizeBytes: fileStat?.size ?? 0,
      thumbnailDataUrl: dataUrl,
    }
  }

  // ── Batch load + thumbnail (amortizes process-spawn cost) ──────────────────
  // Processes imagePaths.length images in a single magick invocation when possible.
  // Fast-path images (PNG/BMP/TGA/WEBP/JPEG headers parseable) are batched into one
  // spawn.  Images whose format magick must determine (PSD, TIFF, …) fall back to
  // individual spawns with -print.

  async loadImageWithThumbnailBatch(
    imagePaths: string[],
    size: number
  ): Promise<Array<ImageInfo & { thumbnailDataUrl: string }>> {
    if (imagePaths.length === 0) return []

    await fs.promises.mkdir(TEMP_DIR, { recursive: true })

    const hashes     = imagePaths.map(p => shortHash(p))
    const thumbPaths = hashes.map(h => path.join(TEMP_DIR, `thumb_${h}_${size}.webp`))

    // ── Phase 1: parallel cache + header checks ──────────────────────────────
    const headerStartMs = Date.now()
    const [thumbStats, hdrs] = await Promise.all([
      Promise.all(thumbPaths.map(tp => fs.promises.stat(tp).catch(() => null))),
      Promise.all(imagePaths.map(p => readHeaderDimensions(p))),
    ])
    const headerTotalMs = Date.now() - headerStartMs

    // Only stat source file when thumb exists (avoids extra stat on cache miss)
    const needsRegen: boolean[] = await Promise.all(
      imagePaths.map(async (p, i) => {
        if (!thumbStats[i]) return true
        const srcStat = await fs.promises.stat(p).catch(() => null)
        return !!(srcStat && thumbStats[i]!.mtimeMs < srcStat.mtimeMs)
      })
    )

    // ── Phase 2: classify each image ─────────────────────────────────────────
    const fastMisses:     number[] = []  // thumb regen needed, header parsed → batch spawn
    const slowMisses:     number[] = []  // thumb regen needed, no header     → individual spawn
    const metaOnlyMisses: number[] = []  // thumb cached but _metaCache empty → identify only

    for (let i = 0; i < imagePaths.length; i++) {
      if (needsRegen[i]) {
        if (hdrs[i]) fastMisses.push(i)
        else         slowMisses.push(i)
      } else if (!this._metaCache.has(imagePaths[i])) {
        // Thumb exists but app restarted — _metaCache is empty; recover metadata.
        if (hdrs[i]) {
          // Fast path: header already read in Phase 1, no spawn needed.
          this._metaCache.set(imagePaths[i], { width: hdrs[i]!.width, height: hdrs[i]!.height, format: hdrs[i]!.format })
        } else {
          // Slow path (PSD, TIFF, …): need a meta-only identify call.
          metaOnlyMisses.push(i)
        }
      }
    }

    // ── Phase 2b: meta-only identify for slow-path cached images ─────────────
    for (const i of metaOnlyMisses) {
      try {
        const output = await new Promise<string>((resolve, reject) => {
          const proc = spawn(getMagickBinary(), ['identify', '-format', '%w %h %m\n', `${imagePaths[i]}[0]`])
          const out: string[] = []
          proc.stdout.on('data', (d: Buffer) => out.push(d.toString()))
          proc.on('close', (code) => code === 0 ? resolve(out.join('')) : reject(new Error(`identify exited ${code}`)))
          proc.on('error', (err) => reject(err))
        })
        const [ws, hs, fmt] = output.split('\n')[0].split(' ')
        this._metaCache.set(imagePaths[i], {
          width:  parseInt(ws, 10) || 0,
          height: parseInt(hs, 10) || 0,
          format: fmt ?? 'UNKNOWN',
        })
      } catch (err) {
        console.error('[import] Failed to get meta for cached image:', imagePaths[i], err)
      }
    }

    // ── Phase 3: single batch spawn for all fast-path misses ─────────────────
    let fastThumbMs = 0
    if (fastMisses.length > 0) {
      // Build one magick command: each image reads, thumbnails, writes, then +delete
      // (last image writes directly to its thumbPath as the final output)
      const batchArgs: string[] = []
      for (let j = 0; j < fastMisses.length; j++) {
        const i = fastMisses[j]
        batchArgs.push(`${imagePaths[i]}[0]`)
        batchArgs.push('-thumbnail', `${size}x${size}>`, '-quality', '85')
        if (j < fastMisses.length - 1) batchArgs.push('-write', thumbPaths[i], '+delete')
        else                            batchArgs.push(thumbPaths[i])
      }

      let batchOk = true
      const batchSpawnT0 = Date.now()
      try {
        await new Promise<void>((resolve, reject) => {
          const proc = spawn(getMagickBinary(), batchArgs, { env: { ...process.env, MAGICK_THREAD_LIMIT: '1' } })
          const stderr: string[] = []
          proc.stderr.on('data', (d: Buffer) => stderr.push(d.toString()))
          proc.on('close', code => code === 0 ? resolve() : reject(new Error(`magick exited ${code}: ${stderr.join('').trim()}`)))
          proc.on('error', err => reject(new Error(`Failed to spawn magick: ${err.message}. Is ImageMagick v7 in PATH?`)))
        })
      } catch (err) {
        batchOk = false
        console.warn('[import] Batch spawn failed, falling back to individual processing:', (err as Error).message)
        for (const i of fastMisses) slowMisses.push(i)
      }

      if (batchOk) {
        fastThumbMs = Date.now() - batchSpawnT0
        for (const i of fastMisses) {
          this._metaCache.set(imagePaths[i], { width: hdrs[i]!.width, height: hdrs[i]!.height, format: hdrs[i]!.format })
        }
      }
    }

    // ── Phase 4: individual spawns for slow-path misses ──────────────────────
    // These are rare (PSD, TIFF, …) so sequential processing is fine.
    const slowThumbMs = new Map<number, number>()
    for (const i of slowMisses) {
      try {
        let stdout = ''
        const slowT0 = Date.now()
        await new Promise<void>((resolve, reject) => {
          const proc = spawn(
            getMagickBinary(),
            [
              `${imagePaths[i]}[0]`,
              '-print', '%w %h %m\n',
              '-thumbnail', `${size}x${size}>`,
              '-quality', '85',
              thumbPaths[i],
            ],
            { env: { ...process.env, MAGICK_THREAD_LIMIT: '1' } }
          )
          proc.stdout.on('data', (d: Buffer) => { stdout += d.toString() })
          const stderr: string[] = []
          proc.stderr.on('data', (d: Buffer) => stderr.push(d.toString()))
          proc.on('close', code => code === 0 ? resolve() : reject(new Error(`magick exited ${code}: ${stderr.join('').trim()}`)))
          proc.on('error', err => reject(new Error(`Failed to spawn magick: ${err.message}. Is ImageMagick v7 in PATH?`)))
        })
        slowThumbMs.set(i, Date.now() - slowT0)
        const [ws, hs, fmt] = stdout.trim().split(' ')
        this._metaCache.set(imagePaths[i], {
          width: parseInt(ws, 10) || 0,
          height: parseInt(hs, 10) || 0,
          format: fmt ?? 'UNKNOWN',
        })
      } catch (err) {
        console.error('[import] Failed to process:', imagePaths[i], err)
      }
    }

    // ── Phase 5: parallel stat + thumbnail read for all images ───────────────
    const fastMissSet  = new Set(fastMisses)
    const slowMissSet  = new Set(slowMisses)
    const avgHeaderPerImage = imagePaths.length > 0 ? Math.round(headerTotalMs / imagePaths.length) : 0
    const avgFastThumb = fastMisses.length > 0 ? Math.round(fastThumbMs / fastMisses.length) : 0

    return Promise.all(imagePaths.map(async (imagePath, i) => {
      const meta = this._metaCache.get(imagePath) ?? { width: 0, height: 0, format: 'UNKNOWN' }
      const [fileStat, thumbnailDataUrl] = await Promise.all([
        fs.promises.stat(imagePath).catch(() => null),
        fileToDataUrl(thumbPaths[i]),
      ])
      if (timings.enabled) {
        const pathType: 'fast' | 'slow' | 'cached' =
          fastMissSet.has(i) ? 'fast' : slowMissSet.has(i) ? 'slow' : 'cached'
        timings.recordImportImage({
          file:          imagePath,
          headerParseMs: avgHeaderPerImage,
          identifyMs:    slowMissSet.has(i) ? (slowThumbMs.get(i) ?? 0) : 0,
          thumbnailMs:   fastMissSet.has(i) ? avgFastThumb : (slowThumbMs.get(i) ?? 0),
          pathType,
        })
      }
      return {
        path: imagePath,
        name: path.basename(imagePath),
        ...meta,
        sizeBytes: fileStat?.size ?? 0,
        thumbnailDataUrl,
      }
    }))
  }

  // ── Thumbnail ───────────────────────────────────────────────────────────────

  async generateThumbnail(imagePath: string, size: number): Promise<string> {
    const hash = shortHash(imagePath)
    const thumbPath = path.join(TEMP_DIR, `thumb_${hash}_${size}.webp`)

    // Only regenerate if missing or source is newer
    const [srcStat, thumbStat] = await Promise.all([
      fs.promises.stat(imagePath).catch(() => null),
      fs.promises.stat(thumbPath).catch(() => null),
    ])

    if (!thumbStat || (srcStat && thumbStat.mtimeMs < srcStat.mtimeMs)) {
      await fs.promises.mkdir(TEMP_DIR, { recursive: true })
      // [0] selects only the first frame — without it, multi-frame images (e.g.
      // JPEGs with embedded EXIF thumbnails) cause magick to write numbered
      // output files instead of the expected path.
      await spawnMagick([`${imagePath}[0]`, '-thumbnail', `${size}x${size}>`, '-quality', '85', thumbPath])
    }

    return fileToDataUrl(thumbPath)
  }

  // ── Preview pipeline ────────────────────────────────────────────────────────
  //
  // Runs each node individually (one magick invocation per node) so intermediate
  // outputs can be cached and re-used on incremental edits.
  //
  // When fromNodeId is provided, the cache for that node and all downstream nodes
  // is invalidated before execution. Earlier nodes will still hit the cache.

  async executePreview(
    graph: NodeGraph,
    imagePath: string,
    registry: NodeRegistry,
    fromNodeId?: string
  ): Promise<{ dataUrl: string; propParams: Record<string, Record<string, unknown>> }> {
    await fs.promises.mkdir(TEMP_DIR, { recursive: true })

    // Always downscale proportionally first — this keeps the data URL small and
    // ensures the image has the correct aspect ratio (no square padding).
    const inputHash = shortHash(imagePath)
    const downscaledPath = path.join(TEMP_DIR, `preview_input_${inputHash}.png`)
    const downscaledExists = await fs.promises
      .access(downscaledPath)
      .then(() => true)
      .catch(() => false)

    if (!downscaledExists) {
      try {
        await spawnMagick([
          `${imagePath}[0]`,
          '-resize',
          `${PREVIEW_MAX_EDGE_PX}x${PREVIEW_MAX_EDGE_PX}>`,
          downscaledPath,
        ])
      } catch (err) {
        // Concurrent preview request may have written the file first; retry access
        const nowExists = await fs.promises.access(downscaledPath).then(() => true).catch(() => false)
        if (!nowExists) throw err
      }
    }

    // No nodes — return the proportionally-downscaled source directly.
    if (graph.nodes.length === 0) return { dataUrl: await fileToDataUrl(downscaledPath), propParams: {} }

    const sorted = topoSort(graph.nodes, graph.edges)

    // Invalidate stale cache entries when a specific node changed
    if (fromNodeId) {
      const changeIdx = sorted.findIndex((n) => n.id === fromNodeId)
      if (changeIdx >= 0) {
        for (let i = changeIdx; i < sorted.length; i++) {
          this.previewCache.invalidateFrom(sorted[i].id)
        }
      }
    }

    // Multi-stream image buffer: keyed "nodeId:out-N" → temp file path.
    // This allows fan-out (channel_split) and fan-in (channel_merge) topologies.
    const imageBuffers = new Map<string, string>()
    imageBuffers.set('workflow-input:out-0', downscaledPath)

    // In set mode, seed companion images for each suffix port of the setInputNode.
    // Infer middle name from the selected imagePath so we can locate sibling files.
    const previewSetNode = sorted.find(n => n.data.definitionId === 'process_as_set')
    if (previewSetNode) {
      const setPrefix   = String(previewSetNode.data.params?.prefix ?? '')
      const setSuffixes = Array.isArray(previewSetNode.data.params?.suffixes)
        ? (previewSetNode.data.params!.suffixes as string[]).filter(Boolean)
        : []
      const imgExt  = path.extname(imagePath)
      const imgBase = path.basename(imagePath, imgExt)
      const imgDir  = path.dirname(imagePath)
      const imgRest = imgBase.startsWith(setPrefix) ? imgBase.slice(setPrefix.length) : imgBase
      let middleName: string | null = null
      for (const s of setSuffixes) {
        if (imgRest.endsWith(s)) { middleName = imgRest.slice(0, imgRest.length - s.length); break }
      }
      if (middleName !== null) {
        for (let i = 0; i < setSuffixes.length; i++) {
          const companionPath = path.join(imgDir, setPrefix + middleName + setSuffixes[i] + imgExt)
          const companionExists = await fs.promises.access(companionPath).then(() => true).catch(() => false)
          if (!companionExists) continue
          const companionHash = shortHash(companionPath)
          const companionDownscaled = path.join(TEMP_DIR, `preview_input_${companionHash}.png`)
          if (!(await fs.promises.access(companionDownscaled).then(() => true).catch(() => false))) {
            try {
              await spawnMagick([`${companionPath}[0]`, '-resize', `${PREVIEW_MAX_EDGE_PX}x${PREVIEW_MAX_EDGE_PX}>`, companionDownscaled])
            } catch { /* companion downscale failure is non-fatal */ }
          }
          if (await fs.promises.access(companionDownscaled).then(() => true).catch(() => false)) {
            imageBuffers.set(`${previewSetNode.id}:out-${i}`, companionDownscaled)
          }
        }
      }
    }

    // Resolve the file path feeding into a node's Nth image input by following edges.
    const getImgBuf = (nodeId: string, inputIdx: number): string => {
      const edge = graph.edges.find(
        (e) => e.target === nodeId && e.targetHandle === `in-${inputIdx}`
      )
      if (!edge) return downscaledPath
      return imageBuffers.get(`${edge.source}:${edge.sourceHandle ?? 'out-0'}`) ?? downscaledPath
    }

    const fileExists = (p: string) => fs.promises.access(p).then(() => true).catch(() => false)

    // Tracks resolved params per node so downstream param-wire consumers can read them.
    const resolvedParams = new Map<string, Record<string, unknown>>()

    // Load image metadata lazily — only when at least one Properties node is present.
    const needsMeta = sorted.some((n) => {
      const def = registry.get(n.data.definitionId)
      return def?.needs_image_meta === true || def?.executor === 'mean_value'
    })
    const meta = needsMeta ? await loadImageMeta(imagePath) : undefined

    for (const node of sorted) {
      // process_as_set is a source node — buffers are pre-seeded above; skip processing.
      if (node.data.definitionId === 'process_as_set') continue
      const def = registry.get(node.data.definitionId)
      if (!def) {
        // Silently skip framework-internal nodes (workflow-input/output, groups)
        // which have no registry entry. Only warn for genuinely unknown IDs.
        if (node.data.definitionId)
          console.warn(`[executor] Unknown node definition: ${node.data.definitionId}`)
        continue
      }

      // Inspector values, overridden by any incoming param-wire connections
      const rawParams: Record<string, unknown> = { ...(node.data.params ?? {}) }
      for (const edge of graph.edges) {
        if (edge.target !== node.id) continue
        const th = edge.targetHandle ?? ''
        const sh = edge.sourceHandle ?? ''
        if (sh.startsWith('param-out-')) {
          const sourceParam = sh.slice('param-out-'.length)
          const srcResolved = resolvedParams.get(edge.source)
          if (srcResolved && sourceParam in srcResolved) {
            if (th.startsWith('param-in-')) {
              rawParams[th.slice('param-in-'.length)] = srcResolved[sourceParam]
            } else if (th.startsWith('txo-')) {
              // text_output dynamic ports: txo-condition → condition param; txo-N → _txo_N slot
              rawParams[`_txo_${th.slice('txo-'.length)}`] = srcResolved[sourceParam]
            }
          }
        }
      }

      // Run pure math/logic/value computation; image nodes return params unchanged
      const isImageNode = def.inputs.some((p) => p.type === 'image' || p.type === 'mask') ||
                          def.outputs.some((p) => p.type === 'image' || p.type === 'mask')
      // Inject compute_js body for pure-value nodes that use the inline JS path
      const computeInput = (!isImageNode && def.compute_js)
        ? { ...rawParams, __compute_js__: def.compute_js }
        : rawParams
      const params = computeNodeParams(isImageNode ? undefined : def.executor, computeInput, meta)
      resolvedParams.set(node.id, params)

      // Pure value/math/logic nodes don't touch the image pipeline
      if (!isImageNode) continue

      // ── Channel Split ──────────────────────────────────────────────────────
      if (def.executor === 'channel_split') {
        const inputPath = getImgBuf(node.id, 0)
        const nodeHash = shortHash(inputPath + JSON.stringify(params))
        const CHANNELS = [
          { suffix: '__r', magickChannel: 'Red'   },
          { suffix: '__g', magickChannel: 'Green' },
          { suffix: '__b', magickChannel: 'Blue'  },
          { suffix: '__a', magickChannel: 'Alpha' },
        ] as const
        for (let i = 0; i < CHANNELS.length; i++) {
          const { suffix, magickChannel } = CHANNELS[i]
          const cacheKey = node.id + suffix
          const cached = this.previewCache.get(cacheKey, nodeHash)
          let outPath: string
          if (cached && await fileExists(cached)) {
            outPath = cached
          } else {
            outPath = path.join(TEMP_DIR, `preview_${node.id}${suffix}_${nodeHash}.png`)
            await spawnMagick([inputPath, '-channel', magickChannel, '-separate', outPath])
            this.previewCache.set(cacheKey, nodeHash, outPath)
          }
          imageBuffers.set(`${node.id}:out-${i}`, outPath)
        }
        continue
      }

      // ── Channel Merge ──────────────────────────────────────────────────────
      if (def.executor === 'channel_merge') {
        const refPath = imageBuffers.get('workflow-input:out-0') ?? downscaledPath

        // Returns the image for a channel port, or a solid-black placeholder when unconnected.
        const resolveChannel = async (inputIdx: number): Promise<string> => {
          const imgEdge = graph.edges.find(
            (e) => e.target === node.id && e.targetHandle === `in-${inputIdx}`,
          )
          if (imgEdge) {
            // Float value wire (param-out-*) — convert to a solid gray image at 0–1 brightness
            if (imgEdge.sourceHandle?.startsWith('param-out-')) {
              const paramKey = imgEdge.sourceHandle.slice('param-out-'.length)
              const srcParams = resolvedParams.get(imgEdge.source)
              const fillVal = Math.max(0, Math.min(1, Number(srcParams?.[paramKey] ?? 0)))
              const pct = Math.round(fillVal * 100)
              const floatKey = `${node.id}__float_${inputIdx}`
              const floatHash = shortHash(refPath + String(pct))
              const cachedFloat = this.previewCache.get(floatKey, floatHash)
              if (cachedFloat && await fileExists(cachedFloat)) return cachedFloat
              const floatPath = path.join(TEMP_DIR, `preview_${floatKey}_${floatHash}.png`)
              await spawnMagick([refPath, '-evaluate', 'set', `${pct}%`, '-colorspace', 'Gray', floatPath])
              this.previewCache.set(floatKey, floatHash, floatPath)
              return floatPath
            }
            return imageBuffers.get(`${imgEdge.source}:${imgEdge.sourceHandle ?? 'out-0'}`) ?? refPath
          }
          // Unconnected — solid black placeholder sized to match the workflow input
          const solidKey = `${node.id}__solid_${inputIdx}`
          const solidHash = shortHash(refPath)
          const cachedSolid = this.previewCache.get(solidKey, solidHash)
          if (cachedSolid && await fileExists(cachedSolid)) return cachedSolid
          const solidPath = path.join(TEMP_DIR, `preview_${solidKey}_${solidHash}.png`)
          await spawnMagick([refPath, '-evaluate', 'set', '0%', '-colorspace', 'Gray', solidPath])
          this.previewCache.set(solidKey, solidHash, solidPath)
          return solidPath
        }

        const r = await resolveChannel(0)
        const g = await resolveChannel(1)
        const b = await resolveChannel(2)
        const channelCount = Number(params.channels ?? 3)
        const aImgEdge = graph.edges.find((e) => e.target === node.id && e.targetHandle === 'in-3')
        // Alpha only when the node is in 4-channel mode and the A port is connected
        const hasAlpha = channelCount >= 4 && !!aImgEdge
        const a = hasAlpha ? await resolveChannel(3) : null

        const nodeHash = shortHash(r + g + b + (a ?? '') + JSON.stringify(params))
        const cached = this.previewCache.get(node.id, nodeHash)
        let outputPath: string
        if (cached && await fileExists(cached)) {
          outputPath = cached
        } else {
          outputPath = path.join(TEMP_DIR, `preview_${node.id}_${nodeHash}.png`)
          const mergeArgs = hasAlpha
            ? [r, g, b, a!, '-set', 'colorspace', 'sRGB', '-combine', '-alpha', 'on', outputPath]
            : [r, g, b, '-set', 'colorspace', 'sRGB', '-combine', outputPath]
          await spawnMagick(mergeArgs)
          this.previewCache.set(node.id, nodeHash, outputPath)
        }
        imageBuffers.set(`${node.id}:out-0`, outputPath)
        continue
      }

      // ── Mean Value — reads channel mean, no image output ──────────────────
      if (def.executor === 'mean_value') {
        const inputPath = getImgBuf(node.id, 0)
        try {
          const value = await loadImageMean(inputPath)
          resolvedParams.set(node.id, { ...rawParams, value })
        } catch (err) {
          console.warn(`[executor] loadImageMean failed for node ${node.id}:`, err)
        }
        continue
      }

      // ── Standard single-in / single-out node ──────────────────────────────
      const inputPath = getImgBuf(node.id, 0)
      // Hash the *actual input* (previous node's output) so that any change to the
      // upstream chain automatically invalidates this node's cached result.
      const nodeHash = shortHash(inputPath + JSON.stringify(params))
      const cached = this.previewCache.get(node.id, nodeHash)

      if (cached && await fileExists(cached)) {
        imageBuffers.set(`${node.id}:out-0`, cached)
        continue
      }

      const outputPath = path.join(TEMP_DIR, `preview_${node.id}_${nodeHash}.png`)

      if (params._enabled === false) {
        // Bypassed: pass image through unchanged
        await fs.promises.copyFile(inputPath, outputPath)
      } else if (def.executor === 'format_convert') {
        // Preview output is always a .png temp file, so just re-encode at the requested quality.
        // The format label is ignored here — the file extension (.png) is authoritative for magick.
        const quality = Number(params.quality ?? 90)
        await spawnMagick([inputPath, '-quality', String(quality), outputPath])
      } else {
        const registeredFn = def.executor ? getExecutor(def.executor) : undefined
        const opArgs = registeredFn
          ? registeredFn(def, params)
          : def.command_js
            ? buildCommandArgsFromJs(def, params)
            : buildCommandArgs(def, params)
        if (opArgs.length > 0) {
          await spawnMagick([inputPath, ...opArgs, outputPath])
        } else {
          // executor-type node or empty template — pass through unchanged
          await fs.promises.copyFile(inputPath, outputPath)
        }
      }

      this.previewCache.set(node.id, nodeHash, outputPath)
      imageBuffers.set(`${node.id}:out-0`, outputPath)
    }

    // Resolve the final output from the edge feeding into workflow-output's image input.
    // When the graph is filtered to a preview sub-graph, workflow-output is excluded, so
    // fall back to the last image-producing node in topological order.
    const outputEdge = graph.edges.find(
      (e) => e.target === 'workflow-output' && e.targetHandle === 'in-0'
    )
    let finalPath: string
    if (outputEdge) {
      finalPath = imageBuffers.get(`${outputEdge.source}:${outputEdge.sourceHandle ?? 'out-0'}`) ?? downscaledPath
    } else {
      finalPath = downscaledPath
      for (let i = sorted.length - 1; i >= 0; i--) {
        const p = imageBuffers.get(`${sorted[i].id}:out-0`)
        if (p && p !== downscaledPath) { finalPath = p; break }
      }
    }

    // Collect computed output values for nodes that produce no image outputs
    // (pure value/logic/property nodes, plus mean_value which consumes an image
    // but emits only a scalar). These are displayed live on canvas nodes.
    const propParams: Record<string, Record<string, unknown>> = {}
    for (const node of sorted) {
      const def = registry.get(node.data.definitionId)
      if (!def) continue
      const hasImageOutput = def.outputs.some((p) => p.type === 'image' || p.type === 'mask')
      if (!hasImageOutput) {
        const resolved = resolvedParams.get(node.id)
        if (resolved) propParams[node.id] = resolved
      }
    }

    return { dataUrl: await fileToDataUrl(finalPath), propParams }
  }

  // ── Batch pipeline ──────────────────────────────────────────────────────────
  //
  // Processes every image at full resolution. All node operations are chained into
  // a single magick invocation per image to minimise I/O overhead.

  async executeBatch(
    graph: NodeGraph,
    imagePaths: string[],
    outputDir: string | null,   // null = same directory as each source image
    overwrite: 'skip' | 'overwrite',
    registry: NodeRegistry,
    onProgress: (p: Progress) => void
  ): Promise<{ processed: number; skipped: number; failed: number; errors: string[]; outputFiles: string[] }> {
    const batchT0 = Date.now()
    const outputFiles: string[] = []
    const sorted = topoSort(graph.nodes, graph.edges)

    // Text Output nodes — treated as "output sinks" so upstream nodes (mean_value, etc.)
    // that only feed text outputs are recognised as output contributors.
    // Also includes workflow-output when it is in text mode.
    const outputNode = sorted.find(n => n.id === 'workflow-output')
    const outputNodeTextMode = (outputNode?.data.params as Record<string, unknown> | undefined)?.outputMode === 'text'
    const textOutputNodes = sorted.filter(n =>
      registry.get(n.data.definitionId)?.executor === 'text_output' ||
      (n.id === 'workflow-output' && outputNodeTextMode)
    )
    const hasTextOutputNodes = textOutputNodes.length > 0
    // Whether the workflow produces an image output (edge to workflow-output in image mode).
    const hasImageOutput = !outputNodeTextMode && graph.edges.some(e => e.target === 'workflow-output')

    // Nodes that actually contribute to the final output — backward BFS from
    // workflow-output following ALL edges (image AND param-wire).
    // This ensures channel_split that feeds mean_value → gate (via param-wires)
    // is correctly recognised as a contributor and uses the multi-stream path.
    // Nodes that are purely decorative (no path to workflow-output of any kind)
    // are excluded so they don't force the slow path unnecessarily.
    const outputContributorIds = new Set<string>()
    {
      // Start BFS from workflow-output AND any text_output nodes so upstream nodes
      // (channel_split, mean_value, etc.) that only feed text outputs are correctly
      // recognised as contributors and trigger the right execution path.
      const queue = ['workflow-output', ...textOutputNodes.map(n => n.id)]
      while (queue.length > 0) {
        const id = queue.shift()!
        if (outputContributorIds.has(id)) continue
        outputContributorIds.add(id)
        for (const e of graph.edges) {
          if (e.target === id) queue.push(e.source)
        }
      }
    }

    // prop_ nodes depend on per-image file metadata (dimensions, name, EXIF, etc.)
    // mean_value depends on per-image pixel data but NOT on loadImageMeta.
    // Either kind requires per-image plan evaluation (no shared opArgs).
    // Only nodes that contribute to output are relevant — disconnected nodes must not
    // influence the shared-plan decision or trigger unnecessary metadata calls.
    const hasImageMetaNodes = sorted.some((n) => {
      if (!outputContributorIds.has(n.id)) return false
      const def = registry.get(n.data.definitionId)
      return def?.needs_image_meta === true
    })
    // prop_name / prop_path only need path.basename — no ImageMagick identify call required.
    // All other needs_image_meta nodes (dimensions, bitdepth, EXIF, …) need the full identify.
    const NAME_PATH_ONLY_EXECUTORS = new Set(['prop_name', 'prop_path'])
    const hasHeavyMetaNodes = sorted.some((n) => {
      if (!outputContributorIds.has(n.id)) return false
      const def = registry.get(n.data.definitionId)
      return def?.needs_image_meta === true && !NAME_PATH_ONLY_EXECUTORS.has(def.executor ?? '')
    })
    const hasPropNodes = hasImageMetaNodes || sorted.some((n) => {
      if (!outputContributorIds.has(n.id)) return false
      const def = registry.get(n.data.definitionId)
      return def?.executor === 'mean_value'
    })

    // Executors that require executeMultiStream (cannot be handled by the fast-path
    // buildOpArgsForImage): channel_split/merge produce multiple image buffers;
    // mean_value reads pixel data per-image and feeds param-wires (gate conditions etc.)
    // — buildOpArgsForImage skips it, leaving downstream gate conditions unset.
    const MULTI_STREAM_EXECUTORS = new Set(['channel_split', 'channel_merge', 'mean_value'])
    const hasMultiStreamNodes = sorted.some((n) => {
      if (!outputContributorIds.has(n.id)) return false
      const def = registry.get(n.data.definitionId)
      return def?.executor && MULTI_STREAM_EXECUTORS.has(def.executor)
    })

    const FORMAT_EXT: Record<string, string> = {
      PNG: '.png', JPEG: '.jpg', WEBP: '.webp', TIFF: '.tif', AVIF: '.avif', BMP: '.bmp', TGA: '.tga',
    }

    interface BatchPlan {
      opArgs: string[]
      outputFormat: string | null   // e.g. 'PNG' — non-null only when format_convert is active
      textLines: string[]           // values collected from text_output nodes (condition=true)
    }

    // Lines collected across all images; written to disk after the batch completes.
    const collectedTextLines: Array<{ index: number; value: string }> = []

    // Returns null when a Gate node suppresses the image (don't write output).
    async function buildOpArgsForImage(imagePath: string): Promise<BatchPlan | null> {
      let meta: ImageMeta | undefined
      if (hasHeavyMetaNodes) {
        try { meta = await loadImageMeta(imagePath) } catch (err) { console.warn(`[executor] loadImageMeta failed for ${imagePath} (non-fatal, prop nodes use defaults):`, err) }
      } else if (hasImageMetaNodes && imagePath !== '') {
        // prop_name / prop_path only — no ImageMagick spawn needed.
        meta = buildEmptyImageMeta(imagePath)
      }
      const resolvedParams = new Map<string, Record<string, unknown>>()
      const opArgs: string[] = []
      const textLines: string[] = []
      let outputFormat: string | null = null
      for (const node of sorted) {
        if (!outputContributorIds.has(node.id)) continue
        if (node.data.definitionId === 'process_as_set') continue
        const def = registry.get(node.data.definitionId)
        if (!def) {
          // workflow-output in text mode acts as a text output sink
          if (node.id === 'workflow-output' && outputNodeTextMode) {
            const rawParams: Record<string, unknown> = { ...(node.data.params ?? {}) }
            for (const edge of graph.edges) {
              if (edge.target !== node.id) continue
              const th = edge.targetHandle ?? '', sh = edge.sourceHandle ?? ''
              if (sh.startsWith('param-out-') && th.startsWith('txo-')) {
                const src = resolvedParams.get(edge.source)
                if (src) rawParams[`_txo_${th.slice('txo-'.length)}`] = src[sh.slice('param-out-'.length)]
              }
            }
            if (Boolean(rawParams._txo_condition ?? true)) {
              const portIds = (rawParams.portIds ?? []) as string[]
              const sep = getSeparator(String(rawParams.separatorType ?? ''), String(rawParams.customSeparator ?? ''))
              const line = portIds
                .map(pid => rawParams[`_txo_${pid.slice('txo-'.length)}`])
                .filter(v => v !== undefined && v !== null && v !== '')
                .map(v => String(v))
                .join(sep)
              if (line) textLines.push(line)
            }
          }
          continue
        }
        const rawParams: Record<string, unknown> = { ...(node.data.params ?? {}) }
        for (const edge of graph.edges) {
          if (edge.target !== node.id) continue
          const th = edge.targetHandle ?? ''
          const sh = edge.sourceHandle ?? ''
          if (sh.startsWith('param-out-')) {
            const sourceParam = sh.slice('param-out-'.length)
            const src = resolvedParams.get(edge.source)
            if (src && sourceParam in src) {
              if (th.startsWith('param-in-')) {
                rawParams[th.slice('param-in-'.length)] = src[sourceParam]
              } else if (th.startsWith('txo-')) {
                rawParams[`_txo_${th.slice('txo-'.length)}`] = src[sourceParam]
              }
            }
          }
        }
        const isImageNode = def.inputs.some((p) => p.type === 'image' || p.type === 'mask') ||
                            def.outputs.some((p) => p.type === 'image' || p.type === 'mask')
        const computeInput = (!isImageNode && def.compute_js)
          ? { ...rawParams, __compute_js__: def.compute_js }
          : rawParams
        const params = computeNodeParams(isImageNode ? undefined : def.executor, computeInput, meta)
        resolvedParams.set(node.id, params)
        if (!isImageNode) {
          // Collect text_output values — written to disk after the full batch completes.
          if (def.executor === 'text_output' && params._enabled !== false && Boolean(params._txo_condition ?? params.condition)) {
            const portIds = (params.portIds ?? []) as string[]
            const sep = getSeparator(String(params.separatorType ?? ''), String(params.customSeparator ?? ''))
            const values = portIds
              .map(pid => params[`_txo_${pid.slice('txo-'.length)}`])
              .filter(v => v !== undefined && v !== null && v !== '')
              .map(v => String(v))
            const line = values.join(sep)
            if (line) textLines.push(line)
          }
          continue
        }
        // Gate node: when active and condition is false, suppress this image entirely
        if (def.executor === 'gate' && params._enabled !== false && !params.condition) return null
        // Mean Value — analysis-only, no image output, no opArgs contribution
        if (def.executor === 'mean_value') continue
        if (params._enabled !== false) {
          if (def.executor === 'format_convert') {
            // Record the target format so processOne can set the output extension.
            // Add quality arg unconditionally — ImageMagick ignores it for lossless formats.
            outputFormat = String(params.format ?? 'PNG').toUpperCase()
            opArgs.push('-quality', String(params.quality ?? 90))
          } else {
            const registeredFn = def.executor ? getExecutor(def.executor) : undefined
            opArgs.push(...(registeredFn
              ? registeredFn(def, params)
              : def.command_js
                ? buildCommandArgsFromJs(def, params)
                : buildCommandArgs(def, params)))
          }
        }
      }
      return { opArgs, outputFormat, textLines }
    }

    // Fast path: no Properties nodes — evaluate once, reuse for all images.
    // undefined = not pre-computed (will be built per-image); null = gate suppressed for all images.
    const sharedPlan: BatchPlan | null | undefined =
      (hasPropNodes || hasMultiStreamNodes) ? undefined : await buildOpArgsForImage('')

    // Multi-stream execution for a single image with two speed optimisations:
    //   1. Command fusion — consecutive standard nodes are chained into a single magick
    //      invocation instead of one process per node (lazy-buffer approach).
    //   2. Channel split — all 4 channels are extracted in one magick call via -write.
    // Returns the final output path and extension, or null if the image should be suppressed.
    const executeMultiStream = async (inputPath: string, imageIndex: number, extraSeeds?: Map<string, string>): Promise<{ resultPath: string; outputExt: string } | null> => {
      const tmpId = shortHash(inputPath + String(imageIndex))
      let _seq = 0
      // MIFF (ImageMagick native) is uncompressed and skips PNG encode/decode overhead
      // for every intermediate step. Only the final output uses the real output extension.
      const newTmp = (ext = '.miff') => path.join(TEMP_DIR, `batch_ms_${tmpId}_${_seq++}${ext}`)

      // Each buffer slot is either a concrete file path (string) or a lazy chain that
      // accumulates magick args to be applied to a base file on demand.
      type Lazy = { base: string; args: string[] }
      const buffers = new Map<string, string | Lazy>()
      buffers.set('workflow-input:out-0', inputPath)
      if (extraSeeds) for (const [k, v] of extraSeeds) buffers.set(k, v)

      // Materialise a buffer slot: flush its lazy args into a temp file if needed.
      const mat = async (key: string): Promise<string> => {
        const v = buffers.get(key)
        if (v === undefined) return inputPath
        if (typeof v === 'string') return v
        if (v.args.length === 0) { buffers.set(key, v.base); return v.base }
        const out = newTmp()
        await spawnMagick([v.base, ...v.args, out])
        buffers.set(key, out)   // upgrade to concrete path so re-materialisation is free
        return out
      }

      // Return the materialised image path for node's Nth image input.
      const getImg = async (nodeId: string, inputIdx: number): Promise<string> => {
        const edge = graph.edges.find(e => e.target === nodeId && e.targetHandle === `in-${inputIdx}`)
        if (!edge) return inputPath
        return mat(`${edge.source}:${edge.sourceHandle ?? 'out-0'}`)
      }

      // Count image-edge consumers per output key so we know when lazy chaining is safe.
      // (A lazy chain can only be extended when we are the sole consumer of its source.)
      const imgConsumers = new Map<string, number>()
      for (const e of graph.edges) {
        if ((e.targetHandle ?? '').startsWith('in-')) {
          const k = `${e.source}:${e.sourceHandle ?? 'out-0'}`
          imgConsumers.set(k, (imgConsumers.get(k) ?? 0) + 1)
        }
      }

      // Pre-detect channel_split nodes whose outputs are consumed ONLY by mean_value.
      // For these we skip writing the 4 channel PNG files and instead compute the
      // per-channel mean directly from the source image (no temp-file I/O at all).
      // Map: "splitNodeId:out-N" → { srcKey of split's image input, channelIdx }
      const channelMeanSources = new Map<string, { srcKey: string; channelIdx: number }>()
      const analysisOnlySplitNodes = new Set<string>()
      for (const n of sorted) {
        const d = registry.get(n.data.definitionId)
        if (d?.executor !== 'channel_split') continue
        const outEdges = graph.edges.filter(e => e.source === n.id && (e.sourceHandle ?? '').startsWith('out-'))
        if (outEdges.length === 0) continue
        const allMeanValue = outEdges.every(e => {
          const consumer = sorted.find(c => c.id === e.target)
          return registry.get(consumer?.data.definitionId ?? '')?.executor === 'mean_value'
        })
        if (!allMeanValue) continue
        analysisOnlySplitNodes.add(n.id)
        const splitInEdge = graph.edges.find(e => e.target === n.id && e.targetHandle === 'in-0')
        const splitSrcKey = splitInEdge
          ? `${splitInEdge.source}:${splitInEdge.sourceHandle ?? 'out-0'}`
          : 'workflow-input:out-0'
        for (const e of outEdges) {
          const chIdx = parseInt((e.sourceHandle ?? 'out-0').replace('out-', ''))
          channelMeanSources.set(`${n.id}:${e.sourceHandle ?? 'out-0'}`, { srcKey: splitSrcKey, channelIdx: chIdx })
        }
      }

      let meta: ImageMeta | undefined
      if (hasHeavyMetaNodes) {
        try { meta = await loadImageMeta(inputPath) } catch (err) { console.warn(`[executor] loadImageMeta failed for ${inputPath} (non-fatal):`, err) }
      } else if (hasImageMetaNodes) {
        // prop_name / prop_path only — no ImageMagick spawn needed.
        meta = buildEmptyImageMeta(inputPath)
      }

      // Pre-compute all channel means for analysis-only split nodes in a single magick
      // spawn per source image, instead of one spawn per channel per mean_value node.
      // Key: srcKey (e.g. "workflow-input:out-0"), Value: array indexed by channelIdx.
      const precomputedMeans = new Map<string, number[]>()
      if (channelMeanSources.size > 0) {
        const bySourceKey = new Map<string, number[]>()
        for (const { srcKey, channelIdx } of channelMeanSources.values()) {
          if (!bySourceKey.has(srcKey)) bySourceKey.set(srcKey, [])
          const arr = bySourceKey.get(srcKey)!
          if (!arr.includes(channelIdx)) arr.push(channelIdx)
        }
        for (const [srcKey, indices] of bySourceKey) {
          try {
            indices.sort((a, b) => a - b)
            const resolvedSrc = await mat(srcKey)
            const means = await loadMultipleChannelMeans(resolvedSrc, indices)
            const byIdx: number[] = []
            for (let i = 0; i < indices.length; i++) byIdx[indices[i]] = means[i]
            precomputedMeans.set(srcKey, byIdx)
          } catch (err) {
            console.warn(`[executor] loadMultipleChannelMeans failed for ${srcKey}:`, err)
          }
        }
      }

      // Track the effective output extension (updated by format_convert nodes).
      let outputExt = path.extname(inputPath)

      const resolvedParams = new Map<string, Record<string, unknown>>()

      for (const node of sorted) {
        if (!outputContributorIds.has(node.id)) continue
        // process_as_set is a source node — buffers are pre-seeded externally; skip processing.
        if (node.data.definitionId === 'process_as_set') continue
        const def = registry.get(node.data.definitionId)
        if (!def) {
          // workflow-output in text mode acts as a text output sink
          if (node.id === 'workflow-output' && outputNodeTextMode) {
            const rawParams: Record<string, unknown> = { ...(node.data.params ?? {}) }
            for (const edge of graph.edges) {
              if (edge.target !== node.id) continue
              const th = edge.targetHandle ?? '', sh = edge.sourceHandle ?? ''
              if (sh.startsWith('param-out-') && th.startsWith('txo-')) {
                const src = resolvedParams.get(edge.source)
                if (src) rawParams[`_txo_${th.slice('txo-'.length)}`] = src[sh.slice('param-out-'.length)]
              }
            }
            if (Boolean(rawParams._txo_condition ?? true)) {
              const portIds = (rawParams.portIds ?? []) as string[]
              const sep = getSeparator(String(rawParams.separatorType ?? ''), String(rawParams.customSeparator ?? ''))
              const line = portIds
                .map(pid => rawParams[`_txo_${pid.slice('txo-'.length)}`])
                .filter(v => v !== undefined && v !== null && v !== '')
                .map(v => String(v))
                .join(sep)
              if (line) collectedTextLines.push({ index: imageIndex, value: line })
            }
          }
          continue
        }

        const rawParams: Record<string, unknown> = { ...(node.data.params ?? {}) }
        for (const edge of graph.edges) {
          if (edge.target !== node.id) continue
          const th = edge.targetHandle ?? ''
          const sh = edge.sourceHandle ?? ''
          if (sh.startsWith('param-out-')) {
            const sp = sh.slice('param-out-'.length)
            const src = resolvedParams.get(edge.source)
            if (src && sp in src) {
              if (th.startsWith('param-in-')) {
                rawParams[th.slice('param-in-'.length)] = src[sp]
              } else if (th.startsWith('txo-')) {
                rawParams[`_txo_${th.slice('txo-'.length)}`] = src[sp]
              }
            }
          }
        }

        const isImageNode = def.inputs.some((p) => p.type === 'image' || p.type === 'mask') ||
                            def.outputs.some((p) => p.type === 'image' || p.type === 'mask')
        const computeInput = (!isImageNode && def.compute_js)
          ? { ...rawParams, __compute_js__: def.compute_js }
          : rawParams
        const params = computeNodeParams(isImageNode ? undefined : def.executor, computeInput, meta)
        resolvedParams.set(node.id, params)
        if (!isImageNode) {
          // Collect text_output values — written to disk after the full batch completes.
          if (def.executor === 'text_output' && params._enabled !== false && Boolean(params._txo_condition ?? params.condition)) {
            const portIds = (params.portIds ?? []) as string[]
            const sep = getSeparator(String(params.separatorType ?? ''), String(params.customSeparator ?? ''))
            const values = portIds
              .map(pid => params[`_txo_${pid.slice('txo-'.length)}`])
              .filter(v => v !== undefined && v !== null && v !== '')
              .map(v => String(v))
            const line = values.join(sep)
            if (line) collectedTextLines.push({ index: imageIndex, value: line })
          }
          continue
        }

        if (def.executor === 'gate' && params._enabled !== false && !params.condition) return null

        if (def.executor === 'channel_split') {
          if (analysisOnlySplitNodes.has(node.id)) {
            // All consumers are mean_value — skip writing channel files entirely.
            // channelMeanSources already maps each out-N to the source image key;
            // mean_value will call loadImageChannelMean directly.
          } else {
            const CHAN = ['Red', 'Green', 'Blue', 'Alpha']
            const usedIdxs = [0, 1, 2, 3].filter(i => (imgConsumers.get(`${node.id}:out-${i}`) ?? 0) > 0)

            if (usedIdxs.length === 1) {
              // Single channel consumed — defer as a lazy chain so the downstream op
              // (e.g. negate) can fuse onto it, avoiding an intermediate temp file.
              const src = await getImg(node.id, 0)
              const i = usedIdxs[0]
              buffers.set(`${node.id}:out-${i}`, { base: src, args: ['-channel', CHAN[i], '-separate'] })
            } else if (usedIdxs.length > 1) {
              // Multiple channels — extract only those actually used in one spawn.
              const src = await getImg(node.id, 0)
              const outs = usedIdxs.map(() => newTmp())
              const lastK = usedIdxs.length - 1
              const args: string[] = [src]
              for (let k = 0; k < lastK; k++) {
                args.push('(', '+clone', '-channel', CHAN[usedIdxs[k]], '-separate', '-write', outs[k], '+delete', ')')
              }
              args.push('-channel', CHAN[usedIdxs[lastK]], '-separate', outs[lastK])
              await spawnMagick(args)
              for (let k = 0; k < usedIdxs.length; k++) {
                buffers.set(`${node.id}:out-${usedIdxs[k]}`, outs[k])
              }
            }
          }

        } else if (def.executor === 'channel_merge') {
          const refPath = await mat('workflow-input:out-0')

          // Returns either a concrete path or inline magick args for a constant fill.
          // Constant channels (param-wired or unconnected) skip the intermediate temp-file
          // spawn and are inlined as parenthesised sub-expressions in the combine command.
          type ChanSrc = { kind: 'path'; path: string } | { kind: 'inline'; args: string[] }

          const resolveChannel = async (inputIdx: number): Promise<ChanSrc> => {
            const imgEdge = graph.edges.find((e) => e.target === node.id && e.targetHandle === `in-${inputIdx}`)
            if (imgEdge) {
              if (imgEdge.sourceHandle?.startsWith('param-out-')) {
                const paramKey = imgEdge.sourceHandle.slice('param-out-'.length)
                const srcParams = resolvedParams.get(imgEdge.source)
                const fillVal = Math.max(0, Math.min(1, Number(srcParams?.[paramKey] ?? 0)))
                const pct = Math.round(fillVal * 100)
                return { kind: 'inline', args: [refPath, '-evaluate', 'set', `${pct}%`, '-colorspace', 'Gray'] }
              }
              return { kind: 'path', path: await mat(`${imgEdge.source}:${imgEdge.sourceHandle ?? 'out-0'}`) }
            }
            return { kind: 'inline', args: [refPath, '-evaluate', 'set', '0%', '-colorspace', 'Gray'] }
          }

          const expand = (ch: ChanSrc): string[] =>
            ch.kind === 'path' ? [ch.path] : ['(', ...ch.args, ')']

          const channelCount = Number(params.channels ?? 3)
          const aImgEdge = graph.edges.find((e) => e.target === node.id && e.targetHandle === 'in-3')
          const hasAlpha = channelCount >= 4 && !!aImgEdge
          // Resolve all channels concurrently — their materialisations are independent.
          const idxs = hasAlpha ? [0, 1, 2, 3] : [0, 1, 2]
          const resolved = await Promise.all(idxs.map(i => resolveChannel(i)))
          const [r, g, b] = resolved
          const a = hasAlpha ? resolved[3] : null
          const out = newTmp()
          await spawnMagick([
            ...expand(r), ...expand(g), ...expand(b),
            ...(hasAlpha && a ? expand(a) : []),
            '-set', 'colorspace', 'sRGB', '-combine',
            ...(hasAlpha ? ['-alpha', 'on'] : []),
            out,
          ])
          buffers.set(`${node.id}:out-0`, out)

        } else if (def.executor === 'mean_value') {
          try {
            const imgInEdge = graph.edges.find(e => e.target === node.id && e.targetHandle === 'in-0')
            const srcSlot = imgInEdge ? `${imgInEdge.source}:${imgInEdge.sourceHandle ?? 'out-0'}` : undefined
            const chanInfo = srcSlot ? channelMeanSources.get(srcSlot) : undefined
            const value = chanInfo
              ? (precomputedMeans.get(chanInfo.srcKey)?.[chanInfo.channelIdx]
                  ?? await loadImageChannelMean(await mat(chanInfo.srcKey), chanInfo.channelIdx))
              : await loadImageMean(await getImg(node.id, 0))
            resolvedParams.set(node.id, { ...rawParams, value })
          } catch (err) {
            console.warn(`[executor] loadImageMean failed for node ${node.id}:`, err)
          }

        } else if (def.executor === 'format_convert') {
          // Format convert must materialise immediately (changes file type).
          const src = await getImg(node.id, 0)
          const fmt = String(params.format ?? 'PNG').toUpperCase()
          const fmtExts: Record<string, string> = {
            PNG: '.png', JPEG: '.jpg', WEBP: '.webp', TIFF: '.tif', AVIF: '.avif', BMP: '.bmp', TGA: '.tga',
          }
          outputExt = fmtExts[fmt] ?? outputExt
          const out = newTmp(outputExt)
          await spawnMagick([src, '-quality', String(params.quality ?? 90), `${fmt}:${out}`])
          buffers.set(`${node.id}:out-0`, out)

        } else if (params._enabled !== false) {
          // Standard image op — fuse into a lazy chain when safe to do so.
          const imgInEdge = graph.edges.find(e => e.target === node.id && e.targetHandle === 'in-0')
          const srcKey = imgInEdge
            ? `${imgInEdge.source}:${imgInEdge.sourceHandle ?? 'out-0'}`
            : 'workflow-input:out-0'
          const outKey = `${node.id}:out-0`
          const registeredFn = def.executor ? getExecutor(def.executor) : undefined
          const opArgs = registeredFn
            ? registeredFn(def, params)
            : def.command_js
              ? buildCommandArgsFromJs(def, params)
              : buildCommandArgs(def, params)

          if (opArgs.length === 0) {
            // No-op — inherit source slot unchanged.
            buffers.set(outKey, buffers.get(srcKey) ?? inputPath)
          } else if ((imgConsumers.get(srcKey) ?? 0) <= 1) {
            // Sole consumer of source — extend (or start) the lazy chain.
            const srcVal = buffers.get(srcKey) ?? inputPath
            if (typeof srcVal === 'string') {
              buffers.set(outKey, { base: srcVal, args: opArgs })
            } else {
              buffers.set(outKey, { base: srcVal.base, args: [...srcVal.args, ...opArgs] })
            }
          } else {
            // Multiple consumers — materialise first so we don't double-apply ops.
            const src = await mat(srcKey)
            buffers.set(outKey, { base: src, args: opArgs })
          }

        } else {
          // Bypassed — pass source slot through unchanged (preserves any lazy chain).
          const imgInEdge = graph.edges.find(e => e.target === node.id && e.targetHandle === 'in-0')
          const srcKey = imgInEdge
            ? `${imgInEdge.source}:${imgInEdge.sourceHandle ?? 'out-0'}`
            : 'workflow-input:out-0'
          buffers.set(`${node.id}:out-0`, buffers.get(srcKey) ?? inputPath)
        }
      }

      const outputEdge = graph.edges.find(
        (e) => e.target === 'workflow-output' && e.targetHandle === 'in-0'
      )
      if (!outputEdge) return { resultPath: inputPath, outputExt: path.extname(inputPath) }

      const finalKey = `${outputEdge.source}:${outputEdge.sourceHandle ?? 'out-0'}`
      const finalVal = buffers.get(finalKey)
      if (!finalVal) return { resultPath: inputPath, outputExt: path.extname(inputPath) }
      if (typeof finalVal === 'string') {
        // If the concrete path is an intermediate format (e.g. .miff) but the output
        // needs a different format (e.g. .PNG), do a single conversion spawn.
        if (path.extname(finalVal).toLowerCase() !== outputExt.toLowerCase()) {
          const finalOut = newTmp(outputExt)
          await spawnMagick([finalVal, finalOut])
          return { resultPath: finalOut, outputExt }
        }
        return { resultPath: finalVal, outputExt }
      }

      // Final materialisation: use the correct output extension (not always .png).
      const finalOut = newTmp(outputExt)
      if (finalVal.args.length > 0) await spawnMagick([finalVal.base, ...finalVal.args, finalOut])
      else await fs.promises.copyFile(finalVal.base, finalOut)
      return { resultPath: finalOut, outputExt }
    }

    // ── Preview substitution — swap full-size paths for cached thumbnails ────────
    if (outputNodeTextMode && (outputNode?.data.params as Record<string, unknown> | undefined)?.usePreviewForProcessing) {
      const inputNode = graph.nodes.find(n => n.id === 'workflow-input')
      const inputParams = (inputNode?.data as Record<string, unknown>)?.params as Record<string, unknown> | undefined
      const thumbSizePx = Number(inputParams?.thumbnailSize ?? 128)
      const thumbPath = (p: string) => path.join(TEMP_DIR, `thumb_${shortHash(p)}_${thumbSizePx}.webp`)
      imagePaths = await Promise.all(imagePaths.map(async (p) => {
        const thumb = thumbPath(p)
        return await fs.promises.access(thumb).then(() => thumb).catch(() => p)
      }))
    }

    // ── Set batch mode ─────────────────────────────────────────────────────────
    // When a setInputNode is present, group images by naming convention and
    // execute one run per set instead of one run per image.
    const setInputNode = sorted.find(n => n.data.definitionId === 'process_as_set')
    if (setInputNode) {
      const setPrefix   = String(setInputNode.data.params?.prefix ?? '')
      const setSuffixes = Array.isArray(setInputNode.data.params?.suffixes)
        ? (setInputNode.data.params!.suffixes as string[]).filter(Boolean)
        : []
      const outParams       = outputNode?.data.params as Record<string, unknown> | undefined
      const setOutputPrefix = String(outParams?.setOutputPrefix ?? '')
      const setOutputSuffix = String(outParams?.setOutputSuffix ?? '')

      const setGroups  = groupBySetPattern(imagePaths, setPrefix, setSuffixes)
      const setEntries = [...setGroups.entries()]
      const totalSets  = setEntries.length

      if (timings.enabled) {
        timings.startBatch(totalSets)
        timings.recordSetup(Date.now() - batchT0)
      }

      this._batchCancelled = false
      const self = this
      let setQueueIdx = 0
      let setCompleted = 0
      let setFailures  = 0
      let setSkipped   = 0
      const setErrors: string[] = []
      let _setStartupRecorded = false
      const activeImages = new Set<string>()

      onProgress({ completed: 0, total: totalSets, currentFile: '', active: [] })

      const setConcurrency = Math.min(Math.max(1, os.cpus().length), Math.max(1, totalSets))

      async function processOneSet(): Promise<void> {
        while (setQueueIdx < setEntries.length) {
          if (self._batchCancelled) return
          const setIndex = setQueueIdx
          const [middleName, suffixMap] = setEntries[setQueueIdx++]
          activeImages.add(middleName)
          onProgress({ completed: setCompleted, total: totalSets, currentFile: middleName, active: [...activeImages] })
          const imgT0 = timings.enabled ? Date.now() : 0
          if (timings.enabled && !_setStartupRecorded) {
            _setStartupRecorded = true
            timings.recordStartup(Date.now() - batchT0)
          }

          // First available path serves as the inputPath fallback for unconnected ports.
          const firstPath = setSuffixes.map(s => suffixMap[s]).find(Boolean) ?? ''

          // Pre-seed one buffer per suffix port; missing suffixes are left unseeded
          // so their downstream nodes receive the firstPath fallback.
          const seeds = new Map<string, string>()
          const setNodeId = setInputNode!.id
          for (let i = 0; i < setSuffixes.length; i++) {
            const p = suffixMap[setSuffixes[i]]
            if (p) seeds.set(`${setNodeId}:out-${i}`, p)
          }

          try {
            const targetDir = outputDir ?? (firstPath ? path.dirname(firstPath) : process.cwd())
            const checkT0 = timings.enabled ? Date.now() : 0
            await fs.promises.mkdir(targetDir, { recursive: true })
            let fileCheckMs = timings.enabled ? Date.now() - checkT0 : 0

            if (hasImageOutput) {
              const msT0 = timings.enabled ? Date.now() : 0
              const msResult = await executeMultiStream(firstPath, setIndex, seeds)
              const msElapsed = timings.enabled ? Date.now() - msT0 : 0
              if (msResult === null) {
                setSkipped++
                activeImages.delete(middleName)
                onProgress({ completed: ++setCompleted, total: totalSets, currentFile: middleName, active: [...activeImages] })
                continue
              }
              const { resultPath, outputExt: msExt } = msResult
              const outBase = setOutputPrefix + middleName + setOutputSuffix
              const outExt  = msExt || path.extname(firstPath)
              const outPath = path.join(targetDir, outBase + outExt)

              if (overwrite === 'skip') {
                const accessT0 = timings.enabled ? Date.now() : 0
                const exists = await fs.promises.access(outPath).then(() => true).catch(() => false)
                if (timings.enabled) fileCheckMs += Date.now() - accessT0
                if (exists) {
                  setSkipped++
                  activeImages.delete(middleName)
                  onProgress({ completed: ++setCompleted, total: totalSets, currentFile: middleName, active: [...activeImages] })
                  continue
                }
              }
              const copyT0 = timings.enabled ? Date.now() : 0
              await fs.promises.copyFile(resultPath, outPath)
              if (timings.enabled) {
                const imgEntry = timings.beginImage(firstPath || middleName)
                imgEntry.fileCheck(fileCheckMs)
                imgEntry.magick(msElapsed)
                imgEntry.copy(Date.now() - copyT0)
                imgEntry.done(Date.now() - imgT0)
              }
              outputFiles.push(outPath)
            }
          } catch (err) {
            setFailures++
            const msg = err instanceof Error ? err.message : String(err)
            setErrors.push(`${middleName}: ${msg}`)
            console.error(`[executor] Failed to process set "${middleName}":`, err)
          }
          activeImages.delete(middleName)
          onProgress({ completed: ++setCompleted, total: totalSets, currentFile: middleName, active: [...activeImages] })
        }
      }

      const threadsPerSetProcess = Math.max(1, Math.floor(os.cpus().length / setConcurrency))
      const prevThreadLimitSet = process.env.MAGICK_THREAD_LIMIT
      process.env.MAGICK_THREAD_LIMIT = String(threadsPerSetProcess)
      try {
        await Promise.all(Array.from({ length: setConcurrency }, processOneSet))
      } finally {
        if (prevThreadLimitSet !== undefined) process.env.MAGICK_THREAD_LIMIT = prevThreadLimitSet
        else delete process.env.MAGICK_THREAD_LIMIT
      }
      if (timings.enabled) {
        const resolvedOutputDir = outputDir ?? (outputFiles.length > 0 ? path.dirname(outputFiles[0]) : null)
        timings.endBatch(resolvedOutputDir)
      }
      return {
        processed: setCompleted - setFailures - setSkipped,
        skipped: setSkipped,
        failed: setFailures,
        errors: setErrors,
        outputFiles,
      }
    }

    // Run all images concurrently, capped at 128 for very large batches.
    // With MAGICK_THREAD_LIMIT=1, each process uses exactly one thread so small
    // oversubscription (e.g. 26 images on 24 cores) costs less than a full extra round.
    // JS single-threaded event loop guarantees queueIdx++ is race-free.
    const concurrency = Math.min(128, imagePaths.length)
    this._batchCancelled = false
    const self = this
    let queueIdx = 0
    let completed = 0
    let failures = 0
    let skipped = 0
    const errors: string[] = []

    if (timings.enabled) {
      timings.startBatch(imagePaths.length)
      timings.recordSetup(Date.now() - batchT0)
    }

    const activeImages = new Set<string>()
    onProgress({ completed: 0, total: imagePaths.length, currentFile: '', active: [] })

    // Resolve rename node params once (shared across all images — index varies per image)
    const renameNode = sorted.find((n) => registry.get(n.data.definitionId)?.executor === 'rename')
    const renameParams = renameNode ? (renameNode.data.params as RenameParams) : undefined

    let _startupRecorded = false

    async function processOne(): Promise<void> {
      while (queueIdx < imagePaths.length) {
        if (self._batchCancelled) return
        const imageIndex = queueIdx          // 0-based index for rename numbering
        const inputPath  = imagePaths[queueIdx++]
        const fileName   = path.basename(inputPath)
        activeImages.add(fileName)
        onProgress({ completed, total: imagePaths.length, currentFile: fileName, active: [...activeImages] })
        const imgT0 = timings.enabled ? Date.now() : 0
        if (timings.enabled && !_startupRecorded) {
          _startupRecorded = true
          timings.recordStartup(Date.now() - batchT0)
        }
        const magickBucket = timings.enabled ? { ms: 0, add(n: number) { this.ms += n } } : null
        // Apply rename transform to determine the output filename stem
        const renamedFileName = renameParams ? computeNewName(fileName, renameParams, imageIndex) : fileName
        try {
          const targetDir = outputDir ?? path.dirname(inputPath)
          const checkT0 = timings.enabled ? Date.now() : 0
          await fs.promises.mkdir(targetDir, { recursive: true })
          let fileCheckMs = timings.enabled ? Date.now() - checkT0 : 0

          if (hasMultiStreamNodes) {
            // Multi-stream path — runs concurrently; unique tmpId per image prevents collisions
            const msT0 = timings.enabled ? Date.now() : 0
            const msResult = await executeMultiStream(inputPath, imageIndex)
            const msElapsed = timings.enabled ? Date.now() - msT0 : 0
            if (msResult === null) {
              skipped++
              activeImages.delete(fileName)
              onProgress({ completed: ++completed, total: imagePaths.length, currentFile: fileName, active: [...activeImages] })
              continue
            }
            if (hasImageOutput) {
              const { resultPath, outputExt: msExt } = msResult
              const outExt  = msExt || path.extname(renamedFileName)
              const outBase = path.basename(renamedFileName, path.extname(renamedFileName))
              const outPath = path.join(targetDir, outBase + outExt)

              if (overwrite === 'skip') {
                const accessT0 = timings.enabled ? Date.now() : 0
                const exists = await fs.promises.access(outPath).then(() => true).catch(() => false)
                if (timings.enabled) fileCheckMs += Date.now() - accessT0
                if (exists) {
                  skipped++
                  activeImages.delete(fileName)
                  onProgress({ completed: ++completed, total: imagePaths.length, currentFile: fileName, active: [...activeImages] })
                  continue
                }
              }
              const copyT0 = timings.enabled ? Date.now() : 0
              await fs.promises.copyFile(resultPath, outPath)
              if (timings.enabled) {
                const imgEntry = timings.beginImage(inputPath)
                imgEntry.fileCheck(fileCheckMs)
                imgEntry.magick(msElapsed)
                imgEntry.copy(Date.now() - copyT0)
                imgEntry.done(Date.now() - imgT0)
              }
              outputFiles.push(outPath)
            }
          } else {
            // Single-command fast path
            const plan = sharedPlan !== undefined ? sharedPlan : await buildOpArgsForImage(inputPath)
            if (plan === null) {
              skipped++
              activeImages.delete(fileName)
              onProgress({ completed: ++completed, total: imagePaths.length, currentFile: fileName, active: [...activeImages] })
              continue
            }
            // Collect text output values from this image's plan.
            for (const line of plan.textLines) {
              collectedTextLines.push({ index: imageIndex, value: line })
            }
            if (hasImageOutput) {
              const { opArgs, outputFormat } = plan
              const outExt  = outputFormat ? (FORMAT_EXT[outputFormat] ?? path.extname(renamedFileName)) : path.extname(renamedFileName)
              const outBase = path.basename(renamedFileName, path.extname(renamedFileName))
              const outPath = path.join(targetDir, outBase + outExt)

              if (overwrite === 'skip') {
                const accessT0 = timings.enabled ? Date.now() : 0
                const exists = await fs.promises.access(outPath).then(() => true).catch(() => false)
                if (timings.enabled) fileCheckMs += Date.now() - accessT0
                if (exists) {
                  skipped++
                  activeImages.delete(fileName)
                  onProgress({ completed: ++completed, total: imagePaths.length, currentFile: fileName, active: [...activeImages] })
                  continue
                }
              }

              if (opArgs.length > 0 || outputFormat) {
                const fmtOut = outputFormat ? `${outputFormat}:${outPath}` : outPath
                await spawnMagick([inputPath, ...opArgs, fmtOut], magickBucket ?? undefined)
                if (timings.enabled && magickBucket) {
                  const imgEntry = timings.beginImage(inputPath)
                  imgEntry.fileCheck(fileCheckMs)
                  imgEntry.magick(magickBucket.ms)
                  imgEntry.copy(0)
                  imgEntry.done(Date.now() - imgT0)
                }
              } else {
                const copyT0 = timings.enabled ? Date.now() : 0
                await fs.promises.copyFile(inputPath, outPath)
                if (timings.enabled && magickBucket) {
                  const imgEntry = timings.beginImage(inputPath)
                  imgEntry.fileCheck(fileCheckMs)
                  imgEntry.magick(0)
                  imgEntry.copy(Date.now() - copyT0)
                  imgEntry.done(Date.now() - imgT0)
                }
              }
              outputFiles.push(outPath)
            }
          }
        } catch (err) {
          failures++
          const msg = err instanceof Error ? err.message : String(err)
          errors.push(`${fileName}: ${msg}`)
          console.error(`[executor] Failed to process ${fileName}:`, err)
        }
        activeImages.delete(fileName)
        onProgress({ completed: ++completed, total: imagePaths.length, currentFile: fileName, active: [...activeImages] })
      }
    }

    // Prevent magick's internal OpenMP thread pool from oversubscribing the CPU.
    // With N concurrent pipelines each trying to use all cores, we'd get N×cores threads
    // competing for cores threads — massive context-switching overhead.  Giving each
    // process an equal share of the hardware threads keeps total thread count at os.cpus().
    const threadsPerProcess = Math.max(1, Math.floor(os.cpus().length / concurrency))
    const prevThreadLimit = process.env.MAGICK_THREAD_LIMIT
    process.env.MAGICK_THREAD_LIMIT = String(threadsPerProcess)
    try {
      await Promise.all(Array.from({ length: concurrency }, processOne))
    } finally {
      if (prevThreadLimit !== undefined) process.env.MAGICK_THREAD_LIMIT = prevThreadLimit
      else delete process.env.MAGICK_THREAD_LIMIT
    }
    // Write collected text output lines to disk (preserving input order).
    if (hasTextOutputNodes && collectedTextLines.length > 0) {
      collectedTextLines.sort((a, b) => a.index - b.index)
      const toNode = textOutputNodes[0]
      const toParams = toNode.data.params as Record<string, unknown>
      // outputPath is the user-configured path in the node inspector;
      // fall back to file_path (old param name) then a safe default.
      const filePath  = String(toParams.outputPath ?? toParams.file_path ?? 'output.txt')
      const appendMode = Boolean(toParams.append ?? false)
      const content   = collectedTextLines.map(l => l.value).join('\n') + '\n'
      try {
        const dir = path.dirname(path.resolve(filePath))
        await fs.promises.mkdir(dir, { recursive: true })
        await fs.promises.writeFile(filePath, content, { flag: appendMode ? 'a' : 'w' })
        outputFiles.push(filePath)
      } catch (err) {
        console.error('[executor] Failed to write text output file:', err)
      }
    }

    if (timings.enabled) {
      const resolvedOutputDir = outputDir ?? (outputFiles.length > 0 ? path.dirname(outputFiles[0]) : null)
      timings.endBatch(resolvedOutputDir)
    }
    return { processed: completed - failures - skipped, skipped, failed: failures, errors, outputFiles }
  }

  // ── CLI script export ────────────────────────────────────────────────────────

  exportCLI(
    shellType: 'powershell' | 'bash' | 'cmd',
    workflowFileName: string,
  ): string {
    const date = new Date().toISOString().slice(0, 10)
    if (shellType === 'powershell') return cliScriptPS(workflowFileName, date)
    if (shellType === 'bash')       return cliScriptBash(workflowFileName, date)
    return cliScriptCmd(workflowFileName, date)
  }

  // ── Cache control ───────────────────────────────────────────────────────────

  clearPreviewCache(): void {
    this.previewCache.clear()
  }
}
