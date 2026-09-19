// Cross-platform Phase 0 runner. All outputs are redirected into a fresh run
// directory, including paths baked into fixtures. Never writes into test_images.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import assert from 'node:assert/strict';

const root = path.dirname(fileURLToPath(import.meta.url));
const repo = path.dirname(root);
const onlyIndex = process.argv.indexOf('--only');
const only = onlyIndex < 0 ? '' : process.argv[onlyIndex + 1];
if (onlyIndex >= 0 && !/^wf-\d{2}$/.test(only ?? '')) throw Error('--only requires wf-NN');
const outRoot = path.join(root, 'out');
fs.mkdirSync(outRoot, { recursive: true });
const runRoot = fs.mkdtempSync(path.join(outRoot, 'reference-'));
const fixture = path.join(runRoot, 'fixtures');
const magick = process.env.BITE_MAGICK || 'magick';
const cli = process.env.BITE_TEST_CLI;
if (cli && process.env.BITE_UPDATE_GOLDENS === '1') throw Error('Only the legacy CLI may update reference goldens');
const compareIndex = process.argv.indexOf('--compare-with');
const compareRoot = compareIndex < 0 ? null : process.argv[compareIndex + 1];
if (compareIndex >= 0 && !compareRoot) throw Error('--compare-with requires a reference run directory');
const tracePath = path.join(runRoot, 'magick-calls.jsonl');
const recorder = path.join(runRoot, 'record-spawn.cjs');
fs.writeFileSync(
  recorder,
  `const cp = require('node:child_process');
const fs = require('node:fs');
const original = cp.spawn;
cp.spawn = function(command, args, options) {
  if (/magick(?:\\.exe)?$/i.test(command)) fs.appendFileSync(process.env.BITE_TRACE_PATH, JSON.stringify(args) + '\\n');
  return original.apply(this, arguments);
};
require('node:module').syncBuiltinESMExports();
`
);
function invoke(exe, args) {
  const r = spawnSync(exe, args, {
    cwd: repo,
    env: { ...process.env, BITE_TRACE_PATH: tracePath },
    encoding: 'utf8',
    timeout: 120000,
    maxBuffer: 16 * 1024 * 1024,
  });
  if (r.error) throw r.error;
  return { status: r.status, stdout: r.stdout.replace(/\r\n/g, '\n'), stderr: r.stderr.replace(/\r\n/g, '\n') };
}
function im(...args) {
  const r = invoke(magick, args);
  assert.equal(r.status, 0, r.stderr);
  return r.stdout.trim();
}
function bite(args) {
  return cli
    ? invoke(cli, args)
    : invoke(process.execPath, ['--require', recorder, path.join(repo, 'dist-cli/cli-bundle.js'), ...args]);
}
function make(dir, name, args, prefix = '') {
  im(...args, prefix + path.join(fixture, dir, name));
}
for (const dir of ['main', 'meta', 'sets', 'flip']) fs.mkdirSync(path.join(fixture, dir), { recursive: true });
for (const [name, color] of [
  ['red', '255,0,0'],
  ['green', '0,255,0'],
  ['blue', '0,0,255'],
])
  make('main', `${name}_256.png`, ['-size', '256x256', `xc:srgb(${color})`]);
make('main', 'gray50_256.png', ['-size', '256x256', 'xc:gray(128)'], 'PNG24:');
make(
  'main',
  'checker_100x80.png',
  [
    '-size',
    '10x10',
    'xc:black',
    'xc:white',
    '+append',
    '(',
    '+clone',
    '-flop',
    ')',
    '-append',
    '-write',
    'mpr:tile',
    '+delete',
    '-size',
    '100x80',
    'tile:mpr:tile',
  ],
  'PNG24:'
);
make(
  'main',
  'alpha_grad_128.png',
  [
    '-size',
    '128x128',
    'xc:white',
    '(',
    '-size',
    '128x128',
    'gradient:',
    ')',
    '-alpha',
    'off',
    '-compose',
    'CopyOpacity',
    '-composite',
  ],
  'PNG32:'
);
make('meta', 'deep16_64.png', ['-size', '64x64', 'gradient:', '-depth', '16']);
make('meta', 'photo_300dpi.jpg', [
  '-size',
  '200x150',
  'gradient:blue-yellow',
  '-units',
  'PixelsPerInch',
  '-density',
  '300',
  '-set',
  'exif:Make',
  'TestCam',
  '-quality',
  '90',
]);
for (const [set, values] of [
  ['alpha', [200, 100, 50]],
  ['beta', [30, 60, 90]],
])
  for (const [i, suffix] of ['diffuse', 'normal', 'rough'].entries())
    make('sets', `set_${set}_${suffix}.png`, ['-size', '64x64', `xc:gray(${values[i]})`]);
