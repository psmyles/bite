import { readFileSync, readdirSync } from 'node:fs';
import { resolve, extname, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { NodeRegistry } from '../main/nodes/registry.js';
import { PipelineExecutor } from '../main/pipeline/executor.js';
import type { NodeGraph } from '../shared/types.js';
import { traceInputNodeId } from '../shared/graphTrace.js';
import { sanitizeWorkflowGraph } from '../main/ipc/workflow-sanitize.js';
import { IMAGE_EXTENSIONS } from '../shared/constants.js';

const __dirname = dirname(fileURLToPath(import.meta.url));

type GraphNode = NodeGraph['nodes'][number];

// Same formats the app accepts (shared list), as dotted extensions for extname().
const IMAGE_EXTS = new Set(IMAGE_EXTENSIONS.map((e) => '.' + e));

function die(msg: string): never {
  console.error(`[imgplex] ${msg}`);
  process.exit(1);
}

function usage(): never {
  console.log(
    [
      'imgplex-cli',
      '',
      'Commands:',
      '  imgplex-cli run <workflow.imgplex> [flags]',
      '',
      'Flags:',
      '  --<cliName> <path>   Named input/output defined in the workflow (see script comments)',
      '  --overwrite          Overwrite existing output files (default: skip)',
      '',
      'Named flags correspond to the CLI Name set on each Input/Output node in the workflow.',
      'Export a CLI script from imgplex (File → Export CLI Script) to see the exact flags for',
      'a given workflow.',
    ].join('\n')
  );
  process.exit(0);
}

function nodeCliName(node: GraphNode): string {
  return ((node.data.params.cliName as string) ?? '').trim();
}

async function main(): Promise<void> {
  const argv = process.argv.slice(2);
  if (argv.length === 0 || argv[0] === '--help' || argv[0] === '-h') usage();
  if (argv[0] !== 'run') die(`Unknown command: "${argv[0]}". Run "imgplex-cli --help" for usage.`);

  const workflowArg = argv[1];
  if (!workflowArg) die('Missing workflow file argument.\nUsage: imgplex-cli run <workflow.imgplex>');

  // Parse --flag value pairs and --overwrite boolean
  let overwrite: 'skip' | 'overwrite' = 'skip';
  const flagValues = new Map<string, string>(); // flag name → path value

  for (let i = 2; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--overwrite') {
      overwrite = 'overwrite';
    } else if (a.startsWith('--') && argv[i + 1] && !argv[i + 1].startsWith('--')) {
      flagValues.set(a.slice(2), argv[++i]);
    }
  }

  // Load workflow
  const workflowPath = resolve(workflowArg);
  let graph: NodeGraph;
  try {
    const raw = JSON.parse(readFileSync(workflowPath, 'utf-8')) as Record<string, unknown>;
    // A .imgplex is untrusted — strip `__`-prefixed params / malicious param wires,
    // matching the Electron load path (defense-in-depth over runtime stripping).
    graph = sanitizeWorkflowGraph(raw.graph ?? raw) as NodeGraph;
  } catch {
    die(`Cannot read workflow: ${workflowPath}`);
  }

  // Build cliName → node map for input nodes and output nodes
  const INPUT_TYPES = new Set(['inputNode']);
  const OUTPUT_TYPES = new Set(['imageOutputNode', 'textOutputNode', 'flipbookOutputNode']);

  const inputNodeByFlag = new Map<string, GraphNode>();
  const outputNodeByFlag = new Map<string, GraphNode>();

  for (const node of graph.nodes) {
    const name = nodeCliName(node);
    if (!name) continue;
    if (INPUT_TYPES.has(node.type)) inputNodeByFlag.set(name, node);
    else if (OUTPUT_TYPES.has(node.type)) outputNodeByFlag.set(name, node);
  }

  // Collect all output nodes (even those without a cliName — they use their baked-in paths)
  const outputNodes = graph.nodes.filter((n) => OUTPUT_TYPES.has(n.type));
  if (outputNodes.length === 0) die('Workflow has no output nodes.');

  // In a pkg-compiled binary, node-definitions are real files next to the exe.
  // In dev, they're at the project root relative to dist-electron/.
  const nodeDefsDir = (process as NodeJS.Process & { pkg?: unknown }).pkg
    ? resolve(dirname(process.execPath), 'node-definitions')
    : resolve(__dirname, '..', 'node-definitions');
  const registry = new NodeRegistry();
  await registry.load(nodeDefsDir);

  const executor = new PipelineExecutor();

  for (const outNode of outputNodes) {
    const outCliName = nodeCliName(outNode);
    const label = outCliName || outNode.type;

    // Find the input node that feeds this output node
    const inputNodeId = traceInputNodeId(graph.nodes, graph.edges, outNode.id);
    if (!inputNodeId) {
      console.warn(`[imgplex] Skipping "${label}" — cannot trace back to an Input node.`);
      continue;
    }

    const inputNode = graph.nodes.find((n) => n.id === inputNodeId)!;
    const inputCliName = nodeCliName(inputNode);

    // Resolve the input directory
    const inputDirRaw = inputCliName ? flagValues.get(inputCliName) : undefined;

    if (!inputDirRaw) {
      die(
        `Missing required flag: --${inputCliName || 'input'}\n` +
          `  Needed to run output node "${label}".\n` +
          `  Export a CLI script from imgplex to see all required flags.`
      );
    }

    const inputAbs = resolve(inputDirRaw!);
    let images: string[];
    try {
      images = readdirSync(inputAbs)
        .filter((f) => IMAGE_EXTS.has(extname(f).toLowerCase()))
        .map((f) => resolve(inputAbs, f));
    } catch {
      die(`Cannot read input directory: ${inputAbs}`);
    }
    if (images.length === 0) die(`No images found in: ${inputAbs}`);

    // Resolve the output path for this node
    let outputDir: string | null = null;
    let patchedGraph = graph;

    if (outNode.type === 'imageOutputNode') {
      const flagValue = outCliName ? flagValues.get(outCliName) : undefined;
      if (flagValue) {
        outputDir = resolve(flagValue);
      } else {
        const outputPath = (outNode.data.params.outputPath as string) ?? 'source';
        if (outputPath === 'custom') {
          const cp = (outNode.data.params.customPath as string) ?? '';
          outputDir = cp ? resolve(cp) : null;
        }
        // 'source' → outputDir stays null (same folder as source)
      }
    } else {
      // textOutputNode / flipbookOutputNode: output path is a file path baked into params
      const flagValue = outCliName ? flagValues.get(outCliName) : undefined;
      if (flagValue) {
        // Patch the graph copy to override the output path for this node
        const paramKey = outNode.type === 'textOutputNode' ? 'outputPath' : 'flipbookOutputPath';
        patchedGraph = {
          ...graph,
          nodes: graph.nodes.map((n) =>
            n.id === outNode.id
              ? { ...n, data: { ...n.data, params: { ...n.data.params, [paramKey]: resolve(flagValue) } } }
              : n
          ),
        };
      }
    }

    process.stdout.write(`\n[${label}] Processing ${images.length} image(s)...\n`);

    await executor.executeBatch(
      patchedGraph,
      outNode.id,
      inputNodeId,
      images,
      outputDir,
      overwrite,
      registry,
      ({ completed, total, currentFile }) => {
        const pct = Math.round((completed / total) * 100)
          .toString()
          .padStart(3);
        const file = currentFile.length > 40 ? '...' + currentFile.slice(-37) : currentFile.padEnd(40);
        process.stdout.write(`\r  [${pct}%] ${completed}/${total}  ${file}`);
      }
    );

    process.stdout.write('\n');
    if (outputDir) {
      console.log(`Done → ${outputDir}`);
    } else if (outNode.type === 'imageOutputNode') {
      console.log(`Done → (same folder as source)`);
    } else {
      const paramKey = outNode.type === 'textOutputNode' ? 'outputPath' : 'flipbookOutputPath';
      const outPath = (patchedGraph.nodes.find((n) => n.id === outNode.id)?.data.params[paramKey] as string) ?? '';
      console.log(`Done → ${outPath || '(path not set)'}`);
    }
  }
}

main().catch((err) => {
  console.error(`[imgplex] ${err instanceof Error ? err.message : String(err)}`);
  process.exit(1);
});
