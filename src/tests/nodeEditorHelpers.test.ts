import { describe, it, expect } from 'vitest'
import {
  getNodeParams, nodeTypeForDef, buildNodeData, expandNodeData,
  sortNodesGroupFirst, buildResizeParamDefs, firstMatchingHandle,
} from '../renderer/nodeEditor/nodeEditorHelpers.js'
import type { NodeDefinition } from '../shared/types.js'

// ── Minimal NodeDefinition factory ───────────────────────────────────────────

function makeDef(overrides: Partial<NodeDefinition> = {}): NodeDefinition {
  return {
    id: 'test',
    label: 'Test',
    description: '',
    category: 'Test',
    executor: 'test_exec',
    inputs: [],
    outputs: [],
    params: [],
    ...overrides,
  }
}

// ── getNodeParams ─────────────────────────────────────────────────────────────

describe('getNodeParams', () => {
  it('returns params from a valid node data object', () => {
    const data = { params: { width: 100, height: 200 } }
    expect(getNodeParams(data)).toEqual({ width: 100, height: 200 })
  })

  it('returns {} for null', () => {
    expect(getNodeParams(null)).toEqual({})
  })

  it('returns {} for undefined', () => {
    expect(getNodeParams(undefined)).toEqual({})
  })

  it('returns {} for non-object', () => {
    expect(getNodeParams(42)).toEqual({})
    expect(getNodeParams('string')).toEqual({})
  })

  it('returns {} when params key is missing', () => {
    expect(getNodeParams({ label: 'foo' })).toEqual({})
  })

  it('returns {} when params is null', () => {
    expect(getNodeParams({ params: null })).toEqual({})
  })

  it('returns {} when params is a primitive', () => {
    expect(getNodeParams({ params: 42 })).toEqual({})
  })
})

// ── nodeTypeForDef ─────────────────────────────────────────────────────────────

describe('nodeTypeForDef', () => {
  it('returns commentNode for id "comment"', () => {
    expect(nodeTypeForDef(makeDef({ id: 'comment' }))).toBe('commentNode')
  })

  it('returns folderPathNode for id "folderpath"', () => {
    expect(nodeTypeForDef(makeDef({ id: 'folderpath' }))).toBe('folderPathNode')
  })

  it('returns "process" for any other id', () => {
    expect(nodeTypeForDef(makeDef({ id: 'blur' }))).toBe('process')
    expect(nodeTypeForDef(makeDef({ id: 'resize' }))).toBe('process')
    expect(nodeTypeForDef(makeDef({ id: 'rename' }))).toBe('process')
  })
})

// ── buildNodeData ─────────────────────────────────────────────────────────────

describe('buildNodeData', () => {
  it('sets label and definitionId from def', () => {
    const def = makeDef({ id: 'blur', label: 'Blur' })
    const data = buildNodeData(def)
    expect(data.label).toBe('Blur')
    expect(data.definitionId).toBe('blur')
  })

  it('seeds params from def defaults', () => {
    const def = makeDef({
      params: [
        { name: 'sigma', type: 'float', label: 'Sigma', default: 2.5, widget: 'slider' },
      ],
    })
    const data = buildNodeData(def)
    expect(data.params.sigma).toBe(2.5)
  })

  it('sets _enabled=true for image-in + image-out nodes', () => {
    const def = makeDef({
      inputs: [{ type: 'image', label: 'In' }],
      outputs: [{ type: 'image', label: 'Out' }],
    })
    const data = buildNodeData(def)
    expect(data.params._enabled).toBe(true)
  })

  it('does not set _enabled for nodes without image IO', () => {
    const def = makeDef({
      inputs: [{ type: 'number', label: 'A' }],
      outputs: [{ type: 'number', label: 'Result' }],
    })
    const data = buildNodeData(def)
    expect(data.params._enabled).toBeUndefined()
  })

  it('seeds heading and body for comment nodes', () => {
    const def = makeDef({ id: 'comment', label: 'Comment' })
    const data = buildNodeData(def)
    expect(data.params.heading).toBe('Comment')
    expect(data.params.body).toBe('')
  })

  it('trims inputs to channels default when channels param is present', () => {
    const def = makeDef({
      inputs: [
        { type: 'number', label: 'A' },
        { type: 'number', label: 'B' },
        { type: 'number', label: 'C' },
        { type: 'number', label: 'D' },
      ],
      outputs: [{ type: 'number', label: 'Out' }],
      params: [{ name: 'channels', type: 'int', label: 'Channels', default: 2, widget: 'number' }],
    })
    const data = buildNodeData(def)
    expect(data.inputs).toHaveLength(2)
    expect(data.inputs).toEqual(['number', 'number'])
  })

  it('excludes enum params from paramDefs', () => {
    const def = makeDef({
      params: [
        { name: 'mode', type: 'enum', label: 'Mode', default: 'a', widget: 'dropdown', options: ['a', 'b'] },
        { name: 'sigma', type: 'float', label: 'Sigma', default: 1, widget: 'slider' },
      ],
    })
    const data = buildNodeData(def)
    expect(data.paramDefs.some(p => p.name === 'mode')).toBe(false)
    expect(data.paramDefs.some(p => p.name === 'sigma')).toBe(true)
  })

  it('maps input/output types and labels correctly', () => {
    const def = makeDef({
      inputs: [
        { type: 'image', label: 'Source' },
        { type: 'mask', label: 'Mask' },
      ],
      outputs: [{ type: 'image', label: 'Result' }],
    })
    const data = buildNodeData(def)
    expect(data.inputs).toEqual(['image', 'mask'])
    expect(data.inputLabels).toEqual(['Source', 'Mask'])
    expect(data.outputs).toEqual(['image'])
    expect(data.outputLabels).toEqual(['Result'])
  })

  it('produces correct initial paramDefs for a resize node (absolute+preserve defaults)', () => {
    const def = makeDef({
      id: 'resize',
      inputs: [{ type: 'image', label: 'Input' }],
      outputs: [{ type: 'image', label: 'Output' }],
      params: [
        { name: 'mode', type: 'enum', label: 'Mode', default: 'absolute', widget: 'dropdown', options: ['absolute', 'relative'] },
        { name: 'width', type: 'int', label: 'Width', default: 1024, widget: 'slider' },
        { name: 'height', type: 'int', label: 'Height', default: 1024, widget: 'slider' },
        { name: 'preserve_aspect', type: 'bool', label: 'Preserve Aspect', default: true, widget: 'checkbox' },
      ],
    })
    const data = buildNodeData(def)
    const names = data.paramDefs.map((p) => p.name)
    // preserve=true absolute → only width (no height), plus preserve_aspect
    expect(names).toContain('preserve_aspect')
    expect(names).toContain('width')
    expect(names).not.toContain('height')
    expect(names).not.toContain('scale_width')
  })
})

