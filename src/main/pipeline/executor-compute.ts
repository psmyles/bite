import fs from 'node:fs';
import path from 'node:path';
import { spawnMagickCapture } from './magick-spawn.js';
import { readHeaderDimensions } from './image-header.js';

// ─── Polymorphic numeric helpers ──────────────────────────────────────────────

export type Numeric = number | number[];

export function numericRaw(val: unknown): Numeric {
  if (Array.isArray(val)) return (val as number[]).map(Number);
  return Number(val ?? 0);
}
export function numericAdd(a: Numeric, b: Numeric): Numeric {
  if (typeof a === 'number' && typeof b === 'number') return a + b;
  if (typeof a === 'number') return (b as number[]).map((x) => x + a);
  if (typeof b === 'number') return (a as number[]).map((x) => x + b);
  return (a as number[]).map((x, i) => x + ((b as number[])[i] ?? 0));
}
export function numericSub(a: Numeric, b: Numeric): Numeric {
  if (typeof a === 'number' && typeof b === 'number') return a - b;
  if (typeof a === 'number') return (b as number[]).map((x) => a - x);
  if (typeof b === 'number') return (a as number[]).map((x) => x - b);
  return (a as number[]).map((x, i) => x - ((b as number[])[i] ?? 0));
}
export function numericMul(a: Numeric, b: Numeric): Numeric {
  if (typeof a === 'number' && typeof b === 'number') return a * b;
  if (typeof a === 'number') return (b as number[]).map((x) => x * a);
  if (typeof b === 'number') return (a as number[]).map((x) => x * b);
  return (a as number[]).map((x, i) => x * ((b as number[])[i] ?? 1));
}
export function numericDiv(a: Numeric, b: Numeric): Numeric {
  const sd = (x: number, d: number) => (d !== 0 ? x / d : 0);
  if (typeof a === 'number' && typeof b === 'number') return sd(a, b);
  if (typeof a === 'number') return (b as number[]).map((d) => sd(a, d));
  if (typeof b === 'number') return (a as number[]).map((x) => sd(x, b));
  return (a as number[]).map((x, i) => sd(x, (b as number[])[i] ?? 0));
}
export function numericPow(base: Numeric, exp: number): Numeric {
  if (typeof base === 'number') return Math.pow(base, exp);
  return base.map((x) => Math.pow(x, exp));
}
export function numericLerp(a: Numeric, b: Numeric, t: number): Numeric {
  if (typeof a === 'number' && typeof b === 'number') return a + (b - a) * t;
  if (typeof a === 'number') return (b as number[]).map((bx) => a + (bx - a) * t);
  if (typeof b === 'number') return (a as number[]).map((ax) => ax + (b - ax) * t);
  return (a as number[]).map((ax, i) => ax + (((b as number[])[i] ?? ax) - ax) * t);
}

/** True if a numeric value is "truthy": non-zero scalar, or any non-zero component in an array. */
export function numericTruthy(val: unknown): boolean {
  if (Array.isArray(val)) return (val as number[]).some((x) => Number(x) !== 0);
  return Boolean(val);
}
/** Scalar representation of a numeric value for ordered comparisons: identity for scalars, magnitude for arrays. */
export function numericScalar(val: unknown): number {
  if (Array.isArray(val)) {
    const arr = (val as number[]).map(Number);
    return Math.sqrt(arr.reduce((s, x) => s + x ** 2, 0));
  }
  return Number(val ?? 0);
}

// ─── Image metadata (for Properties nodes) ────────────────────────────────────

export interface ImageMeta {
  path: string;
  name: string;
  sizeBytes: number;
  width: number;
  height: number;
  bitDepth: number;
  extension: string;
  dpiX: number;
  dpiY: number;
  exif: Record<string, string>;
}

