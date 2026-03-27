// Registry for custom image-node executors.
//
// An ArgBuilderFn covers the standard single-in / single-out pattern: given a node
// definition and its resolved params it returns the ImageMagick CLI args to place
// between the input path and the output path. This matches what command_template and
// command_js produce, so registered executors slot into the same execution paths.
//
// Executors with more complex semantics (multiple image inputs/outputs, side effects
// on pipeline state) currently remain as hardcoded branches inside executor.ts.
// A richer async context interface is deferred to a later phase.

import type { NodeDefinition } from '../../shared/types.js'

export type ArgBuilderFn = (def: NodeDefinition, params: Record<string, unknown>) => string[]

const registry = new Map<string, ArgBuilderFn>()

export function registerExecutor(key: string, fn: ArgBuilderFn): void {
  if (registry.has(key)) console.warn(`[executorRegistry] Overwriting executor "${key}"`)
  registry.set(key, fn)
}

export function getExecutor(key: string): ArgBuilderFn | undefined {
  return registry.get(key)
}

export function registeredKeys(): string[] {
  return [...registry.keys()]
}
