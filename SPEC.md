## 1. Overview

A batch image workflow builder application built around a visual node graph editor powered by imagemagick. Users compose pipelines by connecting nodes (image transforms, logic, value constants, math), preview results in real-time, and batch-process hundreds of images through the pipeline.

**Comparable product:** Retrobatch (Mac-only) - but cross-platform, more extensible, and CLI-exportable.

### Core Features

- **Visual node graph editor** — chain image operations by connecting nodes
- **Filmstrip view** — thumbnail strip of queued images along the bottom
- **Real-time preview** — selected image rendered through the current node network, node-level cached
- **JSON-based node definitions** — add new nodes without recompiling
- **CLI command export** — export any graph as an ImageMagick shell script (PS / Bash / CMD)
- **Workflow save/load** — serialize graphs as `.imgplex` JSON files; double-clicking a `.imgplex` file opens it directly in the app
- **Batch processing** — all images through the pipeline at full resolution, progress-reported
- **Pure-value node graph** — math, logic, and value nodes with typed wires route parameters without touching the image pipeline
- **Node groups** — visually organise sets of nodes into resizable labelled containers
- **Comment nodes** — free-form canvas annotations

### UI Layout

```
┌──────────────┬───────────────────────────┬──────────────────┐
│              │                           │    Inspector     │
│  Node        │     Node Graph Editor     │  (parameters of  │
│  Library     │     (@xyflow/svelte)      │  selected node)  │
│  (sidebar)   │                           ├──────────────────┤
│              │                           │  Image Preview   │
│              │                           │  (real-time)     │
├──────────────┴───────────────────────────┴──────────────────┤
│                    Filmstrip (thumbnails)                   │
└─────────────────────────────────────────────────────────────┘
```

---

## 2. Tech Stack

| Layer | Technology | Notes |
|---|---|---|
| Desktop shell | Electron 30 | Window management, file dialogs, auto-update, bundling |
| Build | Vite 5 + vite-plugin-electron | Hot-reload dev, separate renderer/main/preload bundles |
| UI framework | Svelte 5 (runes) + TypeScript | Entire UI — panels, filmstrip, inspector, preview |
| Node graph editor | @xyflow/svelte 1.x (Svelte Flow) | Custom node components, minimap, controls |
| Image processing | ImageMagick v7 CLI (`magick`) | All image operations; no Sharp / no LGPL deps |
| Packaging | electron-builder (NSIS on Windows) | NSIS installer patched via `scripts/patch-nsis.cjs` postinstall to show file details |
| Language | TypeScript throughout | Main process, renderer, shared types |

### Why These Choices

**Electron over Tauri:** Three blockers emerged during research: (1) Linux WebKitGTK instability — a Tauri maintainer stated they "can't 100% recommend Tauri for Linux"; (2) IPC bottleneck — serialising image data as strings benchmarked at 200ms per 3MB; (3) sidecar lifecycle complexity. Electron eliminates all three: consistent Chromium everywhere, Node.js built in, and battle-tested packaging.

**@xyflow/svelte over LiteGraph.js:** LiteGraph (used by ComfyUI) is canvas-based and handles 200+ nodes, but its original repo is 3+ years unmaintained and ComfyUI uses a divergent fork. @xyflow/svelte is MIT-licensed, actively maintained, has native Svelte integration, and DOM performance is never the bottleneck for image processing graphs (typically 5–30 nodes).

**Svelte 5 over React:** Less boilerplate, no virtual DOM, `$state.raw` rune for performance-conscious reactivity (required by @xyflow/svelte), and native integration with Svelte Flow.

---

## 3. Licensing

| Component | License |
|---|---|
| Electron | MIT |
| Svelte 5 | MIT |
| @xyflow/svelte | MIT |
| Vite / vite-plugin-electron | MIT |
| electron-builder | MIT |
| TypeScript | Apache 2.0 |
| @yao-pkg/pkg | MIT |
| ImageMagick | ImageMagick License (Apache-2.0 derived — attribution required) |
| JetBrains Mono | SIL OFL 1.1 |
| Atkinson Hyperlegible | SIL OFL 1.1 |

**Avoided:** Sharp/libvips (LGPLv3), Bun (statically links JavaScriptCore, LGPL-2).

---

## 4. Architecture

### 4.1 Process Model

