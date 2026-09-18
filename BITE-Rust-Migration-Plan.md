# BITE Rust Migration Plan

## Purpose

This document describes a controlled migration of BITE from its current Electron, Svelte, TypeScript, and JavaScript runtime to a native Rust application with a Dear ImGui interface.

The migration is deliberately incremental. Every phase ends with something independently testable and shippable, and the existing Electron application remains usable until the Rust implementation can reproduce real workflows reliably.

The plan preserves BITE's existing strengths:

- Runtime-loaded JSON node definitions
- Versioned `.bite` workflow files
- Headless CLI execution
- Graph validation and typed connections
- Node-level preview caching
- Native image-header fast paths
- Operation fusion and batched ImageMagick processing
- Specialized multi-stream, channel, and set-processing behavior

The central architectural rule is:

> Expressions calculate values. JSON describes arguments. Rust controls execution topology.

---

## Target architecture

```text
bite/
├── Cargo.toml
├── crates/
│   ├── bite-schema/          JSON schemas and serde types
│   ├── bite-expr/            Constrained expression language
│   ├── bite-core/            Graphs, workflows, planning, and execution
│   ├── bite-imagemagick/     ImageMagick process integration
│   ├── bite-cli/             Headless CLI
│   └── bite-gui/             Dear ImGui application
│
├── schemas/
├── node-definitions/
├── format-definitions/
├── tests/
│   ├── definitions/
│   ├── workflows/
│   ├── golden/
│   └── images/
│
└── legacy-electron/          Temporary home during migration
```

The dependency direction should remain strict:

```text
bite-schema
    ↑
bite-expr
    ↑
bite-core ← bite-imagemagick
   ↑   ↑
   │   └──────── bite-gui
   │
bite-cli
```

Neither the GUI nor CLI should contain pipeline logic. Both should call `bite-core` directly.

---

# JSON format v2

## 1. Add explicit schema versions

Every node definition should identify both its document format and the version of the node itself:

```json
{
  "schema_version": 2,
  "id": "posterize",
  "version": "1.0.0"
}
```

These fields have different meanings:

| Field | Meaning |
|---|---|
| `schema_version` | Version of the JSON document structure |
| `version` | Version of this particular node or format definition |

Use `schema_version` for node definitions, format definitions, and workflow files.

## 2. Replace implementation fields with one tagged structure

Replace `command_template`, `command_js`, `compute_js`, and the current top-level `executor` mechanism with a single `implementation` field.

### ImageMagick node

```json
{
  "implementation": {
    "type": "imagemagick",
    "args": [
      "-posterize",
      { "expr": "levels" }
    ]
  }
}
```

### Pure compute node

```json
{
  "implementation": {
    "type": "compute",
    "outputs": {
      "result": "a + b"
    }
  }
}
```

### Complex native node

```json
{
  "implementation": {
    "type": "native",
    "executor": "channel_split"
  }
}
```

This gives every node one clear implementation entry point while retaining three intentional execution tiers.

## 3. Make ImageMagick arguments token-based

Every string in `args` represents exactly one process argument:

```json
{
  "implementation": {
    "type": "imagemagick",
    "args": [
      "-blur",
      { "expr": "format('0x{}', sigma)" }
    ]
  }
}
```

Another example:

```json
{
  "implementation": {
    "type": "imagemagick",
    "args": [
      "-crop",
      { "expr": "format('{}x{}+{}+{}', width, height, x, y)" },
      "+repage"
    ]
  }
}
```

There is no shell parser, quoting convention, whitespace splitting, or opportunity for one expression to inject several arguments accidentally.

## 4. Support conditional argument groups

```json
{
  "implementation": {
    "type": "imagemagick",
    "args": [
      {
        "when": "attenuate != 1",
        "args": [
          "-attenuate",
          { "expr": "attenuate" }
        ]
      },
      "+noise",
      { "expr": "noise_type" }
    ]
  }
}
```

For `attenuate = 1`, this resolves to:

```text
+noise Gaussian
```

For `attenuate = 2.5`, it resolves to:

```text
-attenuate 2.5 +noise Gaussian
```

## 5. Add declarative switching

