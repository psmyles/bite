// Phase 0 reference generator. Deliberately calls the production TypeScript code.
// Regeneration is explicit: BITE_UPDATE_GOLDENS=1 npm run test:golden.
import { describe, it, expect } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import type { NodeDefinition, FormatDefinition, ParamDefinition, NodeGraph } from '../shared/types.js';
import {
  buildCommandArgs,
  buildCommandArgsFromJs,
  buildFormatConvertArgs,
  getFormatExtension,
} from '../main/pipeline/command-builder.js';
import { buildResizeArgs, computeNodeParamsUnsafe, type ImageMeta } from '../main/pipeline/executor-compute.js';
import { topoSort, findDescendants, findOutputContributors, groupBySetPattern } from '../main/pipeline/graph-utils.js';
import { traceInputNodeId } from '../shared/graphTrace.js';
import { computeNewName, type RenameParams } from '../shared/renameUtils.js';

const root = path.resolve('tests/golden');
const update = process.env.BITE_UPDATE_GOLDENS === '1';
function golden(file: string, value: unknown) {
  // Preserve nonfinite results explicitly instead of silently serializing as null.
  const encoded =
    JSON.stringify(value, (_key, v) => (typeof v === 'number' && !Number.isFinite(v) ? { $number: String(v) } : v), 2) +
    '\n';
  const target = path.join(root, file);
  if (update) {
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.writeFileSync(target, encoded);
  }
  expect(fs.readFileSync(target, 'utf8')).toBe(encoded);
}
function definitions<T>(dir: string): T[] {
  return fs
    .readdirSync(dir)
    .filter((f) => f.endsWith('.json'))
    .sort()
    .map((f) => JSON.parse(fs.readFileSync(path.join(dir, f), 'utf8')) as T);
}
function cases(params: ParamDefinition[]) {
  const defaults = Object.fromEntries(params.filter((p) => p.default !== undefined).map((p) => [p.name, p.default]));
  const result: { name: string; params: Record<string, unknown> }[] = [
    { name: 'defaults', params: defaults },
    { name: 'missing', params: {} },
  ];
  for (const p of params.filter((p) => !p.readonly)) {
    const values: unknown[] = p.options
      ? [...p.options, '__invalid__']
      : p.type === 'bool'
        ? [false, true]
        : ['int', 'float', 'numeric'].includes(p.type)
          ? [0, -1, 0.5, ...(p.min === undefined ? [] : [p.min]), ...(p.max === undefined ? [] : [p.max])]
          : p.type.startsWith('vector') || p.type === 'numeric'
            ? [
                [0, 0],
                [3, 0, 4],
              ]
            : p.type === 'string'
              ? ['', 'a b "quoted"']
              : [];
    if (p.type === 'numeric') values.push([0, 0], [3, 0, 4], [2]);
    for (const [i, value] of values.entries())
      result.push({ name: `${p.name}-${i}`, params: { ...defaults, [p.name]: value } });
  }
  // Interaction cases are essential: independently varying each parameter misses these.
  if ('preserve_aspect' in defaults)
    for (const mode of ['absolute', 'relative'])
      for (const preserve of [false, true])
        for (const anchor of ['width', 'height'])
          result.push({
            name: `${mode}-${preserve}-${anchor}`,
            params: {
              ...defaults,
              mode,
              preserve_aspect: preserve,
              anchor,
              width: 321,
              height: 123,
              scale_width: 50,
              scale_height: 75,
            },
          });
  if ('a' in defaults && 'b' in defaults)
    for (const [a, b] of [
      [[1, 2, 3], [4]],
      [2, [3, 4]],
      [[1, 0], 0],
    ])
      result.push({ name: `broadcast-${result.length}`, params: { ...defaults, a, b } });
  return result;
}
const meta: ImageMeta = {
  path: '/fixtures/photo.test.png',
  name: 'photo.test.png',
  sizeBytes: 4096,
  width: 128,
  height: 75,
  bitDepth: 16,
  extension: 'png',
  dpiX: 300,
  dpiY: 72,
  exif: {
    Make: 'TestCam',
    FNumber: '28/10',
    FocalLength: '50/1',
    ISOSpeedRatings: '200,0',
    DateTimeOriginal: '2020:01:02 03:04:05',
  },
};
const nodes = definitions<NodeDefinition>('node-definitions');
const formats = definitions<FormatDefinition>('format-definitions');
describe('migration reference goldens', () => {
  for (const def of nodes)
    it(`node ${def.id}`, () => {
      const pure = /^(math_|logic_|vec_math_|split_vec$|append_vec$|value_|prop_|text_filter$)/.test(
        def.executor ?? ''
      );
      const samples = cases(def.params).flatMap((c) =>
        pure && def.executor?.startsWith('prop_')
          ? [
              { ...c, meta },
              { ...c, name: c.name + '-no-meta', meta: undefined },
            ]
          : [{ ...c, meta: undefined }]
      );
      golden(`nodes/${def.id}.json`, {
        definition: def,
        cases: samples.map((c) => {
          try {
            const result = pure
              ? { expected_params: computeNodeParamsUnsafe(def.executor, c.params, c.meta) }
              : def.executor === 'resize'
                ? { expected_args: buildResizeArgs(c.params) }
                : def.command_js
                  ? { expected_args: buildCommandArgsFromJs(def, c.params) }
                  : def.command_template
                    ? { expected_args: buildCommandArgs(def, c.params) }
                    : { native_executor: def.executor, expected_args: [] };
            return { ...c, ...result };
          } catch (e) {
            return { ...c, expected_error: String(e) };
          }
        }),
      });
    });
  for (const def of formats)
    it(`format ${def.id}`, () =>
      golden(`formats/${def.id.toLowerCase()}.json`, {
        definition: def,
        cases: [...cases(def.params), { name: 'legacy-quality', params: { quality: 42 } }].map((c) => ({
          ...c,
          expected_args: buildFormatConvertArgs(def.id, c.params),
          expected_extension: getFormatExtension(def.id),
        })),
      }));
  for (const file of fs
    .readdirSync('test-workflows')
    .filter((f) => f.endsWith('.bite'))
    .sort())
    it(`workflow ${file}`, () => {
      const source = fs.readFileSync(path.join('test-workflows', file), 'utf8');
      const graph = JSON.parse(source).graph as NodeGraph;
      const outputs = graph.nodes.filter((n) => /^(imageOutputNode|textOutputNode|flipbookOutputNode)$/.test(n.type));
      golden(`workflows/${file.replace('.bite', '.json')}`, {
        source_sha256: createHash('sha256').update(source.replace(/\r\n/g, '\n')).digest('hex'),
        nodes: graph.nodes.length,
        edges: graph.edges.length,
        execution_order: topoSort(graph.nodes, graph.edges).map((n) => n.id),
        outputs: outputs.map((n) => ({
          id: n.id,
          input: traceInputNodeId(graph.nodes, graph.edges, n.id),
          contributors: [...findOutputContributors(graph.edges, [n.id])],
        })),
        descendants: graph.nodes
          .filter((n) => n.type === 'inputNode')
          .map((n) => ({ id: n.id, nodes: [...findDescendants(graph.edges, [n.id])] })),
      });
    });
  it('rename and set grouping', () => {
    const params: RenameParams = {
      blocks: [
        { type: 'text', value: 'test_' },
        { type: 'number', start: 1, pad: 3 },
        { type: 'oldname', find: 'old', replace_with: 'new' },
      ],
    };
    golden('naming.json', {
      params,
      cases: ['old.png', '.hidden', 'photo.old.jpg', 'no-extension'].map((name, index) => ({
        name,
        index,
        expected: computeNewName(name, params, index),
      })),
      sets: [
        ...groupBySetPattern(
          ['set_alpha_diffuse.png', 'set_alpha_normal.png', 'set_beta_diffuse.png', 'unmatched.png'],
          'set_',
          ['_diffuse', '_normal']
        ),
      ],
    });
  });
});