```
Electron Main Process (Node.js)
  ├── Pipeline Engine (executor.ts)
  ├── Node Registry   (registry.ts)
  └── IPC Handlers    (handlers.ts)
        ↕ Electron IPC (contextBridge + ipcMain/ipcRenderer)
Renderer Process (Chromium)
  └── Svelte 5 Application
        ├── NodeEditor  (@xyflow/svelte canvas)
        ├── NodeLibrary (sidebar)
        ├── Inspector   (param editor)
        ├── Preview     (live image)
        └── Filmstrip   (thumbnails)
```

### 4.2 Folder Structure

```
project-root/
├── electron/
│   ├── main.ts            — App bootstrap, BrowserWindow, native menus, IPC registration
│   └── preload.ts         — contextBridge exposes window.ipcRenderer (invoke/on/off/send)
├── src/
│   ├── shared/
│   │   ├── types.ts       — NodeDefinition, GraphNode, GraphEdge, NodeGraph, ImageInfo, PipelineService, Progress
│   │   ├── constants.ts   — IPC channel names (IPC.*), PREVIEW_MAX_EDGE_PX, WORKFLOW_EXTENSION, APP_NAME
│   │   └── renameUtils.ts — Filename token expansion for Rename node
│   ├── main/              — Node.js / Electron main-process business logic
│   │   ├── nodes/
│   │   │   └── registry.ts          — Loads and watches node-definitions/*.json; hot-reloads in dev
│   │   ├── pipeline/
│   │   │   ├── executor.ts          — Topological sort, preview/batch execution, ImageMagick spawning, temp file cache
│   │   │   ├── executor-compute.ts  — Pure-value node computation (math, logic, type conversion, properties)
│   │   │   ├── executor-cli.ts      — CLI script generators (PowerShell / Bash / CMD)
│   │   │   ├── command-builder.ts   — Per-node ImageMagick argument builders
│   │   │   ├── cache.ts             — In-memory preview cache keyed by (nodeId, inputHash)
│   │   │   └── magick-path.ts       — Resolves portable or PATH-installed magick binary
│   │   └── ipc/
│   │       └── handlers.ts          — IPC handler registration for registry, pipeline, dialogs, shell, workflows
│   └── renderer/
│       ├── main.ts        — Svelte app mount entry point
│       ├── App.svelte     — 3-panel resizable layout; workflow save/load; menu IPC listeners
│       ├── assets/
│       │   ├── theme.css  — CSS custom properties (colors, typography, layout sizes, port colors)
│       │   └── fonts.css  — @font-face for JetBrainsMono-Regular and AtkinsonHyperlegibleNext-Regular
│       ├── components/
│       │   ├── NodeLibrary.svelte       — Categorised node list with search filter; drag-to-canvas
│       │   ├── Inspector.svelte         — Dispatches to per-node inspector based on selected type
│       │   ├── InspectorInputNode.svelte    — Folder import workflow + individual image add
│       │   ├── InspectorOutputNode.svelte   — Output path, overwrite mode, and text-output mode (file path, separator, port order, preview, write)
│       │   ├── InspectorParamEditor.svelte  — Dynamic param widgets for ProcessNode
│       │   ├── InspectorCommentNode.svelte      — Comment node text editing
│       │   ├── InspectorRenameNode.svelte       — Rename node block editor
│       │   ├── InspectorResizeNode.svelte       — Resize node inspector (mode/preserve switcher, dynamic port visibility)
│       │   ├── InspectorTextOutputNode.svelte   — Text output node inspector
│       │   ├── InspectorFolderPathNode.svelte   — Folder path node inspector
│       │   ├── Preview.svelte           — Live preview image; triggers pipeline re-run on node/image change
│       │   ├── Filmstrip.svelte         — Horizontal thumbnail strip; click-to-select
│       │   ├── Dropdown.svelte          — Reusable styled dropdown
│       │   ├── ColorPicker.svelte       — HSL/hex color picker
│       │   ├── MenuBar.svelte           — Custom menubar (File / Help) rendered in renderer
│       │   ├── BatchSummaryModal.svelte — Post-batch summary dialog (processed/skipped/failed + open output folder)
│       │   ├── AboutModal.svelte        — About dialog (version, links to GitHub)
│       │   └── CreditsModal.svelte      — Credits modal (open source deps + fonts) with system-browser links
│       ├── nodeEditor/
│       │   ├── NodeEditor.svelte        — Svelte Flow canvas; drag-drop, context menu, undo/redo, grouping
│       │   ├── ProcessNode.svelte       — Standard processing node (image ports, param rows, bypass tick)
│       │   ├── InputNode.svelte         — Source node (image count footer)
│       │   ├── OutputNode.svelte        — Sink node; Image mode (output path, overwrite) or Text mode (dynamic value ports, write per-image params to a .txt file)
│       │   ├── FolderPathNode.svelte    — Folder path source node (path port output)
│       │   ├── GroupNode.svelte         — Resizable labelled container; children move with group
│       │   ├── CommentNode.svelte       — Resizable sticky note with editable text
│       │   ├── ColoredEdge.svelte       — Type-coloured wire rendering
│       │   ├── NodeContextMenu.svelte   — Right-click creation/action menu (wire-drop + canvas)
│       │   ├── DropHelper.svelte        — Invisible overlay for drag-drop coordinate capture
│       │   ├── portColors.ts            — Wire color map keyed by data type
│       │   ├── wireTypeUtils.ts         — Wire-type compatibility, paramTypeToWireType, paramInHandle/paramOutHandle
│       │   ├── nodeEditorHelpers.ts     — buildNodeData, expandNodeData, buildResizeParamDefs, sortNodesGroupFirst, firstMatchingHandle
│       │   └── nodeEnabledState.ts      — Bypass state helpers
│       ├── stores/
│       │   ├── graph.svelte.ts          — Node graph state (Svelte 5 runes)
│       │   └── images.svelte.ts         — Image list, selection, and streaming import state
│       ├── services/
│       │   └── pipeline-service.ts      — ElectronPipelineService: IPC bridge for batch execution
│       ├── platform.ts                  — IS_ELECTRON flag (detects Electron vs browser context)
│       └── browserIpc.ts               — Browser shim for window.ipcRenderer (workflow builder without Electron)
├── src/tests/             — Vitest unit tests for pure-function modules
│   ├── wireTypeUtils.test.ts
│   ├── nodeEditorHelpers.test.ts
│   ├── executor-compute.test.ts
│   ├── renameUtils.test.ts
│   └── topoSort.test.ts
├── node-definitions/      — JSON node descriptor files (loaded at runtime, hot-reloaded in dev)
├── workflows/             — User workflow save files (.imgplex JSON)
├── scripts/
│   └── patch-nsis.cjs     — Postinstall script: patches electron-builder NSIS templates to show install file details
└── resources/             — Bundled assets (portable ImageMagick, app icon)
```