Enums often map to substantially different command arguments. Represent that directly:

```json
{
  "switch": "mode",
  "cases": {
    "percent": [
      { "expr": "format('{}%', scale)" }
    ],
    "pixels": [
      { "expr": "format('{}x{}', width, height)" }
    ]
  },
  "default": []
}
```

This should replace much of the current `command_js` without making the expression language responsible for argument-list construction.

## 6. Keep the expression language deliberately narrow

Expressions should resemble shader expressions or spreadsheet formulas, not programs.

Valid examples:

```text
width / 2
width == 0
clamp(exposure * 2, 0, 1)
if(lossless, 100, quality)
format("{}x{}", width, height)
image.width / image.height
```

The language must not support:

```text
let
const
var
assignment
for
while
function
return
import
require
```

It must have no implicit access to:

- Filesystem
- Environment variables
- Processes
- Network
- Clock or time
- Randomness
- Application state

Values such as image metadata must be explicitly injected into the evaluation context.

### Initial operators

```text
+  -  *  /  %
==  !=
<  <=  >  >=
&&  ||  !
()
```

Avoid adding operators until an actual migrated definition requires them.

### Initial built-ins

```text
abs
min
max
clamp

floor
ceil
round

sqrt
pow

sin
cos
tan

lerp

str
int
float
bool

format
if

length
normalize
dot

vec2
vec3
vec4
```

Vectors should expose named components:

```text
v.x
v.y
v.z
v.w
```

Do not add a general-purpose array language unless a compelling real-world use case appears.

### Suggested core value types

```rust
enum ExprValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Vector(Vec<f64>),
}
```

### Suggested argument schema

```rust
enum ArgSpec {
    Literal(String),

    Expression {
        expr: Expression,
    },

    Conditional {
        when: Expression,
        args: Vec<ArgSpec>,
    },

    Switch {
        expr: Expression,
        cases: HashMap<String, Vec<ArgSpec>>,
        default: Vec<ArgSpec>,
    },
}
```

Argument resolution then remains small and independent of ImageMagick semantics:

```rust
fn resolve_args(
    specs: &[ArgSpec],
    context: &Context,
) -> Result<Vec<String>> {
    // Recursively evaluate specs.
}
```

## 7. Parse expressions once and evaluate them many times

The runtime should compile expressions as definitions are loaded:

```text
load node JSON
        ↓
parse expressions
        ↓
AST
        ↓
validate and type-check
        ↓
cache compiled AST
        ↓
evaluate for each image
```

Never reparse expressions for every image.

Static validation should report:

- Unknown identifiers, with spelling suggestions
- Invalid function names
- Incorrect function arity
- Incompatible operand types
- Invalid member access
- Excessive expression length
- Excessive AST depth

Example:

```text
Unknown identifier "widht".
Did you mean "width"?
```

## 8. Infer metadata dependencies automatically

Remove `needs_image_meta` from v2 definitions. Infer required metadata from expression references:

```json
{
  "implementation": {
    "type": "compute",
    "outputs": {
      "aspect_ratio": "image.width / image.height"
    }
  }
}
```

The compiled expression records that `image.width` and `image.height` are required.

Distinguish inexpensive metadata that can often be read without `magick identify`:

```text
image.path
image.name
image.extension
image.size
image.width
image.height
image.format
```

from heavier metadata:

```text
image.bit_depth
image.dpi_x
image.dpi_y
image.exif.*
```

This preserves BITE's fast-path metadata behavior while removing a fragile manual flag.

## 9. Put visibility and enabled expressions on parameters

Replace separate `params_visibility` rules with expressions next to the affected parameter:

```json
{
  "name": "webp_quality",
  "label": "Quality",
  "type": "int",
  "widget": "slider",
  "default": 85,
  "min": 1,
  "max": 100,
  "visible_when": "!webp_lossless"
}
```

Also support:

```json
{
  "enabled_when": "some_expression"
}
```

The same expression engine can then control:

- Computed outputs
- Command arguments
- Format arguments
- Parameter visibility
- Parameter enabled state

## 10. Give ports stable names

Image ports should be identified by stable names rather than positional IDs such as `in-0` and `out-2`.

