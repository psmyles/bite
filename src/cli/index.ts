import { readFileSync, readdirSync } from 'node:fs'
import { resolve, extname, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'
import { NodeRegistry } from '../main/nodes/registry.js'
import { PipelineExecutor } from '../main/pipeline/executor.js'
import type { NodeGraph } from '../shared/types.js'

const __dirname = dirname(fileURLToPath(import.meta.url))

const IMAGE_EXTS = new Set([
  '.jpg', '.jpeg', '.png', '.webp', '.avif', '.gif', '.bmp', '.ico',
  '.tif', '.tiff',
  '.heic', '.heif',
  '.jp2', '.j2k', '.jpf', '.jpx', '.jxl',
  '.psd', '.psb', '.exr', '.hdr', '.dpx', '.cin',
  '.cr2', '.cr3', '.nef', '.nrw', '.arw', '.dng', '.orf', '.raf', '.rw2', '.pef', '.srw',
  '.tga', '.pcx', '.ppm', '.pgm', '.pbm', '.pnm', '.sgi', '.dds',
])

function die(msg: string): never {
  console.error(`[imgplex] ${msg}`)
  process.exit(1)
}

function usage(): never {
  console.log([
    'imgplex-cli',
    '',
    'Commands:',
    '  imgplex-cli run <workflow.imgplex> [options]',
    '',
    'Options:',
    '  --input,  -i <dir>   Source image folder  (default: current directory)',
    '  --output, -o <dir>   Output folder         (default: ./output)',
    '  --overwrite          Overwrite existing output files (default: skip)',
  ].join('\n'))
  process.exit(0)
}

async function main(): Promise<void> {
  const argv = process.argv.slice(2)
  if (argv.length === 0 || argv[0] === '--help' || argv[0] === '-h') usage()
  if (argv[0] !== 'run') die(`Unknown command: "${argv[0]}". Run "imgplex-cli --help" for usage.`)

  const workflowArg = argv[1]
  if (!workflowArg) die('Missing workflow file argument.\nUsage: imgplex-cli run <workflow.imgplex>')

  let inputDir  = '.'
  let outputDir = 'output'
  let overwrite: 'skip' | 'overwrite' = 'skip'

  for (let i = 2; i < argv.length; i++) {
    const a = argv[i]
    if ((a === '--input' || a === '-i') && argv[i + 1]) {
      inputDir = argv[++i]
    } else if ((a === '--output' || a === '-o') && argv[i + 1]) {
      outputDir = argv[++i]
    } else if (a === '--overwrite') {
      overwrite = 'overwrite'
    }
  }

  // Load workflow
  const workflowPath = resolve(workflowArg)
  let graph: NodeGraph
  try {
    const raw = JSON.parse(readFileSync(workflowPath, 'utf-8')) as Record<string, unknown>
    graph = (raw.graph ?? raw) as NodeGraph
  } catch {
    die(`Cannot read workflow: ${workflowPath}`)
  }

  // Scan input for images
  const inputAbs = resolve(inputDir)
  let images: string[]
  try {
    images = readdirSync(inputAbs)
      .filter(f => IMAGE_EXTS.has(extname(f).toLowerCase()))
      .map(f => resolve(inputAbs, f))
  } catch {
    die(`Cannot read input directory: ${inputAbs}`)
  }
  if (images.length === 0) die(`No images found in: ${inputAbs}`)

  // In a pkg-compiled binary, node-definitions are real files next to the exe
  // (installed via extraFiles) so users can add custom nodes.
  // In dev, they're at the project root relative to dist-electron/.
  const nodeDefsDir = (process as NodeJS.Process & { pkg?: unknown }).pkg
    ? resolve(dirname(process.execPath), 'node-definitions')
    : resolve(__dirname, '..', 'node-definitions')
  const registry = new NodeRegistry()
  await registry.load(nodeDefsDir)

  const executor    = new PipelineExecutor()
  const outputAbs   = resolve(outputDir)

  process.stdout.write(`Processing ${images.length} image(s)...\n`)

  await executor.executeBatch(
    graph,
    images,
    outputAbs,
    overwrite,
    registry,
    ({ completed, total, currentFile }) => {
      const pct  = Math.round((completed / total) * 100).toString().padStart(3)
      const file = currentFile.length > 40 ? '...' + currentFile.slice(-37) : currentFile.padEnd(40)
      process.stdout.write(`\r  [${pct}%] ${completed}/${total}  ${file}`)
    },
  )

  process.stdout.write('\n')
  console.log(`Done → ${outputAbs}`)
}

main().catch(err => {
  console.error(`[imgplex] ${err instanceof Error ? err.message : String(err)}`)
  process.exit(1)
})
