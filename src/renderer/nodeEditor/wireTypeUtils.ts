/**
 * Wire-type compatibility utilities shared between NodeEditor.svelte and nodeEditorHelpers.ts.
 * No Svelte reactivity — pure TypeScript.
 */

/** Wire types that are all numeric and compatible with 'numeric' */
export const numericWireTypes = new Set(['number', 'numeric', 'vector2', 'vector3', 'vector4', 'color'])

/** Wire types that are scalar (single-value numeric) and inter-compatible */
export const scalarTypes = new Set(['boolean', 'number', 'numeric'])

/**
 * Returns true when two wire types are compatible for a connection.
 * 'any' matches everything; numeric sub-types coerce between each other.
 */
export function wireTypesCompatible(a: string, b: string): boolean {
  if (a === b) return true
  if (a === 'any' || b === 'any') return true
  if ((a === 'numeric' || b === 'numeric') && numericWireTypes.has(a) && numericWireTypes.has(b)) return true
  if (scalarTypes.has(a) && scalarTypes.has(b)) return true
  return false
}

/** Convert a node param definition type string to its wire type equivalent. */
export function paramTypeToWireType(paramType: string): string {
  if (paramType === 'bool')    return 'boolean'
  if (paramType === 'string')  return 'string'
  if (paramType === 'vector2') return 'vector2'
  if (paramType === 'vector3') return 'vector3'
  if (paramType === 'vector4') return 'vector4'
  if (paramType === 'color')   return 'color'
  if (paramType === 'numeric') return 'numeric'
  if (paramType === 'any')     return 'any'
  return 'number'
}

/** Canonical handle ID for a writable param input port. */
export const paramInHandle  = (name: string): string => `param-in-${name}`
/** Canonical handle ID for a readonly param output port. */
export const paramOutHandle = (name: string): string => `param-out-${name}`