// ── sortNodesGroupFirst ────────────────────────────────────────────────────────

describe('sortNodesGroupFirst', () => {
  it('moves group nodes to the front', () => {
    const nodes = [
      { id: 'a', type: 'process' },
      { id: 'g', type: 'group' },
      { id: 'b', type: 'process' },
    ]
    const sorted = sortNodesGroupFirst(nodes)
    expect(sorted[0].id).toBe('g')
  })

  it('preserves relative order of non-group nodes', () => {
    const nodes = [
      { id: 'a', type: 'process' },
      { id: 'b', type: 'process' },
      { id: 'c', type: 'process' },
    ]
    const sorted = sortNodesGroupFirst(nodes)
    expect(sorted.map((n) => n.id)).toEqual(['a', 'b', 'c'])
  })

  it('multiple groups all appear before non-groups', () => {
    const nodes = [
      { id: 'x', type: 'process' },
      { id: 'g1', type: 'group' },
      { id: 'y', type: 'process' },
      { id: 'g2', type: 'group' },
    ]
    const sorted = sortNodesGroupFirst(nodes)
    const types = sorted.map((n) => n.type)
    const firstNonGroup = types.indexOf('process')
    expect(types.slice(0, firstNonGroup).every((t) => t === 'group')).toBe(true)
  })

  it('returns empty array unchanged', () => {
    expect(sortNodesGroupFirst([])).toEqual([])
  })

  it('does not mutate the original array', () => {
    const nodes = [{ id: 'a', type: 'process' }, { id: 'g', type: 'group' }]
    sortNodesGroupFirst(nodes)
    expect(nodes[0].id).toBe('a')
  })
})

// ── buildResizeParamDefs ──────────────────────────────────────────────────────

describe('buildResizeParamDefs', () => {
  it('absolute + preserve: expose preserve_aspect and width only', () => {
    const defs = buildResizeParamDefs('absolute', true)
    const names = defs.map((p) => p.name)
    expect(names).toContain('preserve_aspect')
    expect(names).toContain('width')
    expect(names).not.toContain('height')
    expect(names).not.toContain('scale_width')
    expect(names).not.toContain('scale_height')
  })

  it('absolute + no preserve: expose preserve_aspect, width, and height', () => {
    const defs = buildResizeParamDefs('absolute', false)
    const names = defs.map((p) => p.name)
    expect(names).toContain('width')
    expect(names).toContain('height')
    expect(names).not.toContain('scale_width')
  })

  it('relative + preserve: expose preserve_aspect and scale_width only', () => {
    const defs = buildResizeParamDefs('relative', true)
    const names = defs.map((p) => p.name)
    expect(names).toContain('scale_width')
    expect(names).not.toContain('scale_height')
    expect(names).not.toContain('width')
    expect(names).not.toContain('height')
  })

  it('relative + no preserve: expose preserve_aspect, scale_width, and scale_height', () => {
    const defs = buildResizeParamDefs('relative', false)
    const names = defs.map((p) => p.name)
    expect(names).toContain('scale_width')
    expect(names).toContain('scale_height')
  })

  it('all params are writable (readonly=false)', () => {
    for (const combo of [
      ['absolute', true], ['absolute', false],
      ['relative', true], ['relative', false],
    ] as [string, boolean][]) {
      const defs = buildResizeParamDefs(combo[0], combo[1])
      expect(defs.every((p) => p.readonly === false)).toBe(true)
    }
  })
})