export async function loadImageMeta(imagePath: string): Promise<ImageMeta> {
  const basicOutput = (
    await spawnMagickCapture(['identify', '-format', '%B\n%w\n%h\n%z\n%e\n%x\n%y', imagePath])
  ).trim();

  // identify may return multiple frames; take the first set of 7 lines
  const lines = basicOutput.split('\n');
  const sizeBytes = parseInt(lines[0] ?? '0') || 0;
  const width = parseInt(lines[1] ?? '0') || 0;
  const height = parseInt(lines[2] ?? '0') || 0;
  const bitDepth = parseInt(lines[3] ?? '0') || 0;
  const extension = (lines[4] ?? '').toLowerCase();
  const dpiX = parseFloat(lines[5] ?? '0') || 0;
  const dpiY = parseFloat(lines[6] ?? '0') || 0;

  const exif: Record<string, string> = {};
  try {
    const exifOutput = (
      await spawnMagickCapture(['identify', '-format', '%[EXIF:*]', imagePath]).catch(() => '')
    ).trim();
    // Each line: "exif:Key=Value"
    for (const line of exifOutput.split('\n')) {
      const eqIdx = line.indexOf('=');
      if (eqIdx < 0) continue;
      const rawKey = line.slice(0, eqIdx).trim();
      const key = rawKey.startsWith('exif:') ? rawKey.slice(5) : rawKey;
      exif[key] = line.slice(eqIdx + 1).trim();
    }
  } catch (err) {
    console.warn(`[executor] EXIF read failed for ${imagePath} (non-fatal):`, err);
  }

  return {
    path: imagePath,
    name: path.basename(imagePath),
    sizeBytes,
    width,
    height,
    bitDepth,
    extension,
    dpiX,
    dpiY,
    exif,
  };
}

// ─── Image statistics (mean brightness / channel means) ───────────────────────

export async function loadImageMean(imagePath: string): Promise<number> {
  const output = await spawnMagickCapture([`${imagePath}[0]`, '-format', '%[fx:mean]', 'info:']);
  return parseFloat(output) || 0;
}

const CHANNEL_FX = ['mean.r', 'mean.g', 'mean.b', 'mean.a'] as const;

export async function loadImageChannelMean(imagePath: string, channelIdx: number): Promise<number> {
  const fx = CHANNEL_FX[channelIdx] ?? 'mean';
  const output = await spawnMagickCapture([`${imagePath}[0]`, '-format', `%[fx:${fx}]`, 'info:']);
  return parseFloat(output) || 0;
}

/** Fetch means for multiple channels in a single magick spawn.
 *  channelIndices must be sorted ascending. Returns means in the same order. */
export async function loadMultipleChannelMeans(
  imagePath: string,
  channelIndices: readonly number[]
): Promise<number[]> {
  if (channelIndices.length === 0) return [];
  if (channelIndices.length === 1) return [await loadImageChannelMean(imagePath, channelIndices[0])];
  const format = channelIndices.map((i) => `%[fx:${CHANNEL_FX[i] ?? 'mean'}]`).join('\n');
  const output = await spawnMagickCapture([`${imagePath}[0]`, '-format', format, 'info:']);
  return output.split('\n').map((v) => parseFloat(v) || 0);
}

// ─── Text output helpers ──────────────────────────────────────────────────────

/** Maps a user-facing separator type to the actual separator string. */
export function getSeparator(type: string, custom: string): string {
  switch (type) {
    case 'tab':
      return '\t';
    case 'space':
      return ' ';
    case 'comma':
      return ',';
    case 'custom':
      return custom;
    default:
      return ',';
  }
}

/**
 * Builds a lightweight ImageMeta using only cheap I/O — no ImageMagick spawn.
 * Reads sizeBytes from fs.stat and width/height from the file header.
 * Suitable for prop_name, prop_path, prop_size, prop_filetype, prop_dimensions,
 * and prop_power_of_two nodes; anything needing bitDepth/DPI/EXIF still needs loadImageMeta.
 */
export async function buildEmptyImageMeta(imagePath: string): Promise<ImageMeta> {
  let sizeBytes = 0;
  let width = 0;
  let height = 0;

  if (imagePath !== '') {
    try {
      sizeBytes = (await fs.promises.stat(imagePath)).size;
    } catch {
      /* non-fatal */
    }
    try {
      const dims = await readHeaderDimensions(imagePath);
      if (dims) {
        width = dims.width;
        height = dims.height;
      }
    } catch {
      /* non-fatal — unsupported format stays at 0 */
    }
  }

  return {
    path: imagePath,
    name: path.basename(imagePath),
    sizeBytes,
    width,
    height,
    bitDepth: 0,
    extension: path.extname(imagePath).slice(1).toLowerCase(),
    dpiX: 0,
    dpiY: 0,
    exif: {},
  };
}

