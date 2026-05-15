import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { NodeGraph, NodeDefinition } from '../shared/types.js';
import { EXECUTOR } from '../shared/constants.js';
import type { NodeRegistry } from '../main/nodes/registry.js';
import { PreviewCache } from '../main/pipeline/cache.js';

vi.mock('../main/pipeline/magick-spawn.js', () => ({
  spawnMagick: vi.fn().mockResolvedValue(undefined),
}));

vi.mock('../main/pipeline/thumbnail-service.js', () => ({
  TEMP_DIR: '/tmp/imgplex-test',
  shortHash: (s: string) => Buffer.from(s).toString('hex').slice(0, 8),
}));

vi.mock('../main/pipeline/image-header.js', () => ({
  fileToDataUrl: vi.fn().mockResolvedValue('data:image/png;base64,test'),
  readHeaderDimensions: vi.fn().mockResolvedValue(null),
}));

vi.mock('../main/pipeline/executor-compute.js', () => ({
  computeNodeParams: (_exec: unknown, params: Record<string, unknown>) => params,
  loadImageMeta: vi.fn(),
  loadImageMean: vi.fn(),
}));

const fsMockPromises = {
  mkdir: vi.fn().mockResolvedValue(undefined),
  access: vi.fn().mockRejectedValue(new Error('ENOENT')),
  copyFile: vi.fn().mockResolvedValue(undefined),
};

