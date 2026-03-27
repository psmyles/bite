import fs from 'node:fs'
import path from 'node:path'
import type { NodeDefinition, VisibilityRule } from '../../shared/types.js'

type ChangeListener = (definitions: NodeDefinition[]) => void

export class NodeRegistry {
  private definitions = new Map<string, NodeDefinition>()
  private listeners: ChangeListener[] = []
  private watcher: fs.FSWatcher | null = null
  private debounceTimer: ReturnType<typeof setTimeout> | null = null

  async load(nodeDefinitionsDir: string): Promise<void> {
    this.definitions.clear()

    let files: string[]
    try {
      files = await fs.promises.readdir(nodeDefinitionsDir)
    } catch {
      console.warn(`[registry] Directory not found: ${nodeDefinitionsDir}`)
      return
    }

    await Promise.all(
      files
        .filter((f) => f.endsWith('.json'))
        .map((f) => this.loadFile(path.join(nodeDefinitionsDir, f)))
    )

    console.log(`[registry] Loaded ${this.definitions.size} node definition(s)`)
  }

  watch(nodeDefinitionsDir: string): void {
    try {
      this.watcher = fs.watch(nodeDefinitionsDir, { persistent: false }, (_event, filename) => {
        if (!filename?.endsWith('.json')) return
        if (this.debounceTimer) clearTimeout(this.debounceTimer)
        this.debounceTimer = setTimeout(async () => {
          await this.load(nodeDefinitionsDir)
          this.notify()
        }, 100)
      })
      console.log(`[registry] Watching ${nodeDefinitionsDir}`)
    } catch (err) {
      console.warn('[registry] Could not start watcher:', err)
    }
  }

  onChange(listener: ChangeListener): () => void {
    this.listeners.push(listener)
    return () => {
      this.listeners = this.listeners.filter((l) => l !== listener)
    }
  }

  get(id: string): NodeDefinition | undefined {
    return this.definitions.get(id)
  }

  getAll(): NodeDefinition[] {
    return [...this.definitions.values()]
  }

  stop(): void {
    this.watcher?.close()
    this.watcher = null
    if (this.debounceTimer) clearTimeout(this.debounceTimer)
  }

  private async loadFile(filePath: string): Promise<void> {
    try {
      const raw = await fs.promises.readFile(filePath, 'utf-8')
      const data: unknown = JSON.parse(raw)
      const errors = validate(data)
      if (errors.length > 0) {
        console.warn(`[registry] Invalid definition ${path.basename(filePath)}:`, errors)
        return
      }
      const def = data as NodeDefinition
      this.definitions.set(def.id, def)
    } catch (err) {
      console.warn(`[registry] Failed to load ${path.basename(filePath)}:`, err)
    }
  }

  private notify(): void {
    const defs = this.getAll()
    for (const listener of this.listeners) listener(defs)
  }
}

// ─── Runtime validation ───────────────────────────────────────────────────────

function validate(data: unknown): string[] {
  if (typeof data !== 'object' || data === null) return ['Must be an object']
  const d = data as Record<string, unknown>
  const errors: string[] = []
  if (typeof d.id !== 'string' || !d.id) errors.push('Missing "id" (string)')
  if (typeof d.label !== 'string' || !d.label) errors.push('Missing "label" (string)')
  if (typeof d.category !== 'string' || !d.category) errors.push('Missing "category" (string)')
  if (!Array.isArray(d.inputs)) errors.push('"inputs" must be an array')
  if (!Array.isArray(d.outputs)) errors.push('"outputs" must be an array')
  if (!Array.isArray(d.params)) errors.push('"params" must be an array')
  const IMAGE_TYPES = new Set(['image', 'mask'])
  const hasImagePorts =
    (Array.isArray(d.inputs)  && (d.inputs  as Array<{ type: string }>).some(p => IMAGE_TYPES.has(p.type))) ||
    (Array.isArray(d.outputs) && (d.outputs as Array<{ type: string }>).some(p => IMAGE_TYPES.has(p.type)))
  if (hasImagePorts && !d.command_template && !d.command_js && !d.executor)
    errors.push('Must have either "command_template", "command_js", or "executor"')
  if (d.command_template && d.command_js)
    errors.push('Cannot have both "command_template" and "command_js"')

  // Syntax-check JS fields at load time so typos surface immediately
  if (typeof d.command_js === 'string') {
    try { new Function('params', d.command_js) }
    catch (e) { errors.push(`command_js syntax error: ${e}`) }
  }
  if (typeof d.compute_js === 'string') {
    try { new Function('params', d.compute_js) }
    catch (e) { errors.push(`compute_js syntax error: ${e}`) }
  }

  // params_visibility rules must reference real param names
  if (Array.isArray(d.params_visibility) && Array.isArray(d.params)) {
    const paramNames = new Set((d.params as Array<{ name: string }>).map((p) => p.name))
    for (const rule of d.params_visibility as VisibilityRule[]) {
      if (typeof rule.show !== 'string' || !paramNames.has(rule.show))
        errors.push(`params_visibility: unknown param "${rule.show}"`)
      if (typeof rule.when?.param !== 'string' || !paramNames.has(rule.when.param))
        errors.push(`params_visibility: unknown condition param "${rule.when?.param}"`)
    }
  }

  return errors
}