// ─── Resize argument builder ──────────────────────────────────────────────────

/**
 * Returns the ImageMagick args for a resize node based on its current params.
 * Used by executor.ts in the preview, batch fast-path, and multi-stream paths.
 */
export function buildResizeArgs(params: Record<string, unknown>): string[] {
  const mode = String(params.mode ?? 'absolute');
  const filter = String(params.filter ?? 'Lanczos');
  const preserve = params.preserve_aspect !== false; // default true

  let geometry: string;
  if (mode === 'relative') {
    if (preserve) {
      const s = Math.max(1, Number(params.scale ?? 100));
      geometry = `${s}%`;
    } else {
      const sw = Math.max(1, Number(params.scale_width ?? params.scale ?? 100));
      const sh = Math.max(1, Number(params.scale_height ?? params.scale ?? 100));
      geometry = `${sw}%x${sh}%!`;
    }
  } else {
    const w = Math.max(1, Math.round(Number(params.width ?? 1024)));
    const h = Math.max(1, Math.round(Number(params.height ?? 1024)));
    if (preserve) {
      const anchor = String(params.anchor ?? 'width');
      geometry = anchor === 'height' ? `x${h}` : `${w}`;
    } else {
      geometry = `${w}x${h}!`;
    }
  }

  const density = Math.round(Number(params.density ?? 72));
  const densityArgs = density > 0 ? ['-density', String(density), '-units', 'PixelsPerInch'] : [];

  return [...densityArgs, '-resize', geometry, '-filter', filter];
}

// ─── Pure value / math / logic computation ────────────────────────────────────

export function computeNodeParams(
  executorKey: string | undefined,
  params: Record<string, unknown>,
  meta?: ImageMeta
): Record<string, unknown> {
  try {
    return computeNodeParamsUnsafe(executorKey, params, meta);
  } catch (err) {
    console.error(`[executor] computeNodeParams threw for key "${executorKey}":`, err, { params });
    return params;
  }
}

type P = Record<string, unknown>;

function computeMath(key: string, p: P): P {
  const n = (k: string) => Number(p[k] ?? 0);
  switch (key) {
    case 'math_add':
      return { ...p, result: numericAdd(numericRaw(p.a), numericRaw(p.b)) };
    case 'math_subtract':
      return { ...p, result: numericSub(numericRaw(p.a), numericRaw(p.b)) };
    case 'math_multiply':
      return { ...p, result: numericMul(numericRaw(p.a), numericRaw(p.b)) };
    case 'math_divide':
      return { ...p, result: numericDiv(numericRaw(p.a), numericRaw(p.b)) };
    case 'math_power':
      return { ...p, result: numericPow(numericRaw(p.base), n('exponent')) };
    case 'math_lerp':
      return { ...p, result: numericLerp(numericRaw(p.a), numericRaw(p.b), n('t')) };
    default:
      return p;
  }
}

function computeLogic(key: string, p: P): P {
  switch (key) {
    case 'logic_and':
      return { ...p, result: numericTruthy(p.a) && numericTruthy(p.b) };
    case 'logic_or':
      return { ...p, result: numericTruthy(p.a) || numericTruthy(p.b) };
    case 'logic_not':
      return { ...p, result: !numericTruthy(p.a) };
    case 'logic_branch':
      return { ...p, result: p.condition ? p.value_true : p.value_false };
    case 'logic_comparison': {
      const op = String(p.operator ?? '==');
      const a = numericScalar(p.a),
        b = numericScalar(p.b);
      const result =
        op === 'equal'
          ? a === b
          : op === 'not equal'
            ? a !== b
            : op === 'greater than'
              ? a > b
              : op === 'less than'
                ? a < b
                : op === 'greater or equal'
                  ? a >= b
                  : op === 'less or equal'
                    ? a <= b
                    : false;
      return { ...p, result };
    }
    default:
      return p;
  }
}