for (const [name, color] of [
  ['a_red', '255,0,0'],
  ['b_green', '0,255,0'],
  ['c_blue', '0,0,255'],
])
  make('flip', `fb_${name}.png`, ['-size', '64x64', `xc:srgb(${color})`]);
const names = fs.readdirSync(path.join(fixture, 'main')).sort();
const files = (dir) =>
  fs
    .readdirSync(dir)
    .filter((f) => fs.statSync(path.join(dir, f)).isFile())
    .sort();
const lines = (file) => fs.readFileSync(file, 'utf8').trim().split(/\r?\n/);
function means(file, expected, tolerance = 0.02) {
  const actual = im(file, '-format', '%[fx:mean.r] %[fx:mean.g] %[fx:mean.b]', 'info:').split(/\s+/).map(Number);
  expected.forEach((v, i) => assert.ok(Math.abs(actual[i] - v) <= tolerance, `${file}: ${actual} != ${expected}`));
}
function dims(file, expected) {
  assert.equal(im('identify', '-format', '%wx%h', file), expected);
}
const results = [];
const contract = [];
const observations = [];
for (const file of fs
  .readdirSync(root)
  .filter((f) => /^wf-.*\.bite$/.test(f))
  .sort()) {
  const id = file.slice(0, 5);
  if (only && id !== only) continue;
  const out = path.join(runRoot, id);
  fs.mkdirSync(out, { recursive: true });
  const graphFile = JSON.parse(fs.readFileSync(path.join(root, file), 'utf8'));
  const args = [
    'run',
    path.join(runRoot, file),
    '--in',
    path.join(fixture, id === 'wf-03' ? 'meta' : id === 'wf-06' ? 'sets' : id === 'wf-10' ? 'flip' : 'main'),
  ];
  for (const n of graphFile.graph.nodes) {
    const p = n.data.params;
    if (n.type === 'imageOutputNode') {
      const target =
        id === 'wf-08'
          ? path.join(out, p.cliName.replace('out-', ''))
          : ['wf-05', 'wf-11'].includes(id)
            ? path.join(out, 'images')
            : out;
      p.outputPath = 'custom';
      p.customPath = target;
      if (p.cliName) args.push(`--${p.cliName}`, target);
    } else if (n.type === 'textOutputNode' || n.type === 'flipbookOutputNode') {
      const target = path.join(out, n.type === 'textOutputNode' ? 'report.txt' : 'atlas.png');
      p[n.type === 'textOutputNode' ? 'outputPath' : 'flipbookOutputPath'] = target;
      if (p.cliName) args.push(`--${p.cliName}`, target);
    }
  }
  fs.writeFileSync(args[1], JSON.stringify(graphFile));
  try {
    fs.writeFileSync(tracePath, '');
    const r = bite(args);
    assert.equal(r.status, 0, r.stderr + r.stdout);
    const magickCalls = lines(tracePath)
      .filter(Boolean)
      .map((s) => JSON.parse(s));
    if (id === 'wf-01') {
      assert.deepEqual(files(out), names);
      for (const f of names) dims(path.join(out, f), '128x128');
    }
    if (id === 'wf-02') {
      const report = lines(path.join(out, 'report.txt'));
      assert.equal(report.length, 6);
      report.forEach((s, i) => {
        const [name, ext, size, w, h, pot] = s.split(',');
        assert.equal(name, names[i]);
        assert.equal(ext, 'png');
        assert.ok(Number(size) > 0);
        const checker = name.startsWith('checker');
        assert.equal(`${w}x${h}`, checker ? '100x80' : name.startsWith('alpha') ? '128x128' : '256x256');
        assert.equal(pot, String(!checker));
      });
    }
    if (id === 'wf-03') {
      const report = lines(path.join(out, 'report.txt'));
      assert.equal(report.length, 2);
      assert.match(report[0], /^deep16_64\.png,16,/);
      assert.match(report[1], /^photo_300dpi\.jpg,8,300/);
    }
    if (id === 'wf-04') {
      assert.deepEqual(files(out), names);
      for (const [color, expected] of [
        ['red', [1, 1, 0.5]],
        ['green', [0, 0, 0.5]],
        ['blue', [0, 1, 0.5]],
      ])
        means(path.join(out, `${color}_256.png`), expected);
    }
    if (id === 'wf-05') {
      assert.deepEqual(lines(path.join(out, 'report.txt')), [
        'alpha_grad_128.png,1,1,1,false',
        'blue_256.png,0,0,1,false',
        'checker_100x80.png,0.5,0.5,0.5,false',
        'gray50_256.png,0.502,0.502,0.502,false',
        'green_256.png,0,1,0,false',
        'red_256.png,1,0,0,true',
      ]);
      assert.deepEqual(files(path.join(out, 'images')), ['red_256.png']);
    }
    if (id === 'wf-06') {
      assert.deepEqual(files(out), ['packed_alpha.png', 'packed_beta.png']);
      means(path.join(out, 'packed_alpha.png'), [200 / 255, 100 / 255, 205 / 255]);
      means(path.join(out, 'packed_beta.png'), [30 / 255, 60 / 255, 165 / 255]);
    }
    if (id === 'wf-07') assert.deepEqual(files(out), ['red_256.png']);
    if (id === 'wf-08') {
      for (const [fmt, ext] of Object.entries({
        png: 'png',
        jpeg: 'jpg',
        webp: 'webp',
        avif: 'avif',
        tiff: 'tif',
        bmp: 'bmp',
        tga: 'tga',
      })) {
        const dir = path.join(out, fmt);
        assert.deepEqual(files(dir), names.map((n) => n.replace(/\.png$/, '.' + ext)).sort());
        const red = path.join(dir, `red_256.${ext}`);
        assert.ok(im('identify', '-format', '%m', red).includes(fmt.toUpperCase()));
        means(red, [1, 0, 0], 0.05);
      }
      const comparison = invoke(magick, [
        'compare',
        '-metric',
        'AE',
        path.join(fixture, 'main/red_256.png'),
        path.join(out, 'webp/red_256.webp'),
        'null:',
      ]);
      assert.equal(comparison.status, 0);
      assert.match(comparison.stderr, /^0(?:\s|$)/);
    }
    if (id === 'wf-09') {
      assert.deepEqual(
        files(out),
        names.map((n, i) => `test_${String(i + 1).padStart(3, '0')}_${n}`)
      );
      const probe = path.join(out, 'test_006_red_256.png');
      fs.writeFileSync(probe, 'SENTINEL');
      assert.equal(bite(args).status, 0);
      assert.equal(fs.readFileSync(probe, 'utf8'), 'SENTINEL');
      assert.equal(bite([...args, '--overwrite']).status, 0);
      assert.equal(fs.readFileSync(probe).subarray(1, 4).toString(), 'PNG');
      contract.push({
        case: 'overwrite',
        default: 'skip',
        flag: '--overwrite',
        sentinel_preserved_without_flag: true,
        sentinel_replaced_with_flag: true,
      });
    }
    if (id === 'wf-10') {
      const atlas = path.join(out, 'atlas.png');
      dims(atlas, '128x128');
      for (const [crop, expected] of [
        ['0+0', [1, 0, 0]],
        ['64+0', [0, 1, 0]],
        ['0+64', [0, 0, 1]],
        ['64+64', [1, 0, 1]],
      ])
        means(`${atlas}[64x64+${crop}]`, expected);
    }
    if (id === 'wf-11') {
      assert.deepEqual(files(path.join(out, 'images')), names);
      assert.deepEqual(lines(path.join(out, 'report.txt')), Array(6).fill('5,5,0.6,1024,OK'));
    }
    const observed = [];
    function observe(dir) {
      for (const entry of fs.readdirSync(dir).sort()) {
        const full = path.join(dir, entry);
        if (fs.statSync(full).isDirectory()) {
          observe(full);
          continue;
        }
        const relative = path.relative(out, full).split(path.sep).join('/');
        if (entry.endsWith('.txt')) {
          let rows = lines(full);
          // Encoded file byte size and EXIF writing vary with codec/build version.
          if (id === 'wf-02') rows = rows.map((s) => s.replace(/^(.*?,.*?),[^,]+,/, '$1,<bytes>,')).sort();
          if (id === 'wf-03') rows = rows.map((s) => s.split(',').slice(0, 3).join(',')).sort();
          observed.push({ file: relative, lines: rows });
        } else observed.push({ file: relative, dimensions: im('identify', '-format', '%wx%h', full) });
      }
    }
    observe(out);
    if (compareRoot) {
      for (const item of observed.filter((o) => o.dimensions)) {
        const reference = path.join(compareRoot, id, item.file);
        const actual = path.join(out, item.file);
        const lossy = /\.(jpg|avif)$/i.test(item.file);
        const metric = lossy ? 'RMSE' : 'AE';
        const r = invoke(magick, ['compare', '-metric', metric, reference, actual, 'null:']);
        if (lossy) {
          const normalized = r.stderr.match(/\(([\d.e+-]+)\)/);
          assert.ok(r.status !== 2 && normalized && Number(normalized[1]) <= 0.005, `${item.file}: RMSE ${r.stderr}`);
        } else assert.equal(r.status, 0, `${item.file}: exact pixel comparison failed: ${r.stderr}`);
      }
    }
    observations.push({ workflow: id, outputs: observed });
    if (!cli) {
      const streams = new Map();
      for (const args of magickCalls) {
        const source = args.find((t) => t.startsWith(fixture));
        if (source)
          for (const token of args) {
            const hash = token.match(/batch_ms_([a-f0-9]+)_/);
            if (hash && !streams.has(hash[1])) streams.set(hash[1], path.basename(source));
          }
      }
      const normalizedCalls = magickCalls
        .map((args) =>
          args.map((token) =>
            token
              .replaceAll(runRoot, '<run>')
              .replaceAll('\\', '/')
              .replace(/^.*\/bite-preview\//, '<temporary>/')
              .replace(/batch_ms_([a-f0-9]+)_/g, (_m, hash) => {
                assert.ok(streams.has(hash), `unidentified temporary stream ${hash}`);
                return `batch_ms_${streams.get(hash)}_`;
              })
          )
        )
        .sort((a, b) => JSON.stringify(a).localeCompare(JSON.stringify(b), 'en'));
      const traceGolden = path.join(repo, 'tests/golden/workflows', `${id}-magick.json`);
      if (process.env.BITE_UPDATE_GOLDENS === '1')
        fs.writeFileSync(traceGolden, JSON.stringify(normalizedCalls, null, 2) + '\n');
      assert.deepEqual(
        normalizedCalls,
        JSON.parse(fs.readFileSync(traceGolden, 'utf8')),
        `${id}: ImageMagick argument drift`
      );
    }
    results.push({ workflow: id, passed: true });
    console.log(`PASS ${id}`);
  } catch (error) {
    results.push({ workflow: id, passed: false, error: String(error) });
    console.error(`FAIL ${id}: ${error}`);
  }
}
for (const [name, args, code, pattern] of [
  ['help', ['--help'], 0, /--overwrite/],
  ['unknown-command', ['unknown'], 1, /\[Bite\] Unknown command/],
  ['missing-workflow', ['run'], 1, /\[Bite\] Missing workflow/],
  ['missing-input', ['run', path.join(root, 'wf-01-fastpath.bite')], 1, /\[Bite\] Missing required flag: --in/],
]) {
  const r = bite(args);
  const passed = r.status === code && pattern.test(r.stdout + r.stderr);
  results.push({ case: name, passed });
  contract.push({ case: name, exit_code: r.status, stdout: r.stdout, stderr: r.stderr });
}
const report = { platform: process.platform, imagemagick: im('-version'), results, cli_contract: contract };
fs.writeFileSync(path.join(runRoot, 'results.json'), JSON.stringify(report, null, 2) + '\n');
if (!only && !results.some((r) => !r.passed)) {
  const goldenPath = path.join(repo, 'tests/golden/workflow-execution.json');
  const stable = { workflows: observations, cli_contract: contract };
  if (process.env.BITE_UPDATE_GOLDENS === '1') fs.writeFileSync(goldenPath, JSON.stringify(stable, null, 2) + '\n');
  const expected = JSON.parse(fs.readFileSync(goldenPath, 'utf8'));
  if (cli) {
    // The plan permits additive developer commands. Keep exact stderr/exit codes;
    // help prose and the legacy registry's debug stdout are not the flag contract.
    const help = contract.find((c) => c.case === 'help').stdout;
    for (const token of ['--<cliName>', '--overwrite', 'default: skip']) assert.ok(help.includes(token));
    for (const c of stable.cli_contract) delete c.stdout;
    for (const c of expected.cli_contract) delete c.stdout;
  }
  assert.deepEqual(stable, expected, 'Workflow/CLI reference drift');
}
console.log(`Results: ${path.join(runRoot, 'results.json')}`);
if (!results.some((r) => r.workflow)) throw Error('No workflows selected');
if (results.some((r) => !r.passed)) process.exitCode = 1;