```json
{
  "inputs": [
    {
      "name": "input",
      "type": "image",
      "label": "Input"
    }
  ],
  "outputs": [
    {
      "name": "output",
      "type": "image",
      "label": "Output"
    }
  ]
}
```

Channel Split:

```json
{
  "outputs": [
    { "name": "r", "type": "image", "label": "R" },
    { "name": "g", "type": "image", "label": "G" },
    { "name": "b", "type": "image", "label": "B" },
    { "name": "a", "type": "image", "label": "A" }
  ]
}
```

Edges then reference ports semantically:

```text
out:r
in:input
```

Stable names make node-definition evolution safer and workflows easier to inspect.

## 11. Use the same argument system for format definitions

Remove the separate `args_js` mechanism. Format definitions should use the same `ArgSpec` parser, expression evaluator, validation, and tests as node definitions.

Example WebP definition:

```json
{
  "schema_version": 2,
  "id": "WEBP",
  "version": "1.0.0",
  "extension": ".webp",
  "params": [
    {
      "name": "webp_lossless",
      "label": "Lossless",
      "type": "bool",
      "widget": "checkbox",
      "default": false
    },
    {
      "name": "webp_quality",
      "label": "Quality",
      "type": "int",
      "widget": "slider",
      "default": 85,
      "min": 1,
      "max": 100,
      "visible_when": "!webp_lossless"
    }
  ],
  "args": [
    {
      "when": "webp_lossless",
      "args": [
        "-define",
        "webp:lossless=true"
      ]
    },
    {
      "when": "!webp_lossless",
      "args": [
        "-quality",
        { "expr": "webp_quality" }
      ]
    }
  ]
}
```

Format definitions should also become loose, runtime-loaded JSON rather than build-time bundled data.

## 12. Introduce workflow file v2

```json
{
  "schema_version": 2,
  "created_with": "1.0.0",
  "graph": {
    "nodes": [],
    "edges": [],
    "viewport": {
      "x": 0,
      "y": 0,
      "zoom": 1
    }
  }
}
```

`schema_version` determines how to deserialize and migrate the file. `created_with` is informational and useful for diagnostics. Application version should no longer be the primary compatibility mechanism for workflow files.

---

# Migration phases

## Phase 0 — Freeze current behavior

### Goal

Create a reliable reference for what BITE does today. Do not port anything yet.

### Work

Build a characterization suite around the existing application and capture:

- Every JSON node definition
- Every format definition
- Representative `.bite` workflows
- Generated ImageMagick argument arrays
- Pure-node results
- File naming results
- Graph traversal results
- Format-conversion arguments
- Multi-output behavior
- Set-processing behavior
- Gate behavior
- Properties nodes
- CLI argument handling

Extend the current workflow fixtures and unit tests rather than starting a disconnected test system.

Create machine-readable golden fixtures:

```text
tests/golden/nodes/posterize.json
tests/golden/nodes/add_noise.json
tests/golden/formats/webp.json
tests/golden/workflows/simple_resize.json
```

Example:

```json
{
  "node": "add_noise",
  "params": {
    "noise_type": "Impulse",
    "attenuate": 2.5
  },
  "expected_args": [
    "-attenuate",
    "2.5",
    "+noise",
    "Impulse"
  ]
}
```

### Test at the end

```bash
npm test
npm run test:golden
```

### Exit gate

The existing Electron application plus the golden suite constitutes a known-good behavioral reference. No Rust code is required yet.

---

## Phase 1 — Rust workspace and schema parser

### Goal

Rust can understand BITE's new data formats without processing an image.

### Create

```text
bite-schema
bite-cli
```

Initially, the CLI only needs developer commands:

```text
bite validate-node file.json
bite validate-format file.json
bite validate-workflow file.bite
```

Implement serde structures for:

```text
NodeDefinition
ParamDefinition
PortDefinition
FormatDefinition
Workflow
GraphNode
GraphEdge
Viewport
```

Create formal JSON Schema files:

```text
schemas/node-definition-v2.schema.json
schemas/format-definition-v2.schema.json
schemas/workflow-v2.schema.json
```

During migration, keep v2 definitions separate:

```text
node-definitions-v2/
format-definitions-v2/
```

Do not modify production v1 definitions yet.

### Test at the end

```bash
bite validate-node node-definitions-v2/posterize.json
```

Expected success:

```text
OK: posterize
```

Expected errors should be specific and contextual:

```text
param "sigma": slider requires min and max
output "foo": duplicate port name
```

### Exit gate

All proposed node, format, and workflow JSON structures deserialize and validate. ImageMagick is not involved yet.

---

## Phase 2 — Expression engine

### Goal

Finish the JavaScript replacement before touching pipeline execution.

### Create

```text
bite-expr
```

Pipeline:

```text
source string
    ↓
lexer
    ↓
parser
    ↓
AST
    ↓
static validation and type checking
    ↓
validated AST
    ↓
evaluation
```

Expressions compile once when a definition loads and are not reparsed for each image.

Add:

- Unknown-identifier detection
- Spelling suggestions
- Function arity checking
- Basic static type checking
- Expression-length limit
- AST-depth limit
- Deterministic evaluation
- Clear source-span diagnostics

Add a CLI evaluation tool:

```bash
bite expr \
  --params '{"width":1024,"height":512}' \
  'width / height'
```

Expected output:

```text
2
```

### Test at the end

Golden expression tests should cover:

- Arithmetic
- Booleans
- Strings
- `format()`
- `if()`
- Vectors and member access
- Comparisons
- Invalid syntax
- Unknown identifiers
- Function errors
- Type errors
- Complexity limits

### Exit gate

The engine can represent every existing `command_js`, `compute_js`, and `args_js` use case that belongs in a declarative expression system.

Any case that cannot be represented cleanly must be classified as a native Rust executor now, rather than expanding the DSL into a scripting language.

---

## Phase 3 — Convert node and format definitions to v2

### Goal

Produce a complete v2 definition set while Electron continues to use v1.

### Automate straightforward conversions

Convert simple `command_template` entries mechanically.

Example:

```text
-posterize {{levels}}
```

becomes:

```json
[
  "-posterize",
  { "expr": "levels" }
]
```

Template fragments such as:

```text
{{width}}x{{height}}
```

become:

```text
format("{}x{}", width, height)
```

### Convert complex definitions manually

Explicitly rewrite:

```text
command_js
compute_js
args_js
```

Do not build a JavaScript parser or transpiler for this migration. Manual conversion is more reviewable and forces each definition to validate the new schema.

### Convert visibility rules

Transform `params_visibility` rules into `visible_when` expressions attached to individual parameters.

### Test at the end

For every definition:

```text
v1 parameters
    ↓
existing TypeScript implementation
    ↓
golden result

v2 parameters
    ↓
Rust evaluator
    ↓
same golden result
```

Test default, boundary, conditional, enum, and invalid values—not only the happy path.

### Exit gate

Every node and format definition has a validated v2 counterpart that produces equivalent arguments or computed results.

This is milestone **M1 — Definition parity**.

---

## Phase 4 — Graph and workflow core in Rust

### Goal

Rust understands complete `.bite` workflows.

### Create

```text
bite-core
```

Port pure graph and workflow logic first:

- Topological sort
- Cycle detection and rejection
- Connection validation
- Port/type compatibility
- Graph traversal
- Ancestor tracing
- Descendant tracing
- Parameter resolution
- Parameter-wire resolution
- Workflow loading
- Workflow migration

### Workflow migration

Rust should read a v1 `.bite` file and transform it into an in-memory `WorkflowV2` representation.

Do not rewrite the original file immediately. Save as v2 only when the user explicitly saves from the new application or invokes a migration command.

### Add inspection commands

```bash
bite inspect workflow.bite
bite validate workflow.bite
```

Example inspection output:

```text
Workflow schema: 1 → migrated to 2

Nodes: 14
Edges: 17

Inputs:
  input-1

Outputs:
  output-image-1

Execution order:
  input
  resize
  sharpen
  format_convert
  image_output
```

### Test at the end

- Load every existing workflow fixture.
- Compare node and edge counts.
- Compare graph topology and execution order.
- Validate identical rejection of cycles and incompatible connections.
- Test v1-to-v2 migration without modifying the source file.

