import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { NodeGraph, GraphNode, GraphEdge, NodeDefinition } from '../shared/types.js';
import type { NodeRegistry } from '../main/nodes/registry.js';
import type { BatchContext } from '../main/pipeline/multistream-pipeline.js';
import { EXECUTOR } from '../shared/constants.js';

vi.mock('../main/pipeline/magick-spawn.js', () => ({
  spawnMagick: vi.fn().mockResolvedValue(undefined),
}));

vi.mock('../main/pipeline/thumbnail-service.js', () => ({
  TEMP_DIR: '/tmp/imgplex-test',
  shortHash: (_s: string) => 'testhash',
}));

vi.mock('../main/pipeline/executor-compute.js', () => ({
  computeNodeParams: (_exec: unknown, params: Record<string, unknown>) => params,
  loadImageMeta: vi.fn().mockResolvedValue({}),
  loadImageMean: vi.fn().mockResolvedValue(0.5),
  loadImageChannelMean: vi.fn().mockResolvedValue(0.5),
  loadMultipleChannelMeans: vi.fn().mockResolvedValue([0.5, 0.5, 0.5, 0.5]),
  buildEmptyImageMeta: vi.fn().mockResolvedValue({}),
}));

const INPUT_PATH = '/test/input.jpg';
const INPUT_ID = 'inp';
const OUTPUT_ID = 'out';

function makeNode(id: string, definitionId: string, params: Record<string, unknown> = {}): GraphNode {
  return {
    id,
    type: 'default',
    position: { x: 0, y: 0 },
    data: { label: id, definitionId, params },
  };
}

function makeEdge(source: string, target: string, sourceHandle = 'out-0', targetHandle = 'in-0'): GraphEdge {
  return { id: `${source}-${target}`, source, target, sourceHandle, targetHandle };
}

function makeRegistry(defs: Record<string, Partial<NodeDefinition>>): NodeRegistry {
  return {
    get: (id: string) => {
      const d = defs[id];
      if (!d) return undefined;
      return {
        id,
        label: id,
        category: 'test',
        inputs: [{ label: 'In', type: 'image' }],
        outputs: [{ label: 'Out', type: 'image' }],
        params: [],
        ...d,
      } as NodeDefinition;
    },
    getAll: () => [],
  } as unknown as NodeRegistry;
}

function makeGraph(nodes: GraphNode[], edges: GraphEdge[]): NodeGraph {
  return { nodes, edges, viewport: { x: 0, y: 0, zoom: 1 } };
}

function makeCtx(
  sorted: GraphNode[],
  graph: NodeGraph,
  registry: NodeRegistry,
  overrides: Partial<BatchContext> = {}
): BatchContext {
  const allIds = new Set(sorted.map((n) => n.id));
  allIds.add(INPUT_ID);
  allIds.add(OUTPUT_ID);
  return {
    inputNodeId: INPUT_ID,
    outputNodeId: OUTPUT_ID,
    sorted,
    outputContributorIds: allIds,
    registry,
    graph,
    hasHeavyMetaNodes: false,
    hasImageMetaNodes: false,
    hasImageOutput: true,
    outputDir: null,
    overwrite: 'overwrite',
    isCancelled: () => false,
    ...overrides,
  };
}

