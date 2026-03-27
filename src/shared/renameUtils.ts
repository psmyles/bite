// Pure filename transformation logic — no I/O, usable in both main and renderer processes.

export type TextBlock    = { type: 'text';    value: string }
export type NumberBlock  = { type: 'number';  start: number; pad: number }
export type OldNameBlock = { type: 'oldname'; find: string; replace_with: string }
export type NameBlock    = TextBlock | NumberBlock | OldNameBlock

export interface RenameParams {
  blocks?: NameBlock[]
}

/** Split a filename into stem and extension. */
function splitExt(name: string): { stem: string; ext: string } {
  const dot = name.lastIndexOf('.')
  if (dot <= 0) return { stem: name, ext: '' }
  return { stem: name.slice(0, dot), ext: name.slice(dot) }
}

/**
 * Compute the new filename (with extension) for a given original filename and
 * 0-based position index within the current batch.
 */
export function computeNewName(
  originalName: string,
  params: RenameParams,
  index: number,
): string {
  const { stem, ext } = splitExt(originalName)
  const blocks = params.blocks ?? []
  if (blocks.length === 0) return originalName

  let name = ''
  for (const block of blocks) {
    if (block.type === 'text') {
      name += block.value
    } else if (block.type === 'number') {
      const start = Math.max(0, Math.round(Number(block.start ?? 1)))
      const pad   = Math.max(1, Math.round(Number(block.pad   ?? 2)))
      name += String(start + index).padStart(pad, '0')
    } else if (block.type === 'oldname') {
      let s = stem
      if (block.find) s = s.split(block.find).join(block.replace_with ?? '')
      name += s
    }
  }
  return name + ext
}

/** Compute preview rows for an array of original filenames. */
export function previewRenames(
  originalNames: string[],
  params: RenameParams,
): Array<{ original: string; newName: string; changed: boolean }> {
  return originalNames.map((name, i) => {
    const newName = computeNewName(name, params, i)
    return { original: name, newName, changed: newName !== name }
  })
}
