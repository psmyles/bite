import { describe, it, expect, vi, beforeEach } from 'vitest';
import path from 'node:path';
import type { NodeGraph, NodeDefinition } from '../shared/types.js';
import { EXECUTOR } from '../shared/constants.js';
import type { NodeRegistry } from '../main/nodes/registry.js';

// Depth tests for the per-image fast path. Unlike batch-pipeline.test.ts (which
// only exercises early-return paths that never touch the filesystem) these drive
// images all the way to an output write, so node:fs is mocked. Kept in a separate
// file so the fs mock stays scoped to its own module registry and doesn't affect
// the early-return tests.
const { spawnMagickMock, mkdirMock, accessMock, copyFileMock } = vi.hoisted(() => ({
  spawnMagickMock: vi.fn(),
  mkdirMock: vi.fn(),
  accessMock: vi.fn(),
  copyFileMock: vi.fn(),
}));

vi.mock('../main/pipeline/magick-spawn.js', () => ({
  spawnMagick: spawnMagickMock,
}));

vi.mock('node:fs', () => ({
  default: { promises: { mkdir: mkdirMock, access: accessMock, copyFile: copyFileMock } },
}));

vi.mock('../main/pipeline/thumbnail-service.js', () => ({
  TEMP_DIR: '/tmp/imgplex-test',
  shortHash: (s: string) => s.slice(-8).replace(/[^a-z0-9]/gi, 'x'),
}));

