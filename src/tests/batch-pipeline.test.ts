import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { NodeGraph, NodeDefinition } from '../shared/types.js';
import { EXECUTOR } from '../shared/constants.js';
import type { NodeRegistry } from '../main/nodes/registry.js';

vi.mock('../main/pipeline/magick-spawn.js', () => ({
  spawnMagick: vi.fn().mockResolvedValue(undefined),
}));

vi.mock('../main/pipeline/thumbnail-service.js', () => ({
  TEMP_DIR: '/tmp/imgplex-test',
  shortHash: (s: string) => s.slice(-8).replace(/[^a-z0-9]/gi, 'x'),
}));

vi.mock('../main/pipeline/executor-compute.js', () => ({
  computeNodeParams: (_exec: unknown, params: Record<string, unknown>) => params,
  loadImageMeta: vi.fn(),
  loadImageMean: vi.fn(),
  loadImageChannelMean: vi.fn(),
  loadMultipleChannelMeans: vi.fn(),
  getSeparator: () => '\n',
  buildEmptyImageMeta: () => ({}),
}));

// Registry is used only via .get() in these tests
function makeRegistry(defs: Record<string, Partial<NodeDefinition>>): NodeRegistry {
  return {
    get: (id: string) => {
      const d = defs[id];
      if (!d) return undefined;
      return {
        label: id,
        category: 'test',
        inputs: [],
        outputs: [],
        params: [],
        ...d,
      } as NodeDefinition;
    },
    getAll: () => [],
  } as unknown as NodeRegistry;
}

function makeGraph(nodes: NodeGraph['nodes'], edges: NodeGraph['edges'] = []): NodeGraph {
  return { nodes, edges, viewport: { x: 0, y: 0, zoom: 1 } };
}

const noop = () => {};

describe('executeBatch', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('returns zero counts for an empty image list', async () => {
    const { executeBatch } = await import('../main/pipeline/batch-pipeline.js');
    const graph = makeGraph(
      [
        {
          id: 'workflow-input',
          type: 'workflow-input',
          position: { x: 0, y: 0 },
          data: { label: 'Input', definitionId: 'workflow-input', params: {} },
        },
        {
          id: 'workflow-output',
          type: 'workflow-output',
          position: { x: 0, y: 0 },
          data: { label: 'Output', definitionId: 'workflow-output', params: {} },
        },
      ],
      [{ id: 'e1', source: 'workflow-input', target: 'workflow-output', sourceHandle: 'out-0', targetHandle: 'in-0' }]
    );
    const registry = makeRegistry({});
    const result = await executeBatch(
      graph,
      'workflow-output',
      'workflow-input',
      [],
      null,
      'overwrite',
      registry,
      noop,
      () => false
    );
    expect(result.processed).toBe(0);
    expect(result.failed).toBe(0);
    expect(result.errors).toHaveLength(0);
  });

  it('stops early when isCancelled returns true', async () => {
    const { executeBatch } = await import('../main/pipeline/batch-pipeline.js');
    const graph = makeGraph(
      [
        {
          id: 'workflow-input',
          type: 'workflow-input',
          position: { x: 0, y: 0 },
          data: { label: 'Input', definitionId: 'workflow-input', params: {} },
        },
        {
          id: 'workflow-output',
          type: 'workflow-output',
          position: { x: 0, y: 0 },
          data: { label: 'Output', definitionId: 'workflow-output', params: {} },
        },
      ],
      [{ id: 'e1', source: 'workflow-input', target: 'workflow-output', sourceHandle: 'out-0', targetHandle: 'in-0' }]
    );
    const registry = makeRegistry({});
    const result = await executeBatch(
      graph,
      'workflow-output',
      'workflow-input',
      ['/img/a.png', '/img/b.png'],
      null,
      'overwrite',
      registry,
      noop,
      () => true // cancel immediately
    );
    expect(result.processed + result.skipped + result.failed).toBe(0);
  });

  it('counts gate-suppressed image as skipped', async () => {
    const { executeBatch } = await import('../main/pipeline/batch-pipeline.js');
    const graph = makeGraph(
      [
        {
          id: 'workflow-input',
          type: 'workflow-input',
          position: { x: 0, y: 0 },
          data: { label: 'Input', definitionId: 'workflow-input', params: {} },
        },
        {
          id: 'gate-1',
          type: 'gate',
          position: { x: 0, y: 0 },
          data: { label: 'Gate', definitionId: 'gate-def', params: { condition: false } },
        },
        {
          id: 'workflow-output',
          type: 'workflow-output',
          position: { x: 0, y: 0 },
          data: { label: 'Output', definitionId: 'workflow-output', params: {} },
        },
      ],
      [
        { id: 'e1', source: 'workflow-input', target: 'gate-1', sourceHandle: 'out-0', targetHandle: 'in-0' },
        { id: 'e2', source: 'gate-1', target: 'workflow-output', sourceHandle: 'out-0', targetHandle: 'in-0' },
      ]
    );
    const registry = makeRegistry({
      'gate-def': {
        executor: EXECUTOR.GATE,
        inputs: [{ label: 'In', type: 'image' }],
        outputs: [{ label: 'Out', type: 'image' }],
      },
    });
    const result = await executeBatch(
      graph,
      'workflow-output',
      'workflow-input',
      ['/img/a.png'],
      null,
      'overwrite',
      registry,
      noop,
      () => false
    );
    expect(result.skipped).toBe(1);
    expect(result.processed).toBe(0);
  });
});
