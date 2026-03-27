// ─── Node Definition (JSON schema) ───────────────────────────────────────────

export type ParamType = 'int' | 'float' | 'string' | 'enum' | 'bool'
                      | 'vector2' | 'vector3' | 'vector4' | 'color'
                      | 'numeric' | 'any'
export type WidgetType = 'slider' | 'number' | 'dropdown' | 'text' | 'checkbox' | 'color-picker' | 'vector'

export interface ParamDefinition {
  name: string
  label: string
  type: ParamType
  widget?: WidgetType
  default?: number | string | boolean | number[] | null
  min?: number
  max?: number
  step?: number
  options?: string[]
  /** True for computed output params (e.g. math result) — display-only in Inspector, output port only */
  readonly?: boolean
  /** True for derived output-only ports (e.g. color components) — shown as right-side port handles but not as rows in the node body */
  portOnly?: boolean
  /** True for params that are user-controlled only (e.g. value node pickers) — no input port handle rendered */
  noPort?: boolean
}

export interface PortDefinition {
  type: 'image' | 'mask' | 'number' | 'path'
  label: string
}

/** A single show/hide rule for a param row in the Inspector. */
export interface VisibilityRule {
  /** Name of the param whose row this rule controls. */
  show: string
  /** The row is visible only when `params[when.param] === when.eq`. */
  when: { param: string; eq: unknown }
}

export interface NodeDefinition {
  id: string
  version?: string
  label: string
  description?: string
  category: string
  icon?: string
  inputs: PortDefinition[]
  outputs: PortDefinition[]
  params: ParamDefinition[]
  /** For simple nodes: ImageMagick command template with {{param}} placeholders */
  command_template?: string
  /** For complex nodes: key referencing a TypeScript executor class */
  executor?: string
  /** Inline JS function body — receives `params`, must return `string[]` of IM args.
   *  Alternative to command_template for conditional argument building. */
  command_js?: string
  /** Inline JS function body — receives `params`, must return `Record<string, unknown>` of output values.
   *  Alternative to the executor switch for pure-value nodes. */
  compute_js?: string
  /** Set true to opt in to image metadata loading (replaces the implicit prop_ prefix heuristic). */
  needs_image_meta?: boolean
  /** Inspector-level show/hide rules evaluated against current param values. */
  params_visibility?: VisibilityRule[]
}

// ─── Node Graph ───────────────────────────────────────────────────────────────

/** Mirrors Svelte Flow's node shape */
export interface GraphNode {
  id: string
  type: string
  position: { x: number; y: number }
  width?: number
  height?: number
  parentId?: string    // set on nodes inside a group
  extent?: 'parent'   // constrains node within parent bounds
  data: { label: string; definitionId: string; params: Record<string, unknown> }
}

/** Mirrors Svelte Flow's edge shape */
export interface GraphEdge {
  id: string
  source: string
  sourceHandle?: string
  target: string
  targetHandle?: string
}

export interface GraphViewport {
  x: number
  y: number
  zoom: number
}

export interface NodeGraph {
  nodes: GraphNode[]
  edges: GraphEdge[]
  viewport: GraphViewport
}

// ─── Images ───────────────────────────────────────────────────────────────────

export interface ImageInfo {
  path: string
  name: string
  width: number
  height: number
  format: string
  sizeBytes: number
  thumbnailDataUrl?: string
}

// ─── Pipeline Service (abstraction for Electron IPC / future web) ─────────────

export interface Progress {
  completed: number
  total: number
  currentFile: string
}

export interface BatchResult {
  processed: number
  skipped:   number
  failed:    number
}

export interface PipelineService {
  loadImages(paths: string[]): Promise<ImageInfo[]>
  generateThumbnail(imagePath: string, size: number): Promise<string>
  executePreview(graph: NodeGraph, imagePath: string, fromNodeId?: string): Promise<string>
  executeBatch(
    graph: NodeGraph,
    imagePaths: string[],
    outputDir: string,
    onProgress: (p: Progress) => void
  ): Promise<void>
  exportCLI(graph: NodeGraph): string
}