---

## 5. Node Definition Format

Each node is described by a JSON file in `node-definitions/`:

```json
{
  "id":          "node_id",
  "version":     "1.0.0",
  "label":       "Human Name",
  "description": "What this node does (shown as tooltip on hover)",
  "category":    "Category",
  "inputs":  [{ "type": "image|bool|float|…", "label": "Input" }],
  "outputs": [{ "type": "image|bool|float|…", "label": "Output" }],
  "executor": "executor_key",
  "params": [
    {
      "name": "param", "label": "Label",
      "type": "int|float|bool|string|enum|color|vector2|vector3|vector4|numeric",
      "widget": "slider|checkbox|text|dropdown|color|vector|number",
      "default": …, "min": …, "max": …, "step": …,
      "options": […],
      "readonly": true,
      "portOnly": true,
      "noPort": true
    }
  ]
}
```

### Param Field Reference

| Field | Description |
|---|---|
| `name` | Internal identifier; becomes the param key and handle ID suffix |
| `label` | Human-readable display name in the Inspector |
| `type` | Data type — controls wire color and type-checking |
| `widget` | Inspector control type |
| `default` | Initial value |
| `min` / `max` / `step` | Numeric constraints for slider and number widgets |
| `options` | Dropdown option list (required for `enum` type) |
| `readonly` | If true: right-side output handle, no input handle; display-only in Inspector for executor nodes |
| `portOnly` | If true: only a right-side port, no body row shown in node card |
| `noPort` | If true: body row only, no input handle (Inspector-only param) |

### Special Param Conventions

- The hidden `_enabled` bool param on every processing node is the **bypass port** — `false` passes image through unchanged
- `portOnly` params (e.g. vector component outputs) appear only as right-side handles with labels
- Param values can be wired from other nodes via the param-wire system

---

## 6. Node Type System

### Two Node Families

**Image nodes** — have at least one `image`/`mask` port in `inputs` or `outputs`. Executed by the pipeline engine as ImageMagick operations. Require either `command_template` or `executor`.

**Pure-value nodes** — empty `inputs` and `outputs` arrays. Never touch ImageMagick. Evaluated in topological order; resolved output values forwarded along param-wires to downstream nodes.

### Param-Wire System

