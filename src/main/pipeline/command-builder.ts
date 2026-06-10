// Translates a node definition + current param values into ImageMagick CLI argument tokens
import type { NodeDefinition, FormatDefinition } from '../../shared/types.js';

const _rawFmtDefs = import.meta.glob('../../../format-definitions/*.json', { eager: true });
const _fmtDefs: Record<string, FormatDefinition> = {};
for (const mod of Object.values(_rawFmtDefs)) {
  const d = mod as FormatDefinition;
  _fmtDefs[d.id.toUpperCase()] = d;
}

export function buildFormatConvertArgs(format: string, params: Record<string, unknown>): string[] {
  const def = _fmtDefs[format.toUpperCase()];
  if (!def) return [];

  // Backward compat: map old shared `quality` param to the format-specific key so
  // workflows saved before the per-format param system still produce the correct output.
  const resolvedParams = { ...params };
  const oldQuality = params.quality;
  if (typeof oldQuality === 'number') {
    const qualityKey: Record<string, string> = { JPEG: 'jpeg_quality', WEBP: 'webp_quality', AVIF: 'avif_quality' };
    const key = qualityKey[format.toUpperCase()];
    if (key && !(key in params)) resolvedParams[key] = oldQuality;
  }

  // eslint-disable-next-line @typescript-eslint/no-implied-eval
  const result = new Function('params', def.args_js)(resolvedParams) as unknown;
  if (!Array.isArray(result) || result.some((x) => typeof x !== 'string')) {
    throw new Error(`[${format}] args_js must return string[] — got: ${JSON.stringify(result)}`);
  }
  return result as string[];
}

export function getFormatExtension(format: string): string {
  return _fmtDefs[format.toUpperCase()]?.extension ?? '.png';
}

/**
 * Interpolates {{param}} placeholders in a node's command_template and returns
 * the result as a pre-split argument array ready to spread into spawn().
 * Falls back to the param's default value when the params record has no entry.
 * Returns [] for nodes that use a TypeScript executor (no command_template).
 */
export function buildCommandArgs(def: NodeDefinition, params: Record<string, unknown>): string[] {
  if (!def.command_template) return [];

  // Split on whitespace first (preserving template token boundaries), then substitute
  // params within each word. This keeps adjacent literals+params as one token
  // (e.g. "{{w}}x{{h}}" → "1024x768") while also keeping multi-word param values
  // as single tokens (e.g. color="rgba(0, 0, 0, 1)" stays unsplit).
  return def.command_template
    .split(/\s+/)
    .filter((word) => word.length > 0)
    .map((word) =>
      word.replace(/\{\{(\w+)\}\}/g, (_m, key: string) => {
        const val = key in params ? params[key] : def.params.find((p) => p.name === key)?.default;
        return val != null ? String(val) : '';
      })
    )
    .filter((word) => word.length > 0);
}

/**
 * Executes a node's command_js field in a minimal sandbox (only `params` in scope)
 * and returns the resulting ImageMagick argument array.
 * Throws if the function doesn't return string[].
 */
export function buildCommandArgsFromJs(def: NodeDefinition, params: Record<string, unknown>): string[] {
  // eslint-disable-next-line @typescript-eslint/no-implied-eval
  const fn = new Function('params', def.command_js!) as (p: Record<string, unknown>) => unknown;
  const result = fn(params);
  if (!Array.isArray(result) || result.some((x) => typeof x !== 'string')) {
    throw new Error(`[${def.id}] command_js must return string[] — got: ${JSON.stringify(result)}`);
  }
  return result as string[];
}
