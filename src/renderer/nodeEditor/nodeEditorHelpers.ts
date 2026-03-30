// Pure helper functions for NodeEditor — no Svelte reactivity ($state, $derived, etc.)
import type { NodeDefinition } from '../../shared/types.js'
import { numericWireTypes, scalarTypes } from './wireTypeUtils.js'

// ─── NodeData ─────────────────────────────────────────────────────────────────
// The shape of data stored in every graph node. Re-exported here so components
// outside nodeEditor don't need to cast to Record<string, unknown>.

export interface ParamPortDef {
  name: string
  type: string
  label: string
  readonly: boolean
  default?: unknown
  portOnly: boolean
  noPort: boolean
}

export interface NodeData {
  label: string
  description?: string
  definitionId: string
  inputs?: string[]
  outputs?: string[]
  inputLabels?: string[]
  outputLabels?: string[]
  params?: Record<string, unknown>
  paramDefs?: ParamPortDef[]
}

/** Safely extract the typed params object from a raw node data value. */
export function getNodeParams(data: unknown): Record<string, unknown> {
  if (typeof data !== 'object' || data === null) return {}
  const p = (data as Record<string, unknown>).params
  return typeof p === 'object' && p !== null ? p as Record<string, unknown> : {}
}

/** Sort nodes so group nodes appear before their children. Required by @xyflow for parent-child rendering. */
export function sortNodesGroupFirst<T extends { type?: string }>(nodes: T[]): T[] {
  return [...nodes].sort((a, b) => {
    if (a.type === 'group' && b.type !== 'group') return -1
    if (b.type === 'group' && a.type !== 'group') return 1
    return 0
  })
}

export function nodeTypeForDef(def: NodeDefinition): string {
  if (def.id === 'comment')          return 'commentNode'
  if (def.id === 'folderpath')       return 'folderPathNode'
  if (def.id === 'atlas')            return 'atlasNode'
  if (def.id === 'logic_comparison') return 'compareNode'
  return 'process'
}

/** Compute dynamic paramDefs for a resize node based on current mode + preserve_aspect. */
export function buildResizeParamDefs(mode: string, preserve: boolean): ParamPortDef[] {
  const defs: ParamPortDef[] = [
    { name: 'preserve_aspect', type: 'bool', label: 'Preserve Aspect', readonly: false, default: true, portOnly: false, noPort: false },
  ]
  if (mode === 'relative') {
    defs.push({ name: 'scale_width', type: 'float', label: 'Scale Width', readonly: false, default: 100, portOnly: false, noPort: false })
    if (!preserve)
      defs.push({ name: 'scale_height', type: 'float', label: 'Scale Height', readonly: false, default: 100, portOnly: false, noPort: false })
  } else {
    defs.push({ name: 'width', type: 'int', label: 'Width', readonly: false, default: 1024, portOnly: false, noPort: false })
    if (!preserve)
      defs.push({ name: 'height', type: 'int', label: 'Height', readonly: false, default: 1024, portOnly: false, noPort: false })
  }
  return defs
}

export function buildNodeData(def: NodeDefinition) {
  const params: Record<string, unknown> = {}
  for (const p of def.params) params[p.name] = p.default
  // Comment nodes store heading/body in params (not in JSON def) so they're saved/loaded
  if (def.id === 'comment') {
    params.heading = 'Comment'
    params.body    = ''
  }
  // Seed the bypass param for actionable (image-in + image-out) nodes
  const hasImageIn  = def.inputs.some((p) => p.type === 'image' || p.type === 'mask')
  const hasImageOut = def.outputs.some((p) => p.type === 'image' || p.type === 'mask')
  if (hasImageIn && hasImageOut) params._enabled = true
  // If the definition has a 'channels' param, trim image input ports to match its default.
  const channelsDefault = def.params.find((p) => p.name === 'channels')?.default
  const portCount = channelsDefault != null ? Number(channelsDefault) : Infinity
  const inputSlice = isNaN(portCount) ? def.inputs : def.inputs.slice(0, portCount)

  return {
    label: def.label,
    description: def.description ?? '',
    definitionId: def.id,
    inputs: inputSlice.map((p) => p.type),
    outputs: def.outputs.map((p) => p.type),
    inputLabels: inputSlice.map((p) => p.label),
    outputLabels: def.outputs.map((p) => p.label),
    params,
    // Resize ports are mode+preserve_aspect-dependent — start with absolute+preserve defaults.
    // enum params are not connectable; all other typed params get ports.
    paramDefs: def.id === 'resize'
      ? buildResizeParamDefs(String(params.mode ?? 'absolute'), params.preserve_aspect !== false)
      : def.params
          .filter((p) => p.type !== 'enum')
          .map((p) => ({ name: p.name, type: p.type, label: p.label, readonly: p.readonly ?? false, default: p.default, portOnly: p.portOnly ?? false, noPort: p.noPort ?? false })),
  }
}