- **Writable params** (no `readonly`) expose an input handle on the left.
- **Readonly params** expose an output handle on the right.
- Type compatibility enforced at connection time.
- **Single-input rule:** each input port accepts only one wire; a second connection displaces the first.
- **No cycles:** BFS rejects edges that would create feedback loops.

### Port / Wire Colors (WCAG AAA on node background `#212121`)

| Type | Color | Hex |
|---|---|---|
| `image` | Orange | `#ff8c3f` |
| `mask` | Purple | `#d8a4fc` |
| `number` / `int` / `float` | Cyan | `#22d3ee` |
| `string` | Green | `#22c55e` |
| `boolean` | Yellow | `#eab308` |
| `color` | Pink | `#fc86bc` |
| `vector2` | Amber | `#fb923c` |
| `vector3` | Indigo | `#a5b4fc` |
| `vector4` | Teal | `#2dd4bf` |
| `numeric` | Near-white | `#e2e8f0` |
| `any` | Slate | `#aaaaaa` |

### Available Node Categories

| Category | Nodes |
|---|---|
| **Transform** | Resize, Crop, Rotate, Flip |
| **Adjustments** | Brightness/Contrast, Levels, Grayscale, Hue Offset, Saturation, Negate |
| **Filters** | Blur, Sharpen |
| **Channels** | Channel Split, Channel Merge |
| **Format** | Format Convert |
| **Output** | Rename |
| **Logic** | Gate, Text Filter, Comparison, AND, OR, NOT, Branch |
| **Properties** | Name, Path, Dimensions, Size, Bit Depth, File Type, Resolution, EXIF |
| **Values** | Boolean, Float, String, Color, Vector2, Vector3, Vector4, Solid Image |
| **Math** | Add, Subtract, Multiply, Divide, Power, Lerp, Mean Value |
| **Vector** | Append Vec, Split Vec, Dot Product, Length, Normalize |
| **Graph** | Comment (annotation node), Group (visual container) |

---

## 7. Pipeline Engine

### 7.1 Execution Model

