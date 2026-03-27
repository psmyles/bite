// Translates a node definition + current param values into ImageMagick CLI argument tokens
import type { NodeDefinition } from '../../shared/types.js'

/**
 * Interpolates {{param}} placeholders in a node's command_template and returns
 * the result as a pre-split argument array ready to spread into spawn().
 * Falls back to the param's default value when the params record has no entry.
 * Returns [] for nodes that use a TypeScript executor (no command_template).
 */
export function buildCommandArgs(def: NodeDefinition, params: Record<string, unknown>): string[] {
  if (!def.command_template) return []

  const interpolated = def.command_template.replace(/\{\{(\w+)\}\}/g, (_match, key: string) => {
    const val = key in params ? params[key] : def.params.find((p) => p.name === key)?.default
    return val != null ? String(val) : ''
  })

  return interpolated.split(/\s+/).filter((s) => s.length > 0)
}

/**
 * Executes a node's command_js field in a minimal sandbox (only `params` in scope)
 * and returns the resulting ImageMagick argument array.
 * Throws if the function doesn't return string[].
 */
export function buildCommandArgsFromJs(def: NodeDefinition, params: Record<string, unknown>): string[] {
  // eslint-disable-next-line @typescript-eslint/no-implied-eval
  const fn = new Function('params', def.command_js!) as (p: Record<string, unknown>) => unknown
  const result = fn(params)
  if (!Array.isArray(result) || result.some((x) => typeof x !== 'string')) {
    throw new Error(`[${def.id}] command_js must return string[] — got: ${JSON.stringify(result)}`)
  }
  return result as string[]
}