vi.mock('../main/pipeline/executor-compute.js', () => ({
  computeNodeParams: (_exec: unknown, params: Record<string, unknown>) => params,
  loadImageMeta: vi.fn(),
  buildEmptyImageMeta: () => ({}),
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

function node(id: string, definitionId: string, params: Record<string, unknown> = {}, type = definitionId) {
  return { id, type, position: { x: 0, y: 0 }, data: { label: id, definitionId, params } };
}

function edge(id: string, source: string, target: string) {
  return { id, source, target, sourceHandle: 'out-0', targetHandle: 'in-0' };
}

function makeGraph(nodes: NodeGraph['nodes'], edges: NodeGraph['edges'] = []): NodeGraph {
  return { nodes, edges, viewport: { x: 0, y: 0, zoom: 1 } };
}

const noop = () => {};
const IMAGE_IO: Partial<NodeDefinition> = {
  inputs: [{ label: 'In', type: 'image' }],
  outputs: [{ label: 'Out', type: 'image' }],
};

async function run(
  graph: NodeGraph,
  registry: NodeRegistry,
  imagePaths: string[],
  overwrite: 'skip' | 'overwrite' = 'overwrite',
  outputDir: string | null = '/out'
) {
  const { executeBatch } = await import('../main/pipeline/batch-pipeline.js');
  return executeBatch(
    graph,
    'workflow-output',
    'workflow-input',
    imagePaths,
    outputDir,
    overwrite,
    registry,
    noop,
    () => false
  );
}

describe('executeBatch — fast path depth', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    spawnMagickMock.mockResolvedValue(undefined);
    mkdirMock.mockResolvedValue(undefined);
    accessMock.mockRejectedValue(new Error('ENOENT')); // file does not exist by default
    copyFileMock.mockResolvedValue(undefined);
  });

  it('runs a single magick invocation and records the output file', async () => {
    const graph = makeGraph(
      [
        node('workflow-input', 'workflow-input'),
        node('gray', 'gray-def', {}, 'gray'),
        node('workflow-output', 'workflow-output'),
      ],
      [edge('e1', 'workflow-input', 'gray'), edge('e2', 'gray', 'workflow-output')]
    );
    const registry = makeRegistry({ 'gray-def': { ...IMAGE_IO, command_template: '-colorspace Gray' } });

    const result = await run(graph, registry, ['/img/a.png']);

    expect(result.processed).toBe(1);
    expect(result.failed).toBe(0);
    expect(result.outputFiles).toEqual([path.join('/out', 'a.png')]);
    expect(spawnMagickMock).toHaveBeenCalledTimes(1);
    const args = spawnMagickMock.mock.calls[0][0] as string[];
    expect(args).toEqual(['/img/a.png', '-colorspace', 'Gray', path.join('/out', 'a.png')]);
  });

  it('copies through (no magick spawn) when no operation contributes args', async () => {
    const graph = makeGraph(
      [node('workflow-input', 'workflow-input'), node('workflow-output', 'workflow-output')],
      [edge('e1', 'workflow-input', 'workflow-output')]
    );
    const result = await run(makeGraph(graph.nodes, graph.edges), makeRegistry({}), ['/img/a.png']);

    expect(result.processed).toBe(1);
    expect(spawnMagickMock).not.toHaveBeenCalled();
    expect(copyFileMock).toHaveBeenCalledWith('/img/a.png', path.join('/out', 'a.png'));
  });

  it('skips an image whose output already exists when overwrite=skip', async () => {
    accessMock.mockResolvedValue(undefined); // file exists
    const graph = makeGraph(
      [
        node('workflow-input', 'workflow-input'),
        node('gray', 'gray-def', {}, 'gray'),
        node('workflow-output', 'workflow-output'),
      ],
      [edge('e1', 'workflow-input', 'gray'), edge('e2', 'gray', 'workflow-output')]
    );
    const registry = makeRegistry({ 'gray-def': { ...IMAGE_IO, command_template: '-colorspace Gray' } });

    const result = await run(graph, registry, ['/img/a.png'], 'skip');

    expect(result.skipped).toBe(1);
    expect(result.processed).toBe(0);
    expect(spawnMagickMock).not.toHaveBeenCalled();
  });

  it('derives the output filename from a rename node (copy path)', async () => {
    const graph = makeGraph(
      [
        node('workflow-input', 'workflow-input'),
        node(
          'rn',
          'rename-def',
          {
            blocks: [
              { type: 'text', value: 'shot_' },
              { type: 'number', start: 1, pad: 3 },
            ],
          },
          'rn'
        ),
        node('workflow-output', 'workflow-output'),
      ],
      [edge('e1', 'workflow-input', 'rn'), edge('e2', 'rn', 'workflow-output')]
    );
    const registry = makeRegistry({ 'rename-def': { ...IMAGE_IO, executor: EXECUTOR.RENAME } });

    const result = await run(graph, registry, ['/img/a.png', '/img/b.png']);

    expect(result.processed).toBe(2);
    expect(copyFileMock.mock.calls.map((c) => c[1]).sort()).toEqual(
      [path.join('/out', 'shot_001.png'), path.join('/out', 'shot_002.png')].sort()
    );
  });

  it('uses the format extension and FORMAT:path syntax for a format_convert node', async () => {
    const graph = makeGraph(
      [
        node('workflow-input', 'workflow-input'),
        node('fc', 'fc-def', { format: 'JPEG' }, 'fc'),
        node('workflow-output', 'workflow-output'),
      ],
      [edge('e1', 'workflow-input', 'fc'), edge('e2', 'fc', 'workflow-output')]
    );
    const registry = makeRegistry({ 'fc-def': { ...IMAGE_IO, executor: EXECUTOR.FORMAT_CONVERT } });

    const result = await run(graph, registry, ['/img/a.png']);

    expect(result.processed).toBe(1);
    const outPath = path.join('/out', 'a.jpg');
    expect(result.outputFiles).toEqual([outPath]);
    const args = spawnMagickMock.mock.calls[0][0] as string[];
    expect(args[args.length - 1]).toBe(`JPEG:${outPath}`);
  });

  it('accumulates a per-image error without aborting the batch', async () => {
    spawnMagickMock.mockRejectedValueOnce(new Error('boom')).mockResolvedValueOnce(undefined);
    const graph = makeGraph(
      [
        node('workflow-input', 'workflow-input'),
        node('gray', 'gray-def', {}, 'gray'),
        node('workflow-output', 'workflow-output'),
      ],
      [edge('e1', 'workflow-input', 'gray'), edge('e2', 'gray', 'workflow-output')]
    );
    const registry = makeRegistry({ 'gray-def': { ...IMAGE_IO, command_template: '-colorspace Gray' } });

    const result = await run(graph, registry, ['/img/a.png', '/img/b.png']);

    expect(result.failed).toBe(1);
    expect(result.processed).toBe(1);
    expect(result.errors).toHaveLength(1);
    expect(result.errors[0]).toContain('boom');
  });

  it('counts images as processed without writing when there is no image output', async () => {
    // grayscale node connected to input but NOT wired into the output node's in-0
    const graph = makeGraph(
      [
        node('workflow-input', 'workflow-input'),
        node('gray', 'gray-def', {}, 'gray'),
        node('workflow-output', 'workflow-output'),
      ],
      [edge('e1', 'workflow-input', 'gray')]
    );
    const registry = makeRegistry({ 'gray-def': { ...IMAGE_IO, command_template: '-colorspace Gray' } });

    const result = await run(graph, registry, ['/img/a.png']);

    expect(result.processed).toBe(1);
    expect(result.outputFiles).toHaveLength(0);
    expect(spawnMagickMock).not.toHaveBeenCalled();
    expect(copyFileMock).not.toHaveBeenCalled();
  });
});