vi.mock('node:fs', () => ({
  default: { mkdirSync: vi.fn(), promises: fsMockPromises },
  mkdirSync: vi.fn(),
  promises: fsMockPromises,
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

function makeGraph(nodes: NodeGraph['nodes'], edges: NodeGraph['edges'] = []): NodeGraph {
  return { nodes, edges, viewport: { x: 0, y: 0, zoom: 1 } };
}

describe('executePreview', () => {
  let spawnMagick: ReturnType<typeof vi.fn>;

  beforeEach(async () => {
    vi.clearAllMocks();
    // Restore implementations that clearAllMocks wipes
    fsMockPromises.mkdir.mockResolvedValue(undefined);
    fsMockPromises.access.mockRejectedValue(new Error('ENOENT'));
    fsMockPromises.copyFile.mockResolvedValue(undefined);
    const mod = await import('../main/pipeline/magick-spawn.js');
    spawnMagick = mod.spawnMagick as ReturnType<typeof vi.fn>;
    spawnMagick.mockResolvedValue(undefined);
  });

  it('empty graph returns data URL of downscaled input', async () => {
    const { executePreview } = await import('../main/pipeline/preview-pipeline.js');
    const graph = makeGraph([]);
    const registry = makeRegistry({});
    const result = await executePreview(new PreviewCache(), graph, '/img/test.png', registry);

    expect(result.dataUrl).toBe('data:image/png;base64,test');
    // downscale spawn was called once for the input
    expect(spawnMagick).toHaveBeenCalledOnce();
    expect(spawnMagick.mock.calls[0][0]).toContain('/img/test.png[0]');
  });

  it('format_convert node triggers image re-encode', async () => {
    const { executePreview } = await import('../main/pipeline/preview-pipeline.js');
    const graph = makeGraph(
      [
        {
          id: 'workflow-input',
          type: 'workflow-input',
          position: { x: 0, y: 0 },
          data: { label: 'Input', definitionId: 'workflow-input', params: {} },
        },
        {
          id: 'conv-1',
          type: 'conv',
          position: { x: 0, y: 0 },
          data: { label: 'Convert', definitionId: 'format-convert-def', params: { format: 'JPEG', quality: 85 } },
        },
        {
          id: 'workflow-output',
          type: 'workflow-output',
          position: { x: 0, y: 0 },
          data: { label: 'Output', definitionId: 'workflow-output', params: {} },
        },
      ],
      [
        { id: 'e1', source: 'workflow-input', target: 'conv-1', sourceHandle: 'out-0', targetHandle: 'in-0' },
        { id: 'e2', source: 'conv-1', target: 'workflow-output', sourceHandle: 'out-0', targetHandle: 'in-0' },
      ]
    );
    const registry = makeRegistry({
      'format-convert-def': {
        executor: EXECUTOR.FORMAT_CONVERT,
        inputs: [{ label: 'In', type: 'image' }],
        outputs: [{ label: 'Out', type: 'image' }],
      },
    });
    const result = await executePreview(new PreviewCache(), graph, '/img/test.png', registry);

    expect(result.dataUrl).toBe('data:image/png;base64,test');
    // Calls: 1 downscale + 1 format_convert node
    expect(spawnMagick).toHaveBeenCalledTimes(2);
    const nodeCall = spawnMagick.mock.calls[1][0] as string[];
    expect(nodeCall).toContain('-quality');
    expect(nodeCall).toContain('85');
  });

  it('bypassed node (_enabled=false) copies through without spawning magick', async () => {
    const { executePreview } = await import('../main/pipeline/preview-pipeline.js');
    const graph = makeGraph(
      [
        {
          id: 'workflow-input',
          type: 'workflow-input',
          position: { x: 0, y: 0 },
          data: { label: 'Input', definitionId: 'workflow-input', params: {} },
        },
        {
          id: 'resize-1',
          type: 'resize',
          position: { x: 0, y: 0 },
          data: { label: 'Resize', definitionId: 'resize-def', params: { _enabled: false } },
        },
        {
          id: 'workflow-output',
          type: 'workflow-output',
          position: { x: 0, y: 0 },
          data: { label: 'Output', definitionId: 'workflow-output', params: {} },
        },
      ],
      [
        { id: 'e1', source: 'workflow-input', target: 'resize-1', sourceHandle: 'out-0', targetHandle: 'in-0' },
        { id: 'e2', source: 'resize-1', target: 'workflow-output', sourceHandle: 'out-0', targetHandle: 'in-0' },
      ]
    );
    const registry = makeRegistry({
      'resize-def': {
        command_template: '-resize {{width}}x{{height}}>',
        inputs: [{ label: 'In', type: 'image' }],
        outputs: [{ label: 'Out', type: 'image' }],
      },
    });
    const result = await executePreview(new PreviewCache(), graph, '/img/test.png', registry);

    expect(result.dataUrl).toBe('data:image/png;base64,test');
    // Only the downscale spawn; the bypassed node uses copyFile
    expect(spawnMagick).toHaveBeenCalledOnce();
    expect(fsMockPromises.copyFile).toHaveBeenCalledOnce();
  });

  it('mean_value node result appears in propParams', async () => {
    const { executePreview } = await import('../main/pipeline/preview-pipeline.js');
    const { loadImageMean } = await import('../main/pipeline/executor-compute.js');
    (loadImageMean as ReturnType<typeof vi.fn>).mockResolvedValue(0.42);

    const graph = makeGraph(
      [
        {
          id: 'workflow-input',
          type: 'workflow-input',
          position: { x: 0, y: 0 },
          data: { label: 'Input', definitionId: 'workflow-input', params: {} },
        },
        {
          id: 'mean-1',
          type: 'mean',
          position: { x: 0, y: 0 },
          data: { label: 'Mean', definitionId: 'mean-def', params: {} },
        },
        {
          id: 'workflow-output',
          type: 'workflow-output',
          position: { x: 0, y: 0 },
          data: { label: 'Output', definitionId: 'workflow-output', params: {} },
        },
      ],
      [
        { id: 'e1', source: 'workflow-input', target: 'mean-1', sourceHandle: 'out-0', targetHandle: 'in-0' },
        { id: 'e2', source: 'workflow-input', target: 'workflow-output', sourceHandle: 'out-0', targetHandle: 'in-0' },
      ]
    );
    const registry = makeRegistry({
      'mean-def': {
        executor: EXECUTOR.MEAN_VALUE,
        inputs: [{ label: 'In', type: 'image' }],
        outputs: [{ label: 'Value', type: 'number' }],
      },
    });
    const result = await executePreview(new PreviewCache(), graph, '/img/test.png', registry);

    expect(result.propParams['mean-1']).toBeDefined();
    expect(result.propParams['mean-1'].value).toBe(0.42);
  });
});