function computeVector(key: string, p: P): P {
  const n = (k: string) => Number(p[k] ?? 0);
  switch (key) {
    case 'split_vec': {
      const inp = p.vec;
      if (Array.isArray(inp)) {
        const arr = (inp as number[]).map(Number);
        return { ...p, x: arr[0] ?? 0, y: arr[1] ?? 0, z: arr[2] ?? 0, w: arr[3] ?? 0 };
      }
      return { ...p, x: Number(inp ?? 0), y: 0, z: 0, w: 0 };
    }
    case 'append_vec': {
      const dim = Math.max(2, Math.min(4, Math.round(Number(p.dimensions ?? 4))));
      return { ...p, result: [n('x'), n('y'), n('z'), n('w')].slice(0, dim) };
    }
    case 'vec_math_dot': {
      const a = numericRaw(p.a),
        b = numericRaw(p.b);
      if (typeof a === 'number' && typeof b === 'number') return { ...p, result: a * b };
      const av = Array.isArray(a) ? a : [a as number];
      const bv = Array.isArray(b) ? b : [b as number];
      return { ...p, result: av.reduce((sum: number, x: number, i: number) => sum + x * (bv[i] ?? 0), 0) };
    }
    case 'vec_math_length': {
      const vec = numericRaw(p.vec);
      if (typeof vec === 'number') return { ...p, result: Math.abs(vec) };
      return { ...p, result: Math.sqrt(vec.reduce((s: number, x: number) => s + x ** 2, 0)) };
    }
    case 'vec_math_normalize': {
      const vec = numericRaw(p.vec);
      if (typeof vec === 'number') return { ...p, result: vec === 0 ? 0 : vec / Math.abs(vec) };
      const len = Math.sqrt(vec.reduce((s: number, x: number) => s + x ** 2, 0));
      return { ...p, result: len === 0 ? vec.map(() => 0) : vec.map((x: number) => x / len) };
    }
    default:
      return p;
  }
}

function computeProperties(key: string, p: P, meta?: ImageMeta): P {
  switch (key) {
    case 'prop_name': {
      const rawName = meta?.name ?? '';
      return { ...p, value: p.strip_extension ? rawName.replace(/\.[^.]+$/, '') : rawName };
    }
    case 'prop_path': {
      const fullPath = meta?.path ?? '';
      return { ...p, value: p.strip_filename ? path.dirname(fullPath) : fullPath };
    }
    case 'prop_filetype':
      return { ...p, value: meta?.extension ?? '' };
    case 'prop_bitdepth':
      return { ...p, value: meta?.bitDepth ?? 0 };
    case 'prop_dimensions':
      return { ...p, width: meta?.width ?? 0, height: meta?.height ?? 0 };
    case 'prop_power_of_two': {
      const isPot = (n: number) => n > 0 && (n & (n - 1)) === 0;
      const width_ok = isPot(meta?.width ?? 0);
      const height_ok = isPot(meta?.height ?? 0);
      return { ...p, width_ok, height_ok, result: width_ok && height_ok };
    }
    case 'prop_resolution':
      return { ...p, dpi_x: meta?.dpiX ?? 0, dpi_y: meta?.dpiY ?? 0 };
    case 'prop_size': {
      const unit = String(p.unit ?? 'bytes');
      const bytes = meta?.sizeBytes ?? 0;
      const value =
        unit === 'KB'
          ? bytes / 1024
          : unit === 'MB'
            ? bytes / (1024 * 1024)
            : unit === 'GB'
              ? bytes / (1024 * 1024 * 1024)
              : bytes;
      return { ...p, value };
    }
    case 'prop_exif': {
      if (!meta) return p;
      const exif = meta.exif;
      const exifStr = (k: string) => exif[k] ?? '';
      const exifRational = (k: string): number => {
        const v = exif[k] ?? '';
        const m = v.match(/^(\d+)\/(\d+)$/);
        if (m) {
          const d = parseInt(m[2]);
          return d !== 0 ? parseInt(m[1]) / d : 0;
        }
        return parseFloat(v) || 0;
      };
      const exifInt = (k: string): number => parseInt((exif[k] ?? '').split(',')[0]) || 0;
      return {
        ...p,
        camera_make: exifStr('Make'),
        camera_model: exifStr('Model'),
        lens: exifStr('LensModel') || exifStr('LensMake'),
        exposure_time: exifStr('ExposureTime'),
        shutter_speed: exifStr('ShutterSpeedValue'),
        aperture: exifRational('FNumber') || exifRational('ApertureValue'),
        iso: exifInt('PhotographicSensitivity') || exifInt('ISOSpeedRatings'),
        focal_length: exifRational('FocalLength'),
        date_taken: exifStr('DateTimeOriginal'),
      };
    }
    default:
      return p;
  }
}

