import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { NodeGraph, NodeDefinition } from '../shared/types.js';
import type { NodeRegistry } from '../main/nodes/registry.js';

// Dispatch tests for executeBatch with textOutputNode / flipbookOutputNode
// targets. fs and magick are mocked; executor-compute is real so prop_name
// values and gate conditions resolve through the genuine compute path.
const { spawnMagickMock, mkdirMock, accessMock, writeFileMock, statMock } = vi.hoisted(() => ({
  spawnMagickMock: vi.fn(),
  mkdirMock: vi.fn(),
  accessMock: vi.fn(),
  writeFileMock: vi.fn(),
  statMock: vi.fn(),
}));

vi.mock('../main/pipeline/magick-spawn.js', () => ({
  spawnMagick: spawnMagickMock,
}));

vi.mock('node:fs', () => ({
  default: {
    promises: { mkdir: mkdirMock, access: accessMock, writeFile: writeFileMock, stat: statMock },
  },
}));

vi.mock('../main/pipeline/thumbnail-service.js', () => ({
  TEMP_DIR: '/tmp/imgplex-test',
  shortHash: (s: string) => s.slice(-8).replace(/[^a-z0-9]/gi, 'x'),
}));

function makeRegistry(defs: Record<string, Partial<NodeDefinition>>): NodeRegistry {
  return {
    get: (id: string) => {
      const d = defs[id];
      if (!d) return undefined;
      return { label: id, category: 'test', inputs: [], outputs: [], params: [], ...d } as NodeDefinition;
    },
    getAll: () => [],
  } as unknown as NodeRegistry;
}

function node(
  id: string,
  type: string,
  definitionId: string,
  params: Record<string, unknown> = {}
): NodeGraph['nodes'][number] {
  return { id, type, position: { x: 0, y: 0 }, data: { label: id, definitionId, params } };
}

function makeGraph(nodes: NodeGraph['nodes'], edges: NodeGraph['edges']): NodeGraph {
  return { nodes, edges, viewport: { x: 0, y: 0, zoom: 1 } };
}

const noop = () => {};
const GATE_DEF: Partial<NodeDefinition> = {
  executor: 'gate',
  inputs: [{ label: 'Input', type: 'image' }],
  outputs: [{ label: 'Output', type: 'image' }],
};
const PROP_NAME_DEF: Partial<NodeDefinition> = { executor: 'prop_name', needs_image_meta: true };

async function run(
  graph: NodeGraph,
  outputNodeId: string,
  registry: NodeRegistry,
  imagePaths: string[],
  overwrite: 'skip' | 'overwrite' = 'overwrite'
) {
  const { executeBatch } = await import('../main/pipeline/batch-pipeline.js');
  return executeBatch(graph, outputNodeId, 'input-1', imagePaths, null, overwrite, registry, noop, () => false);
}