### Exit gate

All existing test workflows load successfully, and Rust's graph topology and validation behavior match the current implementation.

---

## Phase 5 — Rust ImageMagick command planner

### Goal

Rust can turn an entire workflow into an execution plan without executing it.

### Create

```text
bite-imagemagick
```

Implement:

- ImageMagick binary discovery
- Argument generation
- Format conversion
- Output extension logic
- Operation-fusion planning
- Environment configuration
- Cross-platform process specifications

Add:

```bash
bite plan workflow.bite
```

Example:

```text
Image 1

magick input.png
  -resize 1024x1024
  -sharpen 0x1
  -quality 85
  WEBP:output.webp
```

### Port structural native executors

At this stage, implement special behaviors that should not enter the expression DSL:

```text
gate
rename
channel_split
channel_merge
mean_value
solid_image
format_convert
process_as_set
```

Also classify and port any other multi-input, multi-output, or structural executor discovered during definition conversion.

### Test at the end

Compare Rust-generated execution plans against Phase 0 golden fixtures, including operation order, exact argument tokenization, output paths, format arguments, and native-executor plan structure.

### Exit gate

Rust produces equivalent ImageMagick argument streams and structural plans for every supported workflow, without invoking ImageMagick.

---

## Phase 6 — Functional Rust CLI

### Goal

Create the first major user-facing Rust milestone: a CLI that can run real workflows.

### CLI surface

```text
bite run workflow.bite
bite validate workflow.bite
bite inspect workflow.bite
bite plan workflow.bite

bite nodes
bite formats
bite version
```

Preserve the named input and output flag concept derived from workflow input and output nodes:

```bash
bite run workflow.bite \
  --input-1 ./photos \
  --output-image-1 ./out
```

### Implement in two passes

First:

- Single input
- Single output
- Linear graph
- Pure compute nodes
- Normal ImageMagick nodes
- Format conversion

Then:

- Multiple inputs
- Multiple outputs
- Gate
- Rename
- Text output
- Channel operations
- Set processing
- Flipbook output

### Test at the end

Run the same workflows through the legacy Node/Electron backend and the Rust CLI.

For lossless transforms, use exact pixel comparison.

For lossy codecs, compare:

- Dimensions
- Output format
- Relevant metadata
- File count and names
- A documented perceptual or pixel-difference threshold

Also compare failure behavior, cancellation, partial outputs, and diagnostic clarity.

### Exit gate

The Rust CLI can replace the existing Node CLI for real work.

This is milestone **M2 — CLI parity**.

---

## Phase 7 — Restore and verify performance optimizations

### Goal

Regain or improve the performance characteristics of the current BITE backend rather than stopping at functional parity.

### Implement

Native Rust header readers for:

```text
PNG
JPEG
WEBP
BMP
TGA
```

Then implement:

- Thumbnail generation
- Thumbnail disk cache
- Metadata memory cache
- Cache expiry and invalidation
- Parallel folder scanning
- Batched ImageMagick spawning
- Operation fusion
- Multi-stream channel path
- Set processing
- Per-process `MAGICK_THREAD_LIMIT`
- Cancellation
- Progress reporting
- Bounded worker pool

Make concurrency configurable:

```bash
bite run workflow.bite --jobs 16
```

Choose a safe platform-aware default rather than directly copying an existing high concurrency number.

### Benchmark suite

Provide reproducible commands or scripts such as:

```text
bite bench-import
bite bench-workflow
```

Track:

- Cold and warm startup time
- Folder-scan time
- Thumbnail generation time
- Preview latency
- Batch throughput
- Peak memory
- Number of ImageMagick processes spawned
- Cache hit rate
- Cancellation latency

Use the existing approximately 2,000-image mixed-format workload as a primary regression benchmark, alongside smaller deterministic fixtures suitable for CI.

### Test at the end

Run the same benchmark matrix against the existing implementation and the Rust implementation on Windows and macOS.

### Exit gate

The Rust backend is at least comparable to the Node implementation for representative real workloads and has no major performance regressions.

This completes milestone **M3 — Performance parity** and the backend migration.

---