// ── expandNodeData ─────────────────────────────────────────────────────────────

describe('expandNodeData', () => {
  it('merges saved params over definition defaults', () => {
    const def = makeDef({
      params: [
        { name: 'sigma', type: 'float', label: 'Sigma', default: 2.0, widget: 'slider' },
      ],
    })
    const data = expandNodeData(def, { sigma: 5.0 })
    expect(data.params.sigma).toBe(5.0)
  })

  it('keeps default for params not in savedParams', () => {
    const def = makeDef({
      params: [
        { name: 'sigma', type: 'float', label: 'Sigma', default: 2.0, widget: 'slider' },
        { name: 'mode', type: 'enum', label: 'Mode', default: 'linear', widget: 'dropdown', options: ['linear'] },
      ],
    })
    const data = expandNodeData(def, {})
    expect(data.params.sigma).toBe(2.0)
  })

  it('re-derives inputs for channel param', () => {
    const def = makeDef({
      inputs: [
        { type: 'image', label: 'R' },
        { type: 'image', label: 'G' },
        { type: 'image', label: 'B' },
        { type: 'image', label: 'A' },
      ],
      params: [{ name: 'channels', type: 'int', label: 'Channels', default: 3, widget: 'number' }],
    })
    const data = expandNodeData(def, { channels: 2 })
    expect(data.inputs).toHaveLength(2)
  })

  it('re-derives resize paramDefs from saved mode+preserve', () => {
    const def = makeDef({
      id: 'resize',
      inputs: [{ type: 'image', label: 'In' }],
      outputs: [{ type: 'image', label: 'Out' }],
      params: [
        { name: 'mode', type: 'enum', label: 'Mode', default: 'absolute', widget: 'dropdown', options: ['absolute', 'relative'] },
        { name: 'width', type: 'int', label: 'Width', default: 1024, widget: 'slider' },
        { name: 'height', type: 'int', label: 'Height', default: 1024, widget: 'slider' },
        { name: 'scale_width', type: 'float', label: 'Scale W', default: 100, widget: 'slider' },
        { name: 'scale_height', type: 'float', label: 'Scale H', default: 100, widget: 'slider' },
        { name: 'preserve_aspect', type: 'bool', label: 'Preserve', default: true, widget: 'checkbox' },
      ],
    })
    const data = expandNodeData(def, { mode: 'relative', preserve_aspect: false })
    const names = data.paramDefs.map((p) => p.name)
    expect(names).toContain('scale_width')
    expect(names).toContain('scale_height')
    expect(names).not.toContain('width')
  })
})

// ── firstMatchingHandle ───────────────────────────────────────────────────────

describe('firstMatchingHandle', () => {
  const imgDef = makeDef({
    inputs: [{ type: 'image', label: 'In' }],
    outputs: [{ type: 'image', label: 'Out' }],
  })

  it('returns in-0 for image wire on input side', () => {
    expect(firstMatchingHandle(imgDef, 'image', 'in')).toBe('in-0')
  })

  it('returns out-0 for image wire on output side', () => {
    expect(firstMatchingHandle(imgDef, 'image', 'out')).toBe('out-0')
  })

  it('returns in-0 for mask wire', () => {
    expect(firstMatchingHandle(imgDef, 'mask', 'in')).toBe('in-0')
  })

  it('matches param port by type on input side', () => {
    const def = makeDef({
      params: [
        { name: 'sigma', type: 'float', label: 'Sigma', default: 1, widget: 'slider' },
      ],
    })
    expect(firstMatchingHandle(def, 'number', 'in')).toBe('param-in-sigma')
  })

  it('matches readonly param as output port', () => {
    const def = makeDef({
      params: [
        { name: 'result', type: 'bool', label: 'Result', default: false, widget: 'checkbox', readonly: true },
      ],
    })
    expect(firstMatchingHandle(def, 'boolean', 'out')).toBe('param-out-result')
  })

  it('returns null when no match exists', () => {
    expect(firstMatchingHandle(makeDef(), 'path', 'in')).toBeNull()
  })
})