describe('executeBatch — textOutputNode dispatch', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    spawnMagickMock.mockResolvedValue(undefined);
    mkdirMock.mockResolvedValue(undefined);
    accessMock.mockRejectedValue(new Error('ENOENT')); // output file does not exist by default
    writeFileMock.mockResolvedValue(undefined);
    statMock.mockRejectedValue(new Error('ENOENT'));
  });

  function textGraph(gateCondition?: boolean): NodeGraph {
    const nodes = [
      node('input-1', 'inputNode', ''),
      node('name-1', 'process', 'prop_name', { value: '', strip_extension: false }),
      node('text-1', 'textOutputNode', '', {
        outputPath: '/out/list.txt',
        portIds: ['txo-0', 'txo-1'],
        separatorType: 'comma',
      }),
    ];
    const edges = [
      {
        id: 'e-img',
        source: 'input-1',
        sourceHandle: 'out-0',
        target: 'text-1',
        targetHandle: 'in-0',
      },
      {
        id: 'e-name',
        source: 'name-1',
        sourceHandle: 'param-out-value',
        target: 'text-1',
        targetHandle: 'txo-0',
      },
    ];
    if (gateCondition !== undefined) {
      nodes.push(node('gate-1', 'process', 'gate', { condition: gateCondition }));
      edges[0] = { id: 'e-img', source: 'gate-1', sourceHandle: 'out-0', target: 'text-1', targetHandle: 'in-0' };
      edges.push({ id: 'e-in-gate', source: 'input-1', sourceHandle: 'out-0', target: 'gate-1', targetHandle: 'in-0' });
    }
    return makeGraph(nodes, edges);
  }

  const registry = () => makeRegistry({ prop_name: PROP_NAME_DEF, gate: GATE_DEF });

  it('writes one line per image to the configured file', async () => {
    const result = await run(textGraph(), 'text-1', registry(), ['/img/a.png', '/img/b.png']);

    expect(result.processed).toBe(2);
    expect(result.failed).toBe(0);
    expect(result.outputFiles).toEqual(['/out/list.txt']);
    expect(writeFileMock).toHaveBeenCalledTimes(1);
    expect(writeFileMock.mock.calls[0][0]).toBe('/out/list.txt');
    expect(writeFileMock.mock.calls[0][1]).toBe('a.png\nb.png\n');
    expect(spawnMagickMock).not.toHaveBeenCalled(); // no image files written for a text run
  });

  it('writes no file when an upstream gate blocks every image', async () => {
    const result = await run(textGraph(false), 'text-1', registry(), ['/img/a.png', '/img/b.png']);

    expect(result.processed).toBe(0);
    expect(result.skipped).toBe(2);
    expect(writeFileMock).not.toHaveBeenCalled();
  });

  it('still writes lines when an upstream gate passes', async () => {
    const result = await run(textGraph(true), 'text-1', registry(), ['/img/a.png']);

    expect(result.processed).toBe(1);
    expect(writeFileMock.mock.calls[0][1]).toBe('a.png\n');
  });

  it('skips entirely when the output file exists and overwrite=skip', async () => {
    accessMock.mockResolvedValue(undefined); // file exists
    const result = await run(textGraph(), 'text-1', registry(), ['/img/a.png', '/img/b.png'], 'skip');

    expect(result.processed).toBe(0);
    expect(result.skipped).toBe(2);
    expect(writeFileMock).not.toHaveBeenCalled();
  });
});

describe('executeBatch — flipbookOutputNode dispatch', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    spawnMagickMock.mockResolvedValue(undefined);
    accessMock.mockRejectedValue(new Error('ENOENT'));
  });

  function flipbookGraph(params: Record<string, unknown> = {}): NodeGraph {
    return makeGraph(
      [
        node('input-1', 'inputNode', ''),
        node('fb-1', 'flipbookOutputNode', '', {
          flipbookOutputPath: '/out/atlas.png',
          rows: 2,
          cols: 2,
          cellWidth: 64,
          cellHeight: 64,
          sortBy: 'name',
          ...params,
        }),
      ],
      [{ id: 'e-img', source: 'input-1', sourceHandle: 'out-0', target: 'fb-1', targetHandle: 'in-0' }]
    );
  }

  it('montages the images into the atlas, truncated to grid capacity', async () => {
    const images = ['/img/c.png', '/img/a.png', '/img/e.png', '/img/b.png', '/img/d.png'];
    const result = await run(flipbookGraph(), 'fb-1', makeRegistry({}), images);

    expect(result.processed).toBe(4); // 2×2 grid
    expect(result.skipped).toBe(1);
    expect(result.outputFiles).toEqual(['/out/atlas.png']);
    const args = spawnMagickMock.mock.calls[0][0] as string[];
    expect(args[0]).toBe('montage');
    // sorted by name, truncated to the first 4
    expect(args.slice(1, 5)).toEqual(['/img/a.png', '/img/b.png', '/img/c.png', '/img/d.png']);
    expect(args).toContain('-tile');
    expect(args[args.indexOf('-tile') + 1]).toBe('2x2');
    expect(args[args.length - 1]).toBe('/out/atlas.png');
  });

  it('skips when the atlas file exists and overwrite=skip', async () => {
    accessMock.mockResolvedValue(undefined); // file exists
    const result = await run(flipbookGraph(), 'fb-1', makeRegistry({}), ['/img/a.png'], 'skip');

    expect(result.processed).toBe(0);
    expect(result.skipped).toBe(1);
    expect(spawnMagickMock).not.toHaveBeenCalled();
  });
});