## Phase 8 — Native GUI technical prototype

### Goal

Prove Dear ImGui and the selected Rust bindings are suitable before rebuilding the entire application.

Do not recreate BITE yet. Build only a technical prototype containing:

- Native window
- Dockspace
- Node editor
- Inspector
- Image texture view
- Filmstrip
- File drag and drop

Prototype with:

```text
Dear ImGui docking
Winit
WGPU
```

Evaluate the binding and node-editor libraries here rather than locking them in earlier.

### Prototype workload

Use ten hardcoded fake node types and test:

- 100+ visible nodes
- Zoom and pan
- Wire dragging
- Node selection
- Multi-select
- Docked panels
- 4K and high-DPI displays
- Retina macOS
- Image upload and texture updates
- File drop
- Keyboard focus
- Text input
- Multiple monitors

### Test at the end

Manually use the fake editor on Windows and macOS. Record performance, DPI, focus, input, and rendering issues.

### Exit gate

Continue only if:

- Node editing feels responsive and predictable.
- DPI behavior is correct.
- Image preview is performant.
- Text editing is acceptable.
- The chosen binding stack is maintained and practical.

This is the major GUI go/no-go checkpoint.

---

## Phase 9 — Functional native BITE GUI

### Goal

Build the smallest native GUI that performs real work.

The GUI should call `bite-core` directly—not spawn the CLI as an intermediary.

```text
             bite-core
             /       \
        bite-cli    bite-gui
```

### First feature set

- Open workflow
- Save workflow
- Node library
- Graph editor
- Inspector
- Image import
- Filmstrip
- Image preview
- Run workflow
- Progress display
- Cancellation

Visual polish is secondary in this phase.

### Preview architecture

Use a direct library call:

```rust
bite_core::preview(...)
```

Do not implement preview as:

```text
GUI
 ↓
spawn bite CLI
 ↓
spawn ImageMagick
```

### Test at the end

Create this workflow entirely inside the native GUI:

```text
Input
 ↓
Resize
 ↓
Sharpen
 ↓
Convert Format
 ↓
Image Output
```

Then:

1. Preview it.
2. Save it.
3. Close BITE.
4. Reopen it.
5. Run it over a folder.
6. Compare results against the Rust CLI.

### Exit gate

The native GUI supports BITE's primary end-to-end workflow.

---

## Phase 10 — Editor parity

### Goal

Port the accumulated workflow-editor behavior and make the native build suitable for daily development.

Treat current editor behavior as requirements, not incidental Svelte implementation details.

### Graph interactions

- Typed wires
- Single-input replacement
- Cycle prevention
- Wire-drop creation menu
- Context-aware node creation
- Duplicate
- Delete
- Copy and paste

### History

- Undo
- Redo
- Transaction grouping

### Organization

- Groups
- Comments
- Resizable groups
- Group movement

### Inspector

- All existing widgets
- `visible_when`
- `enabled_when`
- Read-only parameters
- `portOnly`
- `noPort`

### Workflow features

- Multiple input nodes
- Multiple outputs
- Text output
- Flipbook output
- Process As Set
- CLI names

### Test at the end

Create a parity checklist by inspecting the existing application. Do not rely on memory.

Check every interaction in both Windows and macOS builds, with explicit test cases for undo boundaries, keyboard focus, file dialogs, drag and drop, graph persistence, and high-DPI behavior.

### Exit gate

The native build can replace Electron for normal BITE development and routine production use.

This is milestone **M4 — GUI parity**.

---

## Phase 11 — Packaging and cutover

### Goal

Ship native BITE while preserving installation, bundled dependencies, file associations, and platform conventions.

### Windows

Package:

```text
bite.exe
node-definitions/
format-definitions/
ImageMagick/
licenses/
```

Prefer one binary if practical:

```text
bite.exe
```

- Double-click launches the GUI.
- Terminal subcommands such as `bite run` launch CLI behavior.

If one binary causes platform or console-window problems, use:

```text
bite.exe
bite-cli.exe
```

### macOS

Bundle:

```text
BITE.app
└── Contents/
    ├── MacOS/bite
    └── Resources/
        ├── node-definitions/
        ├── format-definitions/
        └── ImageMagick/
```

