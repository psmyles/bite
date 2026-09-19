// Differential characterization outside the fixed Phase 0 goldens. Each backend
// gets a private workflow copy/output directory; source fixtures stay unchanged.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import assert from 'node:assert/strict';

const root = path.dirname(fileURLToPath(import.meta.url));
const repo = path.dirname(root);
fs.mkdirSync(path.join(root, 'out'), { recursive: true });
const run = fs.mkdtempSync(path.join(root, 'out', 'compat-'));
const rust =
  process.env.BITE_TEST_CLI || path.join(repo, 'target/debug', process.platform === 'win32' ? 'bite.exe' : 'bite');
const magick = process.env.BITE_MAGICK || 'magick';
function invoke(exe, args) {
  const r = spawnSync(exe, args, { cwd: repo, encoding: 'utf8', timeout: 120000 });
  if (r.error) throw r.error;
  return r;
}
function image(dir, name, color) {
  fs.mkdirSync(dir, { recursive: true });
  const r = invoke(magick, ['-size', '32x32', `xc:${color}`, path.join(dir, name)]);
  assert.equal(r.status, 0, r.stderr);
}
function allFiles(dir, relative = '') {
  return fs
    .readdirSync(path.join(dir, relative))
    .flatMap((name) => {
      const entry = path.join(relative, name);
      return fs.statSync(path.join(dir, entry)).isDirectory() ? allFiles(dir, entry) : [entry];
    })
    .sort();
}
function compare(name, fixture, inputs, edit = () => {}, unorderedPixels = false, expectedError = '') {
  const outputs = [];
  for (const backend of ['node', 'rust']) {
    const out = path.join(run, name, backend);
    fs.mkdirSync(out, { recursive: true });
    const doc = JSON.parse(fs.readFileSync(path.join(root, fixture), 'utf8'));
    edit(doc);
    const workflow = path.join(run, `${name}-${backend}.bite`);
    const args = ['run', workflow];
    for (const [flag, dir] of Object.entries(inputs)) args.push(`--${flag}`, dir);
    for (const n of doc.graph.nodes) {
      const p = n.data.params;
      if (n.type === 'imageOutputNode') {
        p.outputPath = 'custom';
        p.customPath = path.join(out, p.cliName);
        args.push(`--${p.cliName}`, p.customPath);
      } else if (n.type === 'flipbookOutputNode') {
        p.flipbookOutputPath = path.join(out, 'atlas.png');
        args.push(`--${p.cliName}`, p.flipbookOutputPath);
      }
    }
    fs.writeFileSync(workflow, JSON.stringify(doc));
    const r =
      backend === 'node'
        ? invoke(process.execPath, [path.join(repo, 'dist-cli/cli-bundle.js'), ...args])
        : invoke(rust, args);
    assert.equal(r.status, 0, `${name}/${backend}: ${r.stderr}\n${r.stdout}`);
    if (expectedError) {
      assert.ok(r.stderr.includes(expectedError), `${name}/${backend}: missing failed-file diagnostic`);
      if (backend === 'rust') assert.match(r.stdout, /1 failed/);
    }
    outputs.push(out);
  }
  const names = allFiles(outputs[0]);
  assert.ok(names.length > 0, `${name}: reference must produce outputs`);
  assert.deepEqual(allFiles(outputs[1]), names, `${name}: output names`);
  const unmatched = new Set(names);
  for (const file of names) {
    if (unorderedPixels) {
      // The legacy concurrent workers claim collision suffixes after async I/O,
      // so input-to-suffix assignment varies. Require the exact same image set.
      const match = [...unmatched].find(
        (candidate) =>
          invoke(magick, [
            'compare',
            '-metric',
            'AE',
            path.join(outputs[0], candidate),
            path.join(outputs[1], file),
            'null:',
          ]).status === 0
      );
      assert.ok(match, `${name}/${file}: no identical unmatched reference image`);
      unmatched.delete(match);
      continue;
    }
    const r = invoke(magick, [
      'compare',
      '-metric',
      'AE',
      path.join(outputs[0], file),
      path.join(outputs[1], file),
      'null:',
    ]);
    assert.equal(r.status, 0, `${name}/${file}: ${r.stderr}`);
  }
  console.log(`PASS ${name} (${names.length} outputs, exact pixels)`);
  return { name, outputs: names, passed: true };
}
const partial = path.join(run, 'partial');
image(partial, 'set_alpha_diffuse.png', 'gray(200)');
image(partial, 'set_alpha_normal.png', 'gray(100)');
image(partial, 'set_beta_rough.png', 'gray(90)');
const atlas = path.join(run, 'atlas');
image(atlas, 'frame1.png', 'red');
image(atlas, 'frame2.png', 'green');
image(atlas, 'frame10.png', 'blue');
const corrupt = path.join(run, 'corrupt');
image(corrupt, 'z-good.png', 'red');
fs.writeFileSync(path.join(corrupt, 'a-bad.png'), 'not an image');
const cases = [
  ['partial-image-failure', 'wf-01-fastpath.bite', { in: corrupt }, () => {}, false, 'a-bad.png'],
  [
    'shared-merge-inputs',
    'wf-04-channels.bite',
    { in: atlas, second: partial },
    (doc) => {
      const first = doc.graph.nodes.find((n) => n.type === 'inputNode');
      const second = structuredClone(first);
      second.id = 'second-input';
      second.data.params.cliName = 'second';
      doc.graph.nodes.push(second);
      const merge = doc.graph.nodes.find((n) => n.data.definitionId === 'channel_merge');
      const blue = doc.graph.edges.find((e) => e.target === merge.id && e.targetHandle === 'in-2');
      blue.source = second.id;
      blue.sourceHandle = 'out-0';
    },
  ],
  [
    'disconnected-rename',
    'wf-09-rename.bite',
    { in: atlas },
    (doc) => {
      const rename = doc.graph.nodes.find((n) => n.data.definitionId === 'rename');
      const incoming = doc.graph.edges.find((e) => e.target === rename.id);
      const outgoing = doc.graph.edges.find((e) => e.source === rename.id);
      outgoing.source = incoming.source;
      outgoing.sourceHandle = incoming.sourceHandle;
      doc.graph.edges = doc.graph.edges.filter((e) => e !== incoming);
    },
  ],
  [
    'chained-renames',
    'wf-09-rename.bite',
    { in: atlas },
    (doc) => {
      const first = doc.graph.nodes.find((n) => n.data.definitionId === 'rename');
      const second = structuredClone(first);
      second.id = 'second-rename';
      second.data.params.blocks = [
        { type: 'text', value: 'second_' },
        { type: 'oldname', find: '', replace_with: '' },
      ];
      const outgoing = doc.graph.edges.find((e) => e.source === first.id);
      const link = { ...outgoing, id: 'rename-chain', target: second.id };
      outgoing.source = second.id;
      doc.graph.nodes.push(second);
      doc.graph.edges.push(link);
    },
  ],
  ...[
    ['bypass-linear', 'wf-01-fastpath.bite', null],
    ['bypass-gate', 'wf-07-gate.bite', 'gate'],
    ['bypass-rename', 'wf-09-rename.bite', 'rename'],
    ['bypass-formats', 'wf-08-formats.bite', 'format_convert'],
    ['bypass-channels', 'wf-04-channels.bite', null],
  ].map(([name, fixture, definition]) => [
    name,
    fixture,
    { in: atlas },
    (doc) => {
      for (const node of doc.graph.nodes) {
        if (node.data.definitionId && (!definition || node.data.definitionId === definition)) {
          node.data.params._enabled = false;
        }
      }
    },
  ]),
  ['partial-sets', 'wf-06-setmode.bite', { in: partial }],
  ['natural-atlas', 'wf-10-flipbook.bite', { in: atlas }],
  [
    'reverse-natural-atlas',
    'wf-10-flipbook.bite',
    { in: atlas },
    (doc) => {
      doc.graph.nodes.find((n) => n.type === 'flipbookOutputNode').data.params.sortBy = 'name_desc';
    },
  ],
  [
    'rename-collisions',
    'wf-09-rename.bite',
    { in: atlas },
    (doc) => {
      doc.graph.nodes.find((n) => n.data.definitionId === 'rename').data.params.blocks = [
        { type: 'text', value: 'same' },
      ];
    },
    true,
  ],
  [
    'multiple-inputs',
    'wf-01-fastpath.bite',
    { in: atlas, second: partial },
    (doc) => {
      const other = structuredClone(doc.graph);
      for (const n of other.nodes) {
        n.id += '-second';
        if (n.type === 'inputNode') n.data.params.cliName = 'second';
        if (n.type === 'imageOutputNode') n.data.params.cliName = 'second-out';
      }
      for (const e of other.edges) {
        e.id += '-second';
        e.source += '-second';
        e.target += '-second';
      }
      doc.graph.nodes.push(...other.nodes);
      doc.graph.edges.push(...other.edges);
    },
  ],
];
const results = [];
for (const args of cases) {
  try {
    results.push(compare(...args));
  } catch (error) {
    console.error(`FAIL ${args[0]}: ${error}`);
    results.push({ name: args[0], passed: false, error: String(error) });
  }
}
// Solid Image is an intentional migration fix, not legacy parity: the original
// executor was unregistered. Verify its defined output independently.
try {
  const doc = JSON.parse(fs.readFileSync(path.join(root, 'wf-04-channels.bite'), 'utf8'));
  const solid = doc.graph.nodes.find((n) => n.data.definitionId === 'value_float');
  solid.data.definitionId = 'solid_image';
  solid.data.params = { value: 0.25, _enabled: true };
  for (const edge of doc.graph.edges.filter((e) => e.source === solid.id)) edge.sourceHandle = 'out-0';
  const out = path.join(run, 'solid-fill');
  const workflow = path.join(run, 'solid-fill.bite');
  fs.writeFileSync(workflow, JSON.stringify(doc));
  const result = invoke(rust, ['run', workflow, '--in', atlas, '--out', out]);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(allFiles(out).length, 3);
  for (const file of allFiles(out)) {
    const observed = invoke(magick, [path.join(out, file), '-format', '%wx%h %[fx:mean.b]', 'info:']);
    assert.equal(observed.status, 0, observed.stderr);
    const [dimensions, blue] = observed.stdout.trim().split(/\s+/);
    assert.equal(dimensions, '32x32');
    assert.ok(Math.abs(Number(blue) - 0.25) <= 0.5 / 255 + 0.00001, observed.stdout);
  }
  results.push({ name: 'solid-fill', passed: true });
  console.log('PASS solid-fill (3 outputs, dimensions and native fill verified)');
} catch (error) {
  results.push({ name: 'solid-fill', passed: false, error: String(error) });
  console.error(`FAIL solid-fill: ${error}`);
}
fs.writeFileSync(path.join(run, 'results.json'), JSON.stringify(results, null, 2) + '\n');
console.log(`Results: ${path.join(run, 'results.json')}`);
if (results.some((r) => !r.passed)) process.exitCode = 1;
