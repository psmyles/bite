// Reproducible Windows/macOS backend comparison. Build both CLIs before running.
import fs from 'node:fs';
import path from 'node:path';
import { performance } from 'node:perf_hooks';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.dirname(here);
const input = path.resolve(process.argv[2] ?? path.join(root, 'test_images'));
const workflow = path.join(here, 'wf-01-fastpath.bite');
const rust = path.join(root, 'target', 'release', process.platform === 'win32' ? 'bite.exe' : 'bite');
const node = process.execPath;
const nodeCli = path.join(root, 'dist-cli', 'cli-bundle.js');
const stamp = new Date().toISOString().replaceAll(/[:.]/g, '-');
const runRoot = path.join(here, 'out', `benchmark-${stamp}`);
fs.mkdirSync(runRoot, { recursive: true });

function invoke(executable, args) {
  const started = performance.now();
  const result = spawnSync(executable, args, { cwd: root, encoding: 'utf8', timeout: 10 * 60_000 });
  const elapsedMs = performance.now() - started;
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${executable} failed (${result.status}):\n${result.stderr}\n${result.stdout}`);
  return { elapsedMs, stdout: result.stdout, stderr: result.stderr };
}

function files(directory) {
  return fs.readdirSync(directory, { withFileTypes: true }).filter((entry) => entry.isFile()).length;
}

const importInput = path.join(runRoot, 'import-input');
fs.cpSync(input, importInput, { recursive: true });
const report = { platform: process.platform, arch: process.arch, input, workflow, startup: {}, import: {}, workflowRuns: {} };
const nodeImport = invoke(node, [
  path.join(here, 'out', 'benchmark-tools', 'benchmark-node-import.mjs'),
  importInput,
  '128',
]);
report.import.node = JSON.parse(nodeImport.stdout.trim().split('\n').at(-1));
const rustImport = invoke(rust, ['bench-import', importInput, '--recursive', '--size', '128', '--jobs', '8']);
report.import.rust = JSON.parse(rustImport.stdout);
for (const [name, executable, prefix] of [
  ['node', node, [nodeCli]],
  ['rust', rust, []],
]) {
  report.startup[name] = Array.from({ length: 5 }, () => invoke(executable, [...prefix, '--help']).elapsedMs);
  const output = path.join(runRoot, name);
  fs.mkdirSync(output, { recursive: true });
  report.workflowRuns[name] = [];
  for (let iteration = 1; iteration <= 2; iteration++) {
    const run = invoke(executable, [
      ...prefix,
      'run',
      workflow,
      '--in',
      input,
      '--out',
      output,
      '--overwrite',
      ...(name === 'rust' ? ['--jobs', '8'] : []),
    ]);
    report.workflowRuns[name].push({ iteration, elapsedMs: run.elapsedMs, outputFiles: files(output) });
  }
}
const reportPath = path.join(runRoot, 'results.json');
fs.writeFileSync(reportPath, JSON.stringify(report, null, 2) + '\n');
console.log(JSON.stringify(report, null, 2));
console.log(`Results: ${reportPath}`);
