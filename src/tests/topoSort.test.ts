import { describe, it, expect } from 'vitest'
import { topoSort } from '../main/pipeline/executor.js'
import type { GraphNode, GraphEdge } from '../shared/types.js'

// ── Helpers ───────────────────────────────────────────────────────────────────

function node(id: string): GraphNode {
  return {
    id,
    type: 'process',
    position: { x: 0, y: 0 },
    data: { label: id, definitionId: id, params: {} },
  }
}

function edge(source: string, target: string): GraphEdge {
  return { id: `${source}->${target}`, source, target }
}

// ── topoSort ─────────────────────────────────────────────────────────────────

describe('topoSort', () => {
  it('single node with no edges', () => {
    const result = topoSort([node('a')], [])
    expect(result.map(n => n.id)).toEqual(['a'])
  })

  it('two nodes with no edges (preserves insertion order among roots)', () => {
    const result = topoSort([node('a'), node('b')], [])
    expect(result).toHaveLength(2)
    expect(result.map(n => n.id)).toContain('a')
    expect(result.map(n => n.id)).toContain('b')
  })

  it('linear chain a → b → c', () => {
    const nodes = [node('a'), node('b'), node('c')]
    const edges = [edge('a', 'b'), edge('b', 'c')]
    const result = topoSort(nodes, edges)
    const ids = result.map(n => n.id)
    expect(ids.indexOf('a')).toBeLessThan(ids.indexOf('b'))
    expect(ids.indexOf('b')).toBeLessThan(ids.indexOf('c'))
  })

  it('diamond: a → b, a → c, b → d, c → d', () => {
    const nodes = [node('a'), node('b'), node('c'), node('d')]
    const edges = [edge('a', 'b'), edge('a', 'c'), edge('b', 'd'), edge('c', 'd')]
    const result = topoSort(nodes, edges)
    const ids = result.map(n => n.id)
    expect(ids.indexOf('a')).toBeLessThan(ids.indexOf('b'))
    expect(ids.indexOf('a')).toBeLessThan(ids.indexOf('c'))
    expect(ids.indexOf('b')).toBeLessThan(ids.indexOf('d'))
    expect(ids.indexOf('c')).toBeLessThan(ids.indexOf('d'))
  })

  it('two independent chains', () => {
    const nodes = [node('a1'), node('a2'), node('b1'), node('b2')]
    const edges = [edge('a1', 'a2'), edge('b1', 'b2')]
    const result = topoSort(nodes, edges)
    const ids = result.map(n => n.id)
    expect(ids.indexOf('a1')).toBeLessThan(ids.indexOf('a2'))
    expect(ids.indexOf('b1')).toBeLessThan(ids.indexOf('b2'))
    expect(result).toHaveLength(4)
  })

  it('returns all nodes', () => {
    const nodes = [node('x'), node('y'), node('z')]
    const edges = [edge('x', 'z'), edge('y', 'z')]
    const result = topoSort(nodes, edges)
    expect(result).toHaveLength(3)
  })

  it('cycle: still returns nodes (Kahn omits cycle members from result)', () => {
    // Kahn's algorithm: cycle nodes never reach degree 0, so they're excluded from sorted
    const nodes = [node('a'), node('b'), node('c')]
    const edges = [edge('a', 'b'), edge('b', 'c'), edge('c', 'b')] // b-c cycle
    const result = topoSort(nodes, edges)
    // 'a' has in-degree 0 and will be included; b and c are cycled so excluded
    expect(result.map(n => n.id)).toContain('a')
    expect(result.map(n => n.id)).not.toContain('b')
    expect(result.map(n => n.id)).not.toContain('c')
  })

  it('empty graph', () => {
    expect(topoSort([], [])).toEqual([])
  })

  it('typical pipeline: input → resize → output', () => {
    const nodes = [node('input'), node('resize'), node('output')]
    const edges = [edge('input', 'resize'), edge('resize', 'output')]
    const result = topoSort(nodes, edges)
    const ids = result.map(n => n.id)
    expect(ids[0]).toBe('input')
    expect(ids[1]).toBe('resize')
    expect(ids[2]).toBe('output')
  })

  it('node not referenced by any edge still appears', () => {
    const nodes = [node('a'), node('orphan'), node('b')]
    const edges = [edge('a', 'b')]
    const result = topoSort(nodes, edges)
    expect(result.map(n => n.id)).toContain('orphan')
    expect(result).toHaveLength(3)
  })
})
