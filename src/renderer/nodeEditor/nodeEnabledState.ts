// ─── Shared bypass / enabled-state resolution ─────────────────────────────────
//
// Used by ProcessNode (visual), NodeEditor (connection validation), and
// Preview (preview target selection). Keep logic here; callers just pass
// their local nodes + edges arrays.

interface MinNode { id: string; data: unknown }
interface MinEdge { target: string; targetHandle?: string | null; source: string; sourceHandle?: string | null }

function nodeParams(data: unknown): Record<string, unknown> {
  if (typeof data !== 'object' || data === null) return {}
  const p = (data as Record<string, unknown>).params
  return typeof p === 'object' && p !== null ? p as Record<string, unknown> : {}
}

/**
 * Returns false when a node's _enabled param is toggled off or wired to a
 * falsy value. Defaults to true (enabled) when no bypass state is set.
 */
export function isNodeEffectivelyEnabled(
  nodeId: string,
  nodes: MinNode[],
  edges: MinEdge[],
): boolean {
  const node = nodes.find((n) => n.id === nodeId)
  if (!node) return true

  const bypassEdge = edges.find(
    (e) => e.target === nodeId && e.targetHandle === 'param-in-_enabled',
  )
  if (bypassEdge?.sourceHandle?.startsWith('param-out-')) {
    const srcParam = bypassEdge.sourceHandle.slice('param-out-'.length)
    const srcNode = nodes.find((n) => n.id === bypassEdge.source)
    const val = nodeParams(srcNode?.data)[srcParam]
    return val !== false && val !== 0 && val !== null && val !== undefined
  }

  const enabled = nodeParams(node.data)._enabled
  return enabled !== false && enabled !== 0
}