Retain:

- Code signing
- Notarization
- DMG creation
- `.bite` file association
- Existing ImageMagick relocation and bundling behavior

### Test at the end

Test on clean machines:

```text
Windows 11
macOS Apple Silicon
```

Verify:

- Installation
- First launch
- `.bite` association
- Drag and drop
- CLI availability or PATH setup
- ImageMagick discovery
- Workflow execution
- Upgrade behavior
- Uninstallation

### Exit gate

The native release behaves like a normal installed application and does not require Node.js.

---

## Phase 12 — Remove JavaScript and Electron

### Goal

Remove the legacy production runtime only after all earlier exit gates pass.

Create a final reference tag or branch such as:

```text
electron-final
```

Then remove:

```text
Electron
Node.js runtime
Svelte
Vite
TypeScript production runtime
npm production dependencies
command_js
compute_js
args_js
command_template
legacy IPC
```

Promote the v2 definitions:

```text
node-definitions-v2   → node-definitions
format-definitions-v2 → format-definitions
```

Make schema v2 canonical while retaining supported workflow migration code.

### Test at the end

Search the production source tree for:

```text
command_js
compute_js
args_js
new Function
electron
svelte
node_modules
```

No legacy production references should remain.

Perform a clean build using only:

- Rust toolchain
- Platform SDK/toolchain
- ImageMagick packaging prerequisites

### Final exit gate

BITE builds, installs, opens, previews, and runs representative workflows on clean Windows and macOS systems without Node.js installed.

---

# What should remain native

Do not try to express every operation in JSON.

| Behavior | Implementation |
|---|---|
| `-blur 0xN` | JSON argument specification |
| Conditional ImageMagick flag | JSON plus expression |
| Calculated ImageMagick value | JSON plus expression |
| Math/value node | JSON plus expression |
| Inspector visibility | Expression |
| Enum argument mapping | JSON `switch` |
| Channel Split | Native Rust |
| Channel Merge | Native Rust |
| Process As Set | Native Rust |
| Multi-image operation | Native Rust |
| Graph structural behavior | Native Rust |
| File rename pipeline | Probably native Rust |
| Flipbook construction | Native Rust |
| Caching | Native Rust |
| Parallel scheduling | Native Rust |

Use this three-tier boundary:

```text
Simple substitution
    ↓
Tokenized JSON arguments

Conditional or computed declarative behavior
    ↓
BITE expressions plus argument schema

Complex graph or image behavior
    ↓
Native Rust executor
```

---

# Milestones

| Milestone | Definition of done |
|---|---|
| **M1 — Definition parity** | Rust validates and evaluates all v2 nodes and formats identically to the current implementation. |
| **M2 — CLI parity** | The Rust CLI processes existing `.bite` workflows correctly and can replace the Node CLI for real work. |
| **M3 — Performance parity** | The Rust backend matches or exceeds the current backend on representative batches without major regressions. |
| **M4 — GUI parity** | The native Dear ImGui build can replace Electron for normal use. |

The required order is:

```text
M1 → M2 → M3 → M4
```

Do not begin the full GUI rebuild before M2. Once the Rust CLI runs real workflows, the hardest architectural risk has been retired and the GUI becomes a frontend project instead of a backend and frontend rewrite happening simultaneously.

---

# Recommended working rules

1. Keep the Electron build usable until the native application reaches parity.
2. Use Phase 0 goldens as the compatibility contract throughout the migration.
3. Do not implement the expression language in TypeScript and then reimplement it in Rust.
4. Compile and validate definitions at load time; evaluate cached ASTs at runtime.
5. Keep expressions deterministic, bounded, and free of side effects.
6. Add DSL capabilities only when a real migrated definition requires them.
7. Keep graph topology, filesystem work, process execution, caching, and concurrency in Rust.
8. Preserve v1 workflow loading until a deliberate compatibility policy says otherwise.
9. Benchmark with representative real data before and after every performance-sensitive change.
10. Make every phase pass its exit gate before expanding scope.

This sequence ensures there is never a point where the working Electron application must be abandoned before its Rust replacement is genuinely useful.