/**
 * Reconstruct full node data from a saved slim record { definitionId, params }.
 * Merges saved user params over the definition defaults, then re-derives any
 * fields that depend on param values (e.g. the inputs array for channel_merge).
 */
export function expandNodeData(def: NodeDefinition, savedParams: Record<string, unknown>) {
  const base = buildNodeData(def)
  const params = { ...base.params, ...savedParams }

  // Re-derive the inputs array when a 'channels' param was saved with a non-default value
  const channelsDef = def.params.find((p) => p.name === 'channels')
  if (channelsDef != null) {
    const portCount = Number(params.channels ?? channelsDef.default)
    const inputSlice = isNaN(portCount) ? def.inputs : def.inputs.slice(0, portCount)
    return {
      ...base,
      inputs: inputSlice.map((p) => p.type),
      inputLabels: inputSlice.map((p) => p.label),
      params,
    }
  }

  // Re-derive resize paramDefs from the saved mode + preserve_aspect values
  if (def.id === 'resize') {
    return {
      ...base,
      params,
      paramDefs: buildResizeParamDefs(String(params.mode ?? 'absolute'), params.preserve_aspect !== false),
    }
  }

  return { ...base, params }
}

/**
 * For wire-drop auto-connect: returns the first handle ID on `def` that matches
 * the wire type on the requested side ('in' = target/input, 'out' = source/output).
 */
export function firstMatchingHandle(def: NodeDefinition, wt: string, side: 'in' | 'out'): string | null {
  if (wt === 'image' || wt === 'mask') return side === 'in' ? 'in-0' : 'out-0'
  // Check port-level inputs/outputs for non-image types (e.g. 'path')
  if (side === 'in') {
    const idx = wt === 'any' ? 0 : def.inputs.findIndex(p => p.type === wt)
    if (idx !== -1 && def.inputs[idx]) return `in-${idx}`
  } else {
    const idx = wt === 'any' ? 0 : def.outputs.findIndex(p => p.type === wt)
    if (idx !== -1 && def.outputs[idx]) return `out-${idx}`
  }
  for (const p of def.params) {
    if (p.type === 'enum') continue
    const paramWt =
      p.type === 'bool'    ? 'boolean' :
      p.type === 'string'  ? 'string'  :
      p.type === 'vector2' ? 'vector2' :
      p.type === 'vector3' ? 'vector3' :
      p.type === 'vector4' ? 'vector4' :
      p.type === 'color'   ? 'color'   :
      p.type === 'numeric' ? 'numeric' :
      p.type === 'any'     ? 'any'     : 'number'
    const typesMatch = paramWt === wt ||
      paramWt === 'any' || wt === 'any' ||
      ((paramWt === 'numeric' || wt === 'numeric') && numericWireTypes.has(paramWt) && numericWireTypes.has(wt)) ||
      (scalarTypes.has(paramWt) && scalarTypes.has(wt))
    if (!typesMatch) continue
    if (side === 'out' && p.readonly)  return `param-out-${p.name}`
    if (side === 'in'  && !p.readonly) return `param-in-${p.name}`
  }
  return null
}