describe('executeMultiStream', () => {
  let spawnMagick: ReturnType<typeof vi.fn>;

  beforeEach(async () => {
    vi.clearAllMocks();
    const mod = await import('../main/pipeline/magick-spawn.js');
    spawnMagick = vi.mocked(mod.spawnMagick);
  });

  it('no output edge → returns inputPath without spawning magick', async () => {
    const { executeMultiStream } = await import('../main/pipeline/multistream-pipeline.js');
    const graph = makeGraph([makeNode(INPUT_ID, INPUT_ID), makeNode(OUTPUT_ID, OUTPUT_ID)], []);
    const ctx = makeCtx([], graph, makeRegistry({}));
    const result = await executeMultiStream(INPUT_PATH, 0, ctx);
    expect(result).not.toBeNull();
    expect(result!.resultPath).toBe(INPUT_PATH);
    expect(spawnMagick).not.toHaveBeenCalled();
  });

  it('single standard node → lazy chain materialised in one spawnMagick call', async () => {
    const { executeMultiStream } = await import('../main/pipeline/multistream-pipeline.js');
    const proc = makeNode('proc', 'negate');
    const edges = [
      makeEdge(INPUT_ID, 'proc'),
      makeEdge('proc', OUTPUT_ID),
    ];
    const graph = makeGraph([makeNode(INPUT_ID, INPUT_ID), proc, makeNode(OUTPUT_ID, OUTPUT_ID)], edges);
    const registry = makeRegistry({ negate: { command_template: '-negate' } });
    const ctx = makeCtx([proc], graph, registry);

    const result = await executeMultiStream(INPUT_PATH, 0, ctx);
    expect(result).not.toBeNull();
    expect(spawnMagick).toHaveBeenCalledOnce();
    // Args: [inputPath, '-negate', outputTmpPath]
    const callArgs = spawnMagick.mock.calls[0][0] as string[];
    expect(callArgs[0]).toBe(INPUT_PATH);
    expect(callArgs.slice(1, -1)).toEqual(['-negate']);
  });

  it('two consecutive nodes → command fusion into single spawnMagick call', async () => {
    const { executeMultiStream } = await import('../main/pipeline/multistream-pipeline.js');
    const n1 = makeNode('n1', 'negate');
    const n2 = makeNode('n2', 'flip');
    const edges = [
      makeEdge(INPUT_ID, 'n1'),
      makeEdge('n1', 'n2'),
      makeEdge('n2', OUTPUT_ID),
    ];
    const graph = makeGraph([makeNode(INPUT_ID, INPUT_ID), n1, n2, makeNode(OUTPUT_ID, OUTPUT_ID)], edges);
    const registry = makeRegistry({
      negate: { command_template: '-negate' },
      flip: { command_template: '-flip' },
    });
    const ctx = makeCtx([n1, n2], graph, registry);

    const result = await executeMultiStream(INPUT_PATH, 0, ctx);
    expect(result).not.toBeNull();
    // Both ops fused → exactly ONE spawnMagick call with combined args
    expect(spawnMagick).toHaveBeenCalledOnce();
    const callArgs = spawnMagick.mock.calls[0][0] as string[];
    expect(callArgs[0]).toBe(INPUT_PATH);
    expect(callArgs.slice(1, -1)).toEqual(['-negate', '-flip']);
  });

  it('bypassed node (_enabled: false) → passes source through, no spawn', async () => {
    const { executeMultiStream } = await import('../main/pipeline/multistream-pipeline.js');
    const proc = makeNode('proc', 'negate', { _enabled: false });
    const edges = [makeEdge(INPUT_ID, 'proc'), makeEdge('proc', OUTPUT_ID)];
    const graph = makeGraph([makeNode(INPUT_ID, INPUT_ID), proc, makeNode(OUTPUT_ID, OUTPUT_ID)], edges);
    const registry = makeRegistry({ negate: { command_template: '-negate' } });
    const ctx = makeCtx([proc], graph, registry);

    const result = await executeMultiStream(INPUT_PATH, 0, ctx);
    expect(result).not.toBeNull();
    // Bypassed node inherits source → final val is inputPath string → no spawn needed
    expect(result!.resultPath).toBe(INPUT_PATH);
    expect(spawnMagick).not.toHaveBeenCalled();
  });

  it('gate with condition=false → returns null', async () => {
    const { executeMultiStream } = await import('../main/pipeline/multistream-pipeline.js');
    const gate = makeNode('gate', 'gate-def', { condition: false });
    const edges = [makeEdge(INPUT_ID, 'gate'), makeEdge('gate', OUTPUT_ID)];
    const graph = makeGraph([makeNode(INPUT_ID, INPUT_ID), gate, makeNode(OUTPUT_ID, OUTPUT_ID)], edges);
    const registry = makeRegistry({
      'gate-def': { executor: EXECUTOR.GATE },
    });
    const ctx = makeCtx([gate], graph, registry);

    const result = await executeMultiStream(INPUT_PATH, 0, ctx);
    expect(result).toBeNull();
  });

  it('format_convert → spawns once and changes outputExt to .png', async () => {
    const { executeMultiStream } = await import('../main/pipeline/multistream-pipeline.js');
    const conv = makeNode('conv', 'fmt-def', { format: 'PNG', quality: 90 });
    const edges = [makeEdge(INPUT_ID, 'conv'), makeEdge('conv', OUTPUT_ID)];
    const graph = makeGraph([makeNode(INPUT_ID, INPUT_ID), conv, makeNode(OUTPUT_ID, OUTPUT_ID)], edges);
    const registry = makeRegistry({
      'fmt-def': { executor: EXECUTOR.FORMAT_CONVERT },
    });
    const ctx = makeCtx([conv], graph, registry);

    const result = await executeMultiStream(INPUT_PATH, 0, ctx);
    expect(result).not.toBeNull();
    expect(result!.outputExt).toBe('.png');
    expect(spawnMagick).toHaveBeenCalledOnce();
    const callArgs = spawnMagick.mock.calls[0][0] as string[];
    expect(callArgs[0]).toBe(INPUT_PATH);
    expect(callArgs).toContain('-quality');
    expect(callArgs).toContain('90');
  });

  it('shared source (imgConsumers > 1) → both branches get the source, each produces its own chain', async () => {
    const { executeMultiStream } = await import('../main/pipeline/multistream-pipeline.js');
    // inp → n1 (negate) → out
    //     ↘ n2 (flip)      (n2 is in sorted but NOT wired to output)
    // inp:out-0 has 2 consumers: n1 and n2
    const n1 = makeNode('n1', 'negate');
    const n2 = makeNode('n2', 'flip');
    const edges = [
      makeEdge(INPUT_ID, 'n1'),
      makeEdge(INPUT_ID, 'n2'),
      makeEdge('n1', OUTPUT_ID),
    ];
    const graph = makeGraph([makeNode(INPUT_ID, INPUT_ID), n1, n2, makeNode(OUTPUT_ID, OUTPUT_ID)], edges);
    const registry = makeRegistry({
      negate: { command_template: '-negate' },
      flip: { command_template: '-flip' },
    });
    const ctx = makeCtx([n1, n2], graph, registry);

    const result = await executeMultiStream(INPUT_PATH, 0, ctx);
    expect(result).not.toBeNull();
    // n1 is sole contributor to output; spawnMagick called once for final materialisation
    expect(spawnMagick).toHaveBeenCalledOnce();
    const callArgs = spawnMagick.mock.calls[0][0] as string[];
    expect(callArgs[0]).toBe(INPUT_PATH);
    expect(callArgs.slice(1, -1)).toEqual(['-negate']);
  });

  it('no-op node (empty args) → inherits source slot without spawning', async () => {
    const { executeMultiStream } = await import('../main/pipeline/multistream-pipeline.js');
    // A node with a command_js that returns [] (pass-through)
    const proc = makeNode('proc', 'passthrough');
    const edges = [makeEdge(INPUT_ID, 'proc'), makeEdge('proc', OUTPUT_ID)];
    const graph = makeGraph([makeNode(INPUT_ID, INPUT_ID), proc, makeNode(OUTPUT_ID, OUTPUT_ID)], edges);
    const registry = makeRegistry({ passthrough: { command_js: 'return []' } });
    const ctx = makeCtx([proc], graph, registry);

    const result = await executeMultiStream(INPUT_PATH, 0, ctx);
    expect(result).not.toBeNull();
    expect(result!.resultPath).toBe(INPUT_PATH);
    expect(spawnMagick).not.toHaveBeenCalled();
  });
});