function computeValues(key: string, p: P): P {
  switch (key) {
    case 'text_output':
      return p; // wiring resolved upstream; no compute needed
    case 'text_filter': {
      const input = String(p.input ?? '');
      const prefix = String(p.prefix ?? '');
      const suffix = String(p.suffix ?? '');
      const contains = String(p.contains ?? '');
      const matchCase = Boolean(p.match_case);
      const norm = (s: string) => (matchCase ? s : s.toLowerCase());
      const result =
        (prefix === '' || norm(input).startsWith(norm(prefix))) &&
        (suffix === '' || norm(input).endsWith(norm(suffix))) &&
        (contains === '' || norm(input).includes(norm(contains)));
      return { ...p, result };
    }
    case 'value_color': {
      const c = Array.isArray(p.color) ? (p.color as number[]).map(Number) : [1, 1, 1, 1];
      const [r = 1, g = 1, b = 1, a = 1] = c;
      return { ...p, rgba: [r, g, b, a], rgb: [r, g, b], r, g, b, a };
    }
    case 'value_vector2': {
      const v = Array.isArray(p.vec) ? (p.vec as number[]).map(Number) : [0, 0];
      const [x = 0, y = 0] = v;
      return { ...p, xy: [x, y], x, y };
    }
    case 'value_vector3': {
      const v = Array.isArray(p.vec) ? (p.vec as number[]).map(Number) : [0, 0, 0];
      const [x = 0, y = 0, z = 0] = v;
      return { ...p, xyz: [x, y, z], x, y, z };
    }
    case 'value_vector4': {
      const v = Array.isArray(p.vec) ? (p.vec as number[]).map(Number) : [0, 0, 0, 0];
      const [x = 0, y = 0, z = 0, w = 0] = v;
      return { ...p, xyzw: [x, y, z, w], x, y, z, w };
    }
    default:
      return p;
  }
}

export function computeNodeParamsUnsafe(
  executorKey: string | undefined,
  params: Record<string, unknown>,
  meta?: ImageMeta
): Record<string, unknown> {
  // compute_js nodes: executor.ts injects the JS body via a hidden key to avoid
  // changing the call signature across the codebase.
  if (typeof params.__compute_js__ === 'string') {
    const jsBody = params.__compute_js__ as string;
    const cleanParams: Record<string, unknown> = { ...params };
    delete cleanParams.__compute_js__;
    const fn = new Function('params', jsBody) as (p: Record<string, unknown>) => unknown;
    const out = fn(cleanParams);
    if (typeof out !== 'object' || out === null || Array.isArray(out)) {
      throw new Error(`[executor] compute_js must return a plain object of output values`);
    }
    return { ...cleanParams, ...(out as Record<string, unknown>) };
  }

  const key = executorKey ?? '';
  if (key.startsWith('math_')) return computeMath(key, params);
  if (key.startsWith('logic_')) return computeLogic(key, params);
  if (key === 'split_vec' || key.startsWith('append_') || key.startsWith('vec_math_'))
    return computeVector(key, params);
  if (key.startsWith('prop_')) return computeProperties(key, params, meta);
  if (key.startsWith('value_') || key === 'text_filter' || key === 'text_output') return computeValues(key, params);

  // executorKey === undefined means image node (caller passes undefined intentionally).
  // 'comment' is a UI-only node with no computation.
  if (executorKey !== undefined && executorKey !== 'comment') {
    console.warn(
      `[executor] Unknown executor key: "${executorKey}" — params returned unchanged. Add a case to computeNodeParams.`
    );
  }
  return params;
}
