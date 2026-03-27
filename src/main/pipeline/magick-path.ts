import path from 'node:path'
import fs from 'node:fs'

/**
 * Returns the path to the magick binary.
 *
 * - In development (not packaged): returns 'magick' and relies on PATH.
 * - In a packaged app: looks for the bundled binary under process.resourcesPath.
 *   Falls back to 'magick' (system PATH) if the bundled binary is not found,
 *   so the app still works on platforms where we haven't bundled a binary.
 */
let _cached: string | undefined

export function getMagickBinary(): string {
  if (_cached !== undefined) return _cached
  _cached = _resolve()
  return _cached
}

function _resolve(): string {
  const exe = process.platform === 'win32' ? 'magick.exe' : 'magick'

  // pkg-compiled CLI binary: process.pkg is defined.
  // The CLI exe sits at the install root; magick is in resources/magick/ alongside it
  // (same location the Electron app's extraResources puts it).
  if ((process as NodeJS.Process & { pkg?: unknown }).pkg) {
    const candidate = path.join(path.dirname(process.execPath), 'resources', 'magick', exe)
    if (fs.existsSync(candidate)) return candidate
    return 'magick'
  }

  // Electron packaged app: process.resourcesPath is set.
  const resourcesPath: string | undefined =
    (process as NodeJS.Process & { resourcesPath?: string }).resourcesPath
  if (resourcesPath) {
    const candidate = path.join(resourcesPath, 'magick', exe)
    if (fs.existsSync(candidate)) return candidate
  }

  // Dev mode — use system magick from PATH
  return 'magick'
}