1. **Topological sort** (Kahn's algorithm) to determine execution order.
2. **Resolve parameters per node** — start from Inspector values, override with upstream wired values.
3. **Pure nodes** (no image ports): `computeNodeParams()` evaluates output values; skip ImageMagick.
4. **Image nodes**: build ImageMagick args from resolved params, execute `magick`, cache output.

### 7.2 Preview Pipeline

- Triggered on: node selection change, parameter edit, active image change, or after a graph edit.
- Graph trimmed to ancestors of the selected node (backward BFS via all edge types).
- Input downscaled proportionally to `PREVIEW_MAX_EDGE_PX = 1024px` before entering the pipeline.
- **Node-level caching:** output keyed by `(nodeId, hash(inputPath + params))`; only re-runs invalidated nodes.
- **Incremental invalidation:** when a node changes, it and all downstream nodes are cache-invalidated.
- **Debounce:** 80ms after last param change (0ms when no nodes present).
- **Temp file race guard:** concurrent preview requests for the same input don't conflict — write errors are caught and re-checked for existence.
- Single image: only the currently selected filmstrip image.

### 7.3 Import Pipeline

Importing images uses a multi-stage streaming pipeline optimised for Windows (where `spawn()` costs ~200ms):

1. **Streaming push model** — results are sent to the renderer one at a time as they complete via `pipeline:load-images-streaming-result` IPC pushes; the renderer adds each image to the filmstrip immediately rather than waiting for the whole batch.
2. **Concurrent workers** — `os.cpus().length` workers run in parallel, each claiming an atomic chunk of `BATCH_SIZE = 8` images at a time.
3. **Native header parsing** — PNG, BMP, WEBP, JPEG, and TGA dimensions are read directly from file headers in Node.js (no magick spawn). These "fast-path" images are classified and batched together.
4. **Batch magick spawn** — all fast-path images in a chunk are processed in one `magick` invocation using `-write <thumbN> +delete` sequencing. This amortises ~200ms process-spawn cost over 8 images instead of paying it per image.
5. **Slow-path fallback** — formats requiring magick for dimensions (PSD, TIFF, RAW, GIF, …) fall back to individual `-print %w %h %m` + thumbnail spawns; these are rare.
6. **In-memory metadata cache** — `_metaCache` on `PipelineExecutor` stores `{width, height, format}` per path for the session; re-importing the same folder hits the cache and skips all magick calls.
7. **Thumbnail disk cache** — thumbnails are written to `%TEMP%/imgplex-preview/thumb_<hash>_<size>.png` and reused if the source file has not changed (mtime comparison).

Result: ~1.6s for 1983 mixed PNG/TGA/JPG/PSD images (vs ~44s before batching).

### 7.4 Batch Pipeline

- Separate from preview — no caching, full resolution.
- **Parallel execution:** runs N concurrent pipelines via `Promise.all` where N = `concurrency` (derived from CPU count). `MAGICK_THREAD_LIMIT` is divided equally across workers to prevent ImageMagick's internal OpenMP pool from oversubscribing the CPU.
- **Fast path:** if no Properties nodes present, `buildOpArgsForImage` runs once to produce a shared plan applied to all images.
- **Slow path:** if Properties nodes present, plan re-evaluated per image with fresh `magick identify` metadata.
- **Multi-stream path:** activated when channel_split, channel_merge, or mean_value nodes are present. Uses a lazy-chain buffer model — consecutive standard ops accumulate into a `{base, args}` lazy chain; materialisation is deferred until a fork (multiple consumers) or a format change. This fuses multiple standard nodes into a single magick invocation.
- **Channel split optimisation:** all 4 channels extracted in one magick call via parenthesised `-write` branches. If all consumers are `mean_value` nodes, the channel PNGs are never written; per-channel mean is computed directly from the source.
- Gate node returns `null` plan → image silently skipped (no output file).
- Format Convert changes output extension and uses `FORMAT:path` prefix.
- Per-image errors are non-fatal; batch continues; error summary reported at end.
- Progress pushed to renderer via IPC event `pipeline:execute-batch:progress`.

### 7.5 Pure Node Executors (`computeNodeParams`)

| Executor key | Operation |
|---|---|
| `math_add` / `math_subtract` / `math_multiply` / `math_divide` | `result = a OP b` |
| `math_power` | `result = base ^ exponent` |
| `math_lerp` | `result = a + (b − a) × t` |
| `mean_value` | `result = mean(inputs)` |
| `logic_and` / `logic_or` / `logic_not` | Boolean operators |
| `logic_branch` | `result = condition ? value_true : value_false` |
| `logic_comparison` | `result = a {==, !=, >, <, >=, <=} b` |
| `text_filter` | `result = string matches(prefix/suffix/contains) pattern` |
| `split_vec` | `{x, y[, z[, w]]} = vec[0..N]` |
| `append_vec` | `vec = [x, y[, z[, w]]]` |
| `vec_math_dot` | `result = dot(a, b)` |
| `vec_math_length` | `result = length(v)` |
| `vec_math_normalize` | `result = v / length(v)` |
| `rename` | Builds filename from ordered blocks (text / number / original-name) |
| `prop_name` / `prop_path` / `prop_dimensions` / `prop_size` / `prop_bitdepth` / `prop_filetype` / `prop_resolution` / `prop_exif` | Per-image metadata from `magick identify` |
| _(no executor)_ | Value nodes — params are the output values directly |

### 7.6 CLI Export

- `IPC.EXPORT_CLI` saves two files side-by-side: a shell script and a companion `.imgplex` workflow file.
- The shell script calls `imgplex-cli run <workflow.imgplex> --input <dir> --output <dir>`.
- `imgplex-cli.exe` is installed alongside the app and added to the user's PATH by the NSIS installer — no separate install step required.
- Supports three shell targets: **PowerShell** (`.ps1`), **Bash** (`.sh`), **CMD** (`.bat`).
- Menu items: Help → Export CLI (PS / Bash / CMD); also exposed as keyboard shortcuts.

---

## 8. Node Editor Behaviour

| Action | Shortcut |
|---|---|
| Duplicate selected nodes | Ctrl+D |
| Delete selected nodes/edges | Delete / Backspace |
| Undo | Ctrl+Z |
| Redo | Ctrl+Y / Ctrl+Shift+Z |
| Group selected nodes | Ctrl+G |
| Ungroup selected group | Ctrl+Shift+G |
| Open context menu | Right-click canvas / Space / Tab |
| Zoom in / out | Ctrl+Scroll or +/- |
| Fit view | Ctrl+Shift+F |

Additional behaviours:
- **Single-input rule** — connecting to an occupied port displaces the existing wire
- **Cycle detection** — BFS prevents feedback loops
- **Type enforcement** — connection rejected if handle types are incompatible
- **Colored wires** — each data type has a distinct wire color; drag-preview wire matches source type
- **Wire-drop node creation** — dragging a wire onto empty canvas opens a filtered node menu; new node auto-connects to the correct typed handle
- **Context menu** — right-clicking empty canvas opens full node creation menu; right-clicking a group node offers ungroup
- **Node groups** — Ctrl+G wraps selected nodes in a resizable labelled container; children move with group and are constrained inside via `parentId` + `extent: 'parent'`; Ctrl+Shift+G restores children to absolute positions
- **Comment nodes** — resizable sticky note; text editable on double-click; no ports
- **Node tooltips** — hover header for 1s shows `description` field from node definition

---

## 9. Workflow Files

```json
{
  "version": "1.0",
  "graph": {
    "nodes": [ /* GraphNode[] — includes group nodes with width/height/parentId */ ],
    "edges": [ /* GraphEdge[] */ ],
    "viewport": { "x": 0, "y": 0, "zoom": 1 }
  }
}
```

- File extension: `.imgplex`
- Dirty-state tracking: unsaved changes prompt before close/new/open
- Group nodes saved with `width`, `height`; children saved with `parentId` and `extent: 'parent'`

---

## 10. IPC Channels

| Channel | Direction | Purpose |
|---|---|---|
| `registry:get-all` | invoke | Load all node definitions |
| `registry:updated` | push | Hot-reload notification |
| `pipeline:load-images` | invoke | Load image list with metadata (legacy; used by non-streaming paths) |
| `pipeline:load-images-with-thumbnails` | invoke | Load images + generate thumbnails in one call |
| `pipeline:load-images-streaming-start` | invoke | Begin streaming import; results pushed per image |
| `pipeline:load-images-streaming-result` | push | One loaded image result during streaming import |
| `pipeline:load-images-streaming-cancel` | invoke | Cancel an in-progress streaming import |
| `pipeline:generate-thumbnail` | invoke | Generate a single thumbnail (used for preview warm-up) |
| `pipeline:execute-preview` | invoke | Run preview pipeline |
| `pipeline:execute-batch` | invoke | Run batch pipeline |
| `pipeline:execute-batch:progress` | push | Progress updates during batch |
| `pipeline:export-cli` | invoke | Export CLI script (PS/Bash/CMD) |
| `dialog:open-images` | invoke | File open dialog for images |
| `dialog:open-folder` | invoke | Folder picker (no scan) |
| `dialog:scan-folder-dialog` | invoke | Pick folder via dialog then scan for matching extensions |
| `dialog:scan-folder` | invoke | Scan a given folder path for matching extensions (no dialog) |
| `text-output:browse` | invoke | Save-file dialog for text output path |
| `text-output:preview` | invoke | Compute text output lines without writing (preview) |
| `text-output:write` | invoke | Write text output file; sends per-image progress |
| `text-output:write-progress` | push | Progress during text output write |
| `text-output:write-cancel` | invoke | Cancel an in-progress text output write |
| `workflow:save` | invoke | Save workflow JSON |
| `workflow:load` | invoke | Load workflow JSON (shows open dialog) |
| `workflow:open-path` | invoke | Load workflow JSON from a given file path (no dialog) |
| `app:quit` | invoke | Graceful quit with dirty-state check |
| `app:open-file-path` | push | Main → renderer: carry file path from OS file-association open |
| `shell:open-external` | invoke | Open URL in default system browser |
| `shell:open-path` | invoke | Open a folder path in the system file manager |
| `menu:*` | push | Native menu action forwarding to renderer |

---

## 11. Key Patterns & Gotchas

- **`$state.raw` for nodes/edges** — `@xyflow/svelte` requires `$state.raw` (not `$state`) on the nodes and edges arrays to avoid deep-reactivity performance overhead. Both are always fully reassigned, never mutated in place.
- **`useSvelteFlow()` scope** — Only works inside children of `<SvelteFlow>`; use `bind:viewport` for drop coordinates in the canvas wrapper.
- **Viewport → canvas coords** — `x = (clientX - rect.left - vp.x) / vp.zoom`
- **Wire preview start point** — Query `[data-handleid]` via DOM and use `getBoundingClientRect()`; do not compute from canvas coordinates.
- **Store files** — Do not use `.svelte.ts` or `.ts` store files imported from `.svelte` components — Vite cannot resolve them. Use prop drilling or Svelte context instead.
- **Node definitions** — Loaded once in `App.svelte` via IPC `$effect`, passed as `{definitions}` prop to NodeLibrary and NodeEditor.
- **Dynamic paramDefs** — The Resize node updates its `paramDefs` array at runtime via `graphStore.updateResizeParamDefs()` when mode or preserve-aspect changes, removing stale wired edges automatically. `buildResizeParamDefs(mode, preserve)` is the pure function driving this.
- **Properties nodes in batch** — Require per-image evaluation; `hasPropNodes` flag in executor triggers slow path with `magick identify` per image.
- **Format conversion** — `FORMAT:path` prefix forces ImageMagick output format regardless of extension; `-format` is `identify`-only and must not appear in convert operations.
- **Group node ordering** — Group nodes must appear before their children in the nodes array for @xyflow/svelte to render them behind children.
- **`shell.openExternal`** — Must be called from main process via IPC (`shell:open-external`); do not import `shell` in the preload script.
- **NSIS installer patching** — `scripts/patch-nsis.cjs` runs as a postinstall hook and patches `common.nsh` (`ShowInstDetails show`) and `installSection.nsh` (`SetDetailsPrint both`) in electron-builder's templates to display file names and progress during installation.

---

## 12. Build & Dev

```bash
npm run dev          # Electron + Vite hot reload (renderer, main, preload)
npm run build        # tsc + vite build + electron-builder (production)
npm run build:web    # Renderer-only build (browser preview/testing)
npm test             # Run all Vitest unit tests once
npm run test:watch   # Vitest in watch mode (re-runs on file save)
```

Build outputs:
- `dist/` — renderer bundle
- `dist-electron/` — main + preload bundles
- `node-definitions/` — copied as-is (runtime-loaded, hot-reloaded in dev)
- `dist-cli/` — standalone CLI binary (via `@yao-pkg/pkg`)

---

## 13. Implementation Status

### Completed ✓

- 3-panel resizable layout (NodeLibrary / NodeEditor / Inspector+Preview + Filmstrip)
- Svelte Flow canvas: drag-to-canvas, wire connections, minimap, zoom controls
- Full node definition system (JSON-driven, hot-reloaded in dev)
- All node categories: Transform, Adjustments, Filters, Channels, Format, Output, Logic, Properties, Values, Math, Vector
- Inspector: all widget types (slider, number, checkbox, text, dropdown, color picker, vector)
- Param-wire system (wire any typed output port to any compatible param input)
- Bypass system (`_enabled` port with visual bypass tick; conditional processing)
- Preview pipeline with node-level caching and incremental invalidation
- Batch execution with progress reporting, fast/slow path, Gate node suppression
- Per-image metadata loading for Properties nodes in batch
- Text Filter node, Format Convert node, Rename node, Solid Image node
- Workflow save / load (`.imgplex` JSON, dirty-state tracking, close confirmation)
- Filmstrip (loaded images, click-to-select, thumbnail generation)
- Wire preview with accurate start-point (DOM `getBoundingClientRect`)
- Custom app menu with keyboard shortcuts; DevTools shortcuts preserved
- Undo / redo (full history stack in NodeEditor)
- Node search / filter in Node Library
- Node duplication (Ctrl+D, also via Edit menu)
- Node groups (Ctrl+G / Ctrl+Shift+G, resizable containers, save/load)
- Comment nodes (resizable sticky notes, editable text)
- Minimap (built-in @xyflow/svelte `MiniMap` component)
- CLI export (PowerShell / Bash / CMD; linear and branching graph support)
- NSIS installer: shows file names and progress bar during installation
- Credits modal in Help menu (open-source deps + fonts, links open in system browser)
- Typography: AtkinsonHyperlegibleNext (UI), JetBrainsMono-Regular (monospace)
- Port color WCAG AAA contrast (all port type colors ≥ 7:1 on node background)
- Input inspector: two-step folder import (pick → scan → import) with format chip filters
- Post-batch summary dialog (processed / skipped / failed counts + open output folder button)
- Text output mode in the Output node — switch the Output node to Text mode to write per-image param values to a `.txt` file with configurable separator, condition port, and live preview
- Streaming import with native header parsing (PNG/BMP/WEBP/JPEG/TGA) and batch magick spawning; ~1.6s for ~2000 images (~27× faster than sequential per-image spawning)
- Multi-stream batch pipeline with command fusion (lazy chaining) and single-spawn channel split
- File association — double-clicking a `.imgplex` file opens the workflow in the app; single-instance enforcement on Windows/Linux brings the existing window to the front for subsequent opens
- Unit tests: Vitest with 204 tests across 7 test files

### Pending — Priority Order

#### P1 — Core usability gaps

1. **Copy / Paste nodes** — No clipboard implementation. Ctrl+C / Ctrl+V for selected nodes (single or multi).

2. **Multi-select operations** — @xyflow/svelte supports box-select, but multi-delete, multi-move, and multi-duplicate interactions need validation. Copy/paste of multi-node selections is the main gap.

#### P2 — Additional image operations

7. **Watermark / Composite** — Overlay a second image or text onto the source image.
8. **Color Balance** — Separate highlight/midtone/shadow RGB adjustments.
9. **Noise Reduction** — `-despeckle` / `-enhance` / `-noise` operations.
10. **Vignette** — Radial darkening effect.
11. **Border / Padding** — Add solid-color or transparent border.
12. **Auto-orient** — Apply EXIF rotation (`-auto-orient`) for phone photos.

#### P3 — Advanced graph features

13. **Subgraph / macro nodes** — Save a node selection as a reusable compound node.
14. **Watch mode** — Monitor an input folder and auto-process new files.

#### P4 — Polish & infrastructure

15. **Settings panel** — ImageMagick path override, default output folder, theme (dark/light).
17. **Auto-update** — electron-updater integration for delivering new releases.

## 14. Alternatives Considered and Rejected

| Option | Why rejected |
|---|---|
| Tauri v2 | Linux WebKitGTK instability; IPC bottleneck for image data (~200ms/3MB); sidecar lifecycle complexity |
| Tauri + Bun sidecar | I/O speedup irrelevant when bottleneck is ImageMagick processing time; sidecar adds complexity |
| LiteGraph.js | Canvas-based, 200+ node performance; but original repo 3+ years unmaintained, ComfyUI uses a divergent fork |
| React + React Flow | Most popular node editor, but Svelte chosen for less boilerplate, no virtual DOM, native Svelte Flow integration |
| C++ / Qt 6 | Good node editors (QtNodes) but learning C++ alongside complex project would slow development |
| C# / AvaloniaUI | Viable, but Photino's webview has no Node.js — all native work must go through C# |
| Godot | GPU shaders for preview would be great; rejected because desktop UI infrastructure (inputs, dropdowns, file dialogs) would take weeks to build |
| NW.js / Wails / NeutralinoJS | Older/thinner ecosystems; Wails uses OS webview (WebKitGTK issues on Linux) |

---

## 15. Future Considerations

- **GPU-accelerated preview** — brightness, hue shift etc. could be GPU fragment shaders for instant feedback, without changing the architecture.
- **WASM-ImageMagick for web "lite" version** — ~3–5× slower than native but viable for single-image preview in a browser.
- **Plugin system** — The current JSON + executor design works well for nodes that map cleanly onto a single ImageMagick command, but hits specific ceilings:
  - **`command_template` is dumb substitution** — only regex `{{param}}` → value replacement; no conditional arguments, no parameter preprocessing, no multi-group output. Anything more complex requires a hardcoded executor branch in TypeScript.
  - **Each custom executor requires 3-file code changes** — `executor-compute.ts` for pure-value nodes, or `executor.ts` preview + batch paths for image nodes, plus possibly a custom inspector component. There is no plugin interface; the only path is forking the source.
  - **Hardcoded special-cases accumulate** — `channel_split`, `channel_merge`, `mean_value`, `resize`, `gate`, `rename`, and `format_convert` are all if-branches keyed on string executor IDs inside `executor.ts`, with no registration mechanism. Each new non-standard node adds another branch.
  - **Fixed port counts** — the JSON schema has no way to express a node whose input/output count varies at runtime (e.g. a merge accepting N images). The only workaround is the `channels` param trick, which requires a custom inspector and `paramDefs` mutation per node.
  - **No parameter interdependencies** — the schema cannot express "when `mode=relative`, hide param `width`"; each such relationship requires a bespoke inspector component (e.g. `InspectorResizeNode.svelte`) and `graphStore.updateResizeParamDefs()` wiring.
  - **String executor keys are unvalidated** — the registry validates JSON structure but not whether an `executor` value maps to any real handler. Typos silently fall through to a `console.warn` at runtime.
  - **Properties nodes use an implicit naming contract** — metadata loading is triggered by `executorKey.startsWith('prop_')`. Any new property-style node must adopt this prefix or metadata loading is silently skipped; this contract is not expressed in the schema.

  A future plugin system could address these with: (a) a sandboxed JS expression field in the node JSON for lightweight conditional argument building; (b) a `registerExecutor(key, handler)` interface so third-party code can add nodes without forking the pipeline; (c) a parameter-visibility DSL in JSON (show/hide/require rules evaluated by the inspector at runtime).
- **Multiple image inputs per node** — Compositing and blending need two input image ports; the schema already supports multiple inputs.
