import path from 'node:path';
import fs from 'node:fs';

/**
 * Resolves the magick binary, and the environment it needs, for every way the
 * app ships:
 *
 * - Windows: a static magick.exe at <resources>/magick/magick.exe - no env needed.
 * - macOS: a relocated Homebrew build at <resources>/magick/bin/magick, whose
 *   dylibs, coder modules and config files sit alongside it.  The compiled-in
 *   paths still point at the build machine's Homebrew tree, so the module and
 *   config paths have to be handed to it through the environment (see
 *   scripts/bundle-magick-mac.sh).
 * - Anywhere without a bundled binary: 'magick' from PATH.
 */
/** Extra environment variables layered over process.env when spawning magick. */
export type MagickEnv = Record<string, string>;

let _cached: string | undefined;
let _cachedEnv: MagickEnv | undefined;

export function getMagickBinary(): string {
  if (_cached === undefined) _resolve();
  return _cached!;
}

/**
 * Environment overrides the bundled binary needs, to merge over process.env.
 * Empty for a system magick and for the Windows bundle.
 */
export function getMagickEnv(): MagickEnv {
  if (_cached === undefined) _resolve();
  return _cachedEnv!;
}

/** Test seam - drops the memoized resolution. */
export function resetMagickPathCache(): void {
  _cached = undefined;
  _cachedEnv = undefined;
}

function _resolve(): void {
  const root = _bundleRoot();
  if (root === undefined) {
    _cached = 'magick';
    _cachedEnv = {};
    return;
  }
  _cached = _binaryIn(root)!;
  _cachedEnv = process.platform === 'darwin' ? _darwinEnv(root) : {};
}

/** The bundled magick root (the directory holding bin/ or magick.exe), if present. */
function _bundleRoot(): string | undefined {
  const candidates: string[] = [];

  // pkg-compiled CLI binary: the bundle sits next to the executable, in the
  // same layout the Electron app's extraResources produces.
  if ((process as NodeJS.Process & { pkg?: unknown }).pkg) {
    const dir = path.dirname(process.execPath);
    candidates.push(path.join(dir, 'magick'), path.join(dir, 'resources', 'magick'));
  }

  // Electron packaged app.
  const resourcesPath: string | undefined = (process as NodeJS.Process & { resourcesPath?: string }).resourcesPath;
  if (resourcesPath) candidates.push(path.join(resourcesPath, 'magick'));

  // Dev mode - use the vendored bundle for this platform if it has been built.
  if (process.env.APP_ROOT) {
    const perPlatform = process.platform === 'win32' ? 'win' : process.platform === 'darwin' ? 'mac' : 'linux';
    candidates.push(path.join(process.env.APP_ROOT, 'resources', perPlatform, 'magick'));
  }

  return candidates.find((c) => _binaryIn(c) !== undefined);
}

/** The magick executable inside a bundle root, or undefined if it isn't there. */
function _binaryIn(root: string): string | undefined {
  const candidate = process.platform === 'win32' ? path.join(root, 'magick.exe') : path.join(root, 'bin', 'magick');
  return fs.existsSync(candidate) ? candidate : undefined;
}

function _darwinEnv(root: string): MagickEnv {
  // Version-stamped directory names (modules-Q16HDRI, ImageMagick-7, ...) vary
  // with the build, so discover them rather than hard-coding.
  const imLib = path.join(root, 'lib', 'ImageMagick');
  const modulesDir = _childStartingWith(imLib, 'modules-');
  const configDir = _childStartingWith(imLib, 'config-');
  const etcDir = _childStartingWith(path.join(root, 'etc'), 'ImageMagick-');
  const shareDir = _childStartingWith(path.join(root, 'share'), 'ImageMagick-');

  const env: MagickEnv = { MAGICK_HOME: root };
  const configurePath = [configDir, etcDir, shareDir].filter((d): d is string => d !== undefined);
  if (configurePath.length > 0) env.MAGICK_CONFIGURE_PATH = configurePath.join(path.delimiter);
  if (modulesDir !== undefined) {
    env.MAGICK_CODER_MODULE_PATH = path.join(modulesDir, 'coders');
    env.MAGICK_FILTER_MODULE_PATH = path.join(modulesDir, 'filters');
  }
  return env;
}

function _childStartingWith(dir: string, prefix: string): string | undefined {
  let entries: string[];
  try {
    entries = fs.readdirSync(dir);
  } catch {
    return undefined;
  }
  const match = entries.find((e) => e.startsWith(prefix));
  return match === undefined ? undefined : path.join(dir, match);
}
