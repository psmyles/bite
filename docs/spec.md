## 1. Overview

A batch image workflow builder application built around a visual node graph editor powered by imagemagick. Users compose pipelines by connecting nodes (image transforms, logic, value constants, math), preview results in real-time, and batch-process hundreds of images through the pipeline.

**Comparable product:** Retrobatch (Mac-only) - but cross-platform, more extensible, and CLI-exportable.

### Core Features

- **Visual node graph editor** - chain image operations by connecting nodes
- **Multi-input / multi-output workflows** - any number of Input nodes (each with its own filmstrip) and any mix of Image Output, Text Output, and Flipbook Output nodes, wired independently
- **Run Workflow button** - validates and executes all output nodes sequentially; shows a dialog when some are invalid; located at the bottom of the Inspector panel
- **Filmstrip view** - per-input-node thumbnail strip; tracks whichever Input node is selected
- **Real-time preview** - selected image rendered through the current node network, node-level cached
- **JSON-based node definitions** - add new nodes without recompiling
- **Data-driven output formats** - per-format encoding controls and ImageMagick args defined in `format-definitions/*.json`; add/tune a format with no recompile
- **CLI command export** - export any graph as an ImageMagick shell script (PS / Bash / CMD)
- **Workflow save/load** - serialize graphs as `.imgplex` JSON files; double-clicking a `.imgplex` file opens it directly in the app; untrusted params sanitized and version-compatibility checked on load
- **Update notification** - startup (and on-demand) check against the latest GitHub release; shows an Update Available dialog (no auto-download)
- **Batch processing** - all images through the pipeline at full resolution, progress-reported
- **Pure-value node graph** - math, logic, and value nodes with typed wires route parameters without touching the image pipeline
- **Node groups** - visually organise sets of nodes into resizable labelled containers
- **Comment nodes** - free-form canvas annotations

### UI Layout

```
┌──────────────┬───────────────────────────┬──────────────────┐
│              │                           │    Inspector     │
│  Node        │     Node Graph Editor     │  (parameters of  │
│  Library     │     (@xyflow/svelte)      │  selected node)  │
│  (sidebar)   │                           ├──────────────────┤
│              │                           │  Image Preview   │
│              │                           │  (real-time)     │
│              │                           ├──────────────────┤
│              │                           │  Run Workflow    │
├──────────────┴───────────────────────────┴──────────────────┤
│              Filmstrip (active Input node thumbnails)       │
└─────────────────────────────────────────────────────────────┘
```

---

## 2. Tech Stack

| Layer             | Technology                         | Notes                                                                                |
| ----------------- | ---------------------------------- | ------------------------------------------------------------------------------------ |
| Desktop shell     | Electron 30                        | Window management, file dialogs, auto-update, bundling                               |
| Build             | Vite 5 + vite-plugin-electron      | Hot-reload dev, separate renderer/main/preload bundles                               |
| UI framework      | Svelte 5 (runes) + TypeScript      | Entire UI - panels, filmstrip, inspector, preview                                    |
| Node graph editor | @xyflow/svelte 1.x (Svelte Flow)   | Custom node components, minimap, controls                                            |
| Image processing  | ImageMagick v7 CLI (`magick`)      | All image operations; no Sharp / no LGPL deps                                        |
| Packaging         | electron-builder (NSIS on Windows) | NSIS installer patched via `scripts/patch-nsis.cjs` postinstall to show file details |
| Language          | TypeScript throughout              | Main process, renderer, shared types                                                 |

### Why These Choices

**Electron over Tauri:** Three blockers emerged during research: (1) Linux WebKitGTK instability - a Tauri maintainer stated they "can't 100% recommend Tauri for Linux"; (2) IPC bottleneck - serialising image data as strings benchmarked at 200ms per 3MB; (3) sidecar lifecycle complexity. Electron eliminates all three: consistent Chromium everywhere, Node.js built in, and battle-tested packaging.

**@xyflow/svelte over LiteGraph.js:** LiteGraph (used by ComfyUI) is canvas-based and handles 200+ nodes, but its original repo is 3+ years unmaintained and ComfyUI uses a divergent fork. @xyflow/svelte is MIT-licensed, actively maintained, has native Svelte integration, and DOM performance is never the bottleneck for image processing graphs (typically 5–30 nodes).

**Svelte 5 over React:** Less boilerplate, no virtual DOM, `$state.raw` rune for performance-conscious reactivity (required by @xyflow/svelte), and native integration with Svelte Flow.

---

## 3. Licensing

| Component                   | License                                                         |
| --------------------------- | --------------------------------------------------------------- |
| Electron                    | MIT                                                             |
| Svelte 5                    | MIT                                                             |
| @xyflow/svelte              | MIT                                                             |
| Vite / vite-plugin-electron | MIT                                                             |
| electron-builder            | MIT                                                             |
| TypeScript                  | Apache 2.0                                                      |
| @yao-pkg/pkg                | MIT                                                             |
| ImageMagick                 | ImageMagick License (Apache-2.0 derived - attribution required) |
| JetBrains Mono              | SIL OFL 1.1                                                     |
| Atkinson Hyperlegible       | SIL OFL 1.1                                                     |

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
│   ├── main.ts            - App bootstrap, BrowserWindow, native menus, IPC registration, GitHub update check, file-association open
│   ├── preload.ts         - contextBridge exposes window.ipcRenderer (invoke/on/off/send)
│   ├── update-utils.ts    - Pure helpers (no electron imports): extractImgplexPath (find .imgplex in argv), compareSemver
│   ├── ipcListenerTracker.ts - Dev-only guard that tracks ipcMain listener registration to catch duplicate handlers on hot-reload
│   └── electron-env.d.ts  - Ambient types for the preload-exposed window.ipcRenderer
├── src/
│   ├── shared/
│   │   ├── types.ts       - NodeDefinition, GraphNode, GraphEdge, NodeGraph, ImageInfo, FormatDefinition, Progress
│   │   ├── constants.ts   - IPC channel name map (IPC.*), EXECUTOR key map, LIGHT_META_EXECUTORS, IMAGE_EXTENSIONS, PREVIEW_MAX_EDGE_PX, THUMBNAIL_SIZE_PX, PREVIEW_DEBOUNCE_MS, APP_NAME
│   │   ├── graphTrace.ts  - traceInputNodeId(nodes, edges, outputNodeId): canonical backward-BFS (skips param- wires); shared by renderer, main, and CLI
│   │   ├── compatUtils.ts - isCompatible(fileVersion, appVersion): checks a .imgplex file's version against the app using compat-ranges.json
│   │   ├── compat-ranges.json - Ordered list of mutually-compatible app version ranges; files load only within the same range as the running app
│   │   └── renameUtils.ts - Filename token expansion for Rename node
│   ├── main/              - Node.js / Electron main-process business logic
│   │   ├── logger.ts                - Process-wide logger; mirrors entries to the renderer log viewer via IPC.LOG_ENTRY and a ring buffer
│   │   ├── nodes/
│   │   │   └── registry.ts          - Loads and watches node-definitions/*.json; hot-reloads in dev
│   │   ├── pipeline/
│   │   │   ├── executor.ts          - Thin shell (~60 lines). PipelineExecutor class with one-liner delegates for every public method.
│   │   │   ├── graph-utils.ts       - Pure graph functions: topoSort, groupBySetPattern, findOutputContributors, applyParamWires
│   │   │   ├── image-header.ts      - Native header parsing for PNG, JPEG, BMP, WebP, TGA (no magick spawn for dimensions); fileToDataUrl
│   │   │   ├── magick-spawn.ts      - spawnMagick (Promise<void>) and spawnMagickCapture (Promise<string>) helpers; centralises binary resolution
│   │   │   ├── thumbnail-service.ts - Thumbnail generation and image loading; module-level _metaCache per path
│   │   │   ├── preview-pipeline.ts  - executePreview as a standalone function; per-node output caching via PreviewCache
│   │   │   ├── batch-pipeline.ts    - executeBatch orchestrator (~450 lines); builds BatchContext, delegates to multistream-pipeline / set-pipeline; owns fast-path buildOpArgsForImage and regular processOne loop
│   │   │   ├── multistream-pipeline.ts - BatchContext interface + executeMultiStream; lazy-chain command fusion, channel split batching, per-image mean value pre-computation
│   │   │   ├── set-pipeline.ts      - executeSetBatch; groups images by naming convention, runs one executeMultiStream per set
│   │   │   ├── resolve-params.ts    - resolveNodeParams(node, def, edges, resolvedParams, meta): shared param-resolution step used by batch, preview, and text-output pipelines
│   │   │   ├── executor-compute.ts  - Pure numeric/vector helpers and computeNodeParams (reads image metadata via magick identify)
│   │   │   ├── executor-cli.ts      - CLI script generators (PowerShell / Bash / CMD); graph-aware — reads cliName params to emit one named flag per workflow node
│   │   │   ├── executorRegistry.ts  - ArgBuilderFn registry for nodes needing custom argument building beyond command_template/command_js
│   │   │   ├── imageNodeExecutors.ts - Registers executors for image-processing nodes with logic beyond a template
│   │   │   ├── command-builder.ts   - Converts command_template / command_js + resolved params into string[] of ImageMagick args; also buildFormatConvertArgs(format, params) (evaluates a format-definition's args_js) and getFormatExtension(format)
│   │   │   ├── cache.ts             - Node-level output cache (PreviewCache) keyed by nodeId:inputHash
│   │   │   ├── timing.ts            - Optional perf instrumentation (timings.enabled); writes perf.log with batch timing summary and per-image ImageMagick verbose output
│   │   │   ├── output-log.ts        - Writes a timestamped output log file to the output directory after a batch completes
│   │   │   ├── scan-folder.ts       - scanFolder(root, recursive, extSet): async recursive listing of files matching a set of extensions
│   │   │   └── magick-path.ts       - Resolves bundled resources/win/magick/magick.exe in production, system magick in dev
│   │   └── ipc/
│   │       ├── handlers.ts          - Core IPC handler registration (registry, pipeline, dialogs, shell, app lifecycle, workflows)
│   │       ├── workflow-sanitize.ts - sanitizeWorkflowGraph: strips `__`-prefixed param keys (e.g. __compute_js__) from untrusted .imgplex data on load
│   │       ├── text-output-handlers.ts - IPC handlers for text output operations (browse, preview, write, cancel)
│   │       └── atlas-handlers.ts    - IPC handlers for the flipbook atlas generation feature
│   └── renderer/
│       ├── main.ts        - Svelte app mount entry point
│       ├── App.svelte     - 3-panel resizable layout; workflow save/load; menu IPC listeners; two $effects for active input node tracking (selecting an inputNode calls imageStore.setActive; falls back to first inputNode if active is deleted)
│       ├── assets/
│       │   ├── theme.css  - CSS custom properties (colors, typography, layout sizes, port colors)
│       │   └── fonts.css  - @font-face for JetBrainsMono-Regular and AtkinsonHyperlegibleNext-Regular
│       ├── components/
│       │   ├── RunWorkflowButton.svelte     - Run Workflow button rendered at the bottom of the Inspector panel; validates all output nodes (imageOutputNode, textOutputNode, flipbookOutputNode); shows RunWorkflowDialog when some are invalid; handles native menu shortcut (IPC.MENU_RUN_WORKFLOW); exports OutputNodeStatus type
│       │   ├── RunWorkflowDialog.svelte     - Modal listing each output node's readiness (✅/⚠️ + reason); "Run N nodes" / Cancel; imports OutputNodeStatus from RunWorkflowButton.svelte
│       │   ├── NodeLibrary.svelte           - Pinned Workflow section (Input, Image Output, Text Output, Flipbook Output) + categorised JSON-defined nodes; drag-to-canvas; collapsible, search-filtered
│       │   ├── Inspector.svelte             - Routes to the correct inspector by node.type
│       │   ├── InspectorInputNode.svelte    - Takes nodeId prop; scopes all image ops (add/clear/select) to that node
│       │   ├── InspectorImageOutputNode.svelte - Output path, overwrite mode, log generation; no per-node run button (execution goes through RunWorkflowButton)
│       │   ├── InspectorTextOutputNode.svelte  - Text output settings; uses traceInputNodeId for connected image list; dynamic value port management
│       │   ├── InspectorFlipbookOutputNode.svelte - Flipbook atlas settings (grid, padding, format, skip/overwrite mode)
│       │   ├── InspectorFormatConvertNode.svelte  - Format dropdown + format-specific encoding params sourced from format-definitions/<format>.json; params shown/hidden via params_visibility rules
│       │   ├── InspectorSetInputNode.svelte - Process-As-Set inspector (prefix + suffix list; adds/removes suffix output ports)
│       │   ├── InspectorParamEditor.svelte  - Dynamic param widgets for ProcessNode
│       │   ├── InspectorCommentNode.svelte      - Comment node text editing
│       │   ├── InspectorRenameNode.svelte       - Rename node block editor
│       │   ├── InspectorResizeNode.svelte       - Resize node inspector (mode/preserve switcher, dynamic port visibility)
│       │   ├── InspectorFolderPathNode.svelte   - Folder path node inspector
│       │   ├── Preview.svelte           - Live preview image; triggers pipeline re-run on node/image change
│       │   ├── Filmstrip.svelte         - Horizontal thumbnail strip; shows active input node's images; click-to-select
│       │   ├── Dropdown.svelte          - Reusable styled dropdown
│       │   ├── ColorPicker.svelte       - HSL/hex color picker
│       │   ├── colorConversions.ts      - Pure RGB/HSL/hex conversion helpers used by ColorPicker
│       │   ├── MenuBar.svelte           - Custom menubar (File / Help) rendered in renderer
│       │   ├── BatchProgressModal.svelte - Live batch progress (per-image counter, cancel button) wired to IPC.EXECUTE_BATCH_PROGRESS / EXECUTE_BATCH_CANCEL
│       │   ├── BatchSummaryModal.svelte - Post-batch summary dialog (processed/skipped/failed + open output folder)
│       │   ├── ImportProgressModal.svelte - Streaming-import progress (images loaded so far) with cancel
│       │   ├── ConfirmModal.svelte      - Generic confirm/cancel dialog (Cancel autofocused as the safe default)
│       │   ├── UpdateModal.svelte       - "Update available" dialog shown on IPC.UPDATE_AVAILABLE; release notes + link to GitHub release
│       │   ├── IncompatibleVersionModal.svelte - Shown when a loaded .imgplex was saved by an incompatible app version (compatUtils)
│       │   ├── runWorkflowTypes.ts      - Shared OutputNodeStatus type imported by RunWorkflowButton / RunWorkflowDialog / their component tests
│       │   ├── AboutModal.svelte        - About dialog (version, links to GitHub)
│       │   └── CreditsModal.svelte      - Credits modal (open source deps + fonts) with system-browser links
│       ├── nodeEditor/
│       │   ├── NodeEditor.svelte        - Svelte Flow canvas; drag-drop, context menu, undo/redo, grouping; deletion guard; workflow node drag/drop and context menu
│       │   ├── graphTransforms.ts       - Pure graph transforms extracted from NodeEditor (computeDeleteSelected, duplicate, CLI-name assignment) so delete/duplicate logic is unit-testable without a DOM
│       │   ├── ProcessNode.svelte       - Standard processing node (image ports, param rows, bypass tick)
│       │   ├── InputNode.svelte         - Image source node; shows per-node image count via imageStore.getImages(id)
│       │   ├── SetInputNode.svelte      - Renderer node for the process_as_set definition; one image input + dynamic per-suffix string/image outputs
│       │   ├── CompareNode.svelte       - Comparison node rendering operator symbols (=, ≠, >, <, ≥, ≤) inline
│       │   ├── ImageOutputNode.svelte   - Image file output node; amber header tint; output path footer
│       │   ├── TextOutputNode.svelte    - Text/metadata output node; in-0 image input + dynamic txo-* value ports; output file footer
│       │   ├── FlipbookOutputNode.svelte - Flipbook atlas output node; grid dimensions footer
│       │   ├── FolderPathNode.svelte    - Folder path source node (path port output)
│       │   ├── GroupNode.svelte         - Resizable labelled container; children move with group
│       │   ├── CommentNode.svelte       - Resizable sticky note with editable text
│       │   ├── ColoredEdge.svelte       - Type-coloured wire rendering
│       │   ├── NodeContextMenu.svelte   - Right-click creation/action menu (wire-drop + canvas); includes Workflow node section
│       │   ├── DropHelper.svelte        - Rendered inside <SvelteFlow> for store context; exposes screenToFlowPosition, setViewport, updateNodeInternals (via useUpdateNodeInternals()) to parent
│       │   ├── connectionValidation.ts  - isValidConnection, handleToWireType, resolveEffectiveWireType and all type-compatibility helpers (pure functions, no Svelte reactivity)
│       │   ├── undoRedoManager.ts       - UndoRedoManager class: push / schedulePush (deferred batching) / undo / redo
│       │   ├── portColors.ts            - Wire color map keyed by data type
│       │   ├── wireTypeUtils.ts         - Wire-type compatibility, paramTypeToWireType, paramInHandle/paramOutHandle
│       │   ├── nodeEditorHelpers.ts     - buildNodeData, expandNodeData, buildResizeParamDefs, sortNodesGroupFirst, firstMatchingHandle; maps process_as_set → setInputNode component type
│       │   └── nodeEnabledState.ts      - Bypass state helpers
│       ├── stores/
│       │   ├── graph.svelte.ts          - GraphStore: nodes (seeded via makeSeedNodes()), edges, selection, batch state, viewport; canDeleteNode() guards last inputNode / last output node of any type
│       │   └── images.svelte.ts         - Per-node image storage: Map<nodeId, ImageInfo[]>; activeInputNodeId tracks the filmstrip; getImages(nodeId), setActive(nodeId), add(paths, nodeId), clear(nodeId), removeNode(nodeId)
│       ├── workflowUtils.ts             - hasSetInputInChain(...) + re-exports traceInputNodeId from shared/graphTrace.ts; renderer calls window.ipcRenderer.invoke directly (no service layer)
│       ├── platform.ts                  - IS_ELECTRON flag (detects Electron vs browser context)
│       └── browserIpc.ts               - Browser shim for window.ipcRenderer (workflow builder without Electron)
├── src/tests/             - Vitest unit tests; node project for pure logic + jsdom project for *.svelte.test.ts component tests
│                            (graph-utils, graphTrace, graphTransforms, batch-pipeline, preview-pipeline, command-builder,
│                            format-definitions, executor-cli, workflow-sanitize, update-utils, compatUtils, store tests,
│                            registry, ipc-channels, RunWorkflowDialog component test, etc.)
├── node-definitions/      - JSON node descriptor files (loaded at runtime, hot-reloaded in dev)
├── format-definitions/    - Per-format encoding definitions (jpeg/png/webp/avif/tiff/bmp/tga .json); FormatDefinition shape, loaded via import.meta.glob in command-builder.ts and InspectorFormatConvertNode.svelte
├── workflows/             - User workflow save files (.imgplex JSON)
├── scripts/
│   └── patch-nsis.cjs     - Postinstall script: patches electron-builder NSIS templates to show install file details
└── resources/             - Bundled assets (portable ImageMagick, app icon)
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

| Field                  | Description                                                                                      |
| ---------------------- | ------------------------------------------------------------------------------------------------ |
| `name`                 | Internal identifier; becomes the param key and handle ID suffix                                  |
| `label`                | Human-readable display name in the Inspector                                                     |
| `type`                 | Data type - controls wire color and type-checking                                                |
| `widget`               | Inspector control type                                                                           |
| `default`              | Initial value                                                                                    |
| `min` / `max` / `step` | Numeric constraints for slider and number widgets                                                |
| `options`              | Dropdown option list (required for `enum` type)                                                  |
| `readonly`             | If true: right-side output handle, no input handle; display-only in Inspector for executor nodes |
| `portOnly`             | If true: only a right-side port, no body row shown in node card                                  |
| `noPort`               | If true: body row only, no input handle (Inspector-only param)                                   |

### Special Param Conventions

- The hidden `_enabled` bool param on every processing node is the **bypass port** - `false` passes image through unchanged
- `portOnly` params (e.g. vector component outputs) appear only as right-side handles with labels
- Param values can be wired from other nodes via the param-wire system

---

## 6. Node Type System

### Two Node Families

**Image nodes** - have at least one `image`/`mask` port in `inputs` or `outputs`. Executed by the pipeline engine as ImageMagick operations. Require either `command_template` or `executor`.

**Pure-value nodes** - empty `inputs` and `outputs` arrays. Never touch ImageMagick. Evaluated in topological order; resolved output values forwarded along param-wires to downstream nodes.

### Param-Wire System

- **Writable params** (no `readonly`) expose an input handle on the left.
- **Readonly params** expose an output handle on the right.
- Type compatibility enforced at connection time.
- **Single-input rule:** each input port accepts only one wire; a second connection displaces the first.
- **No cycles:** BFS rejects edges that would create feedback loops.

### Port / Wire Colors (WCAG AAA on node background `#212121`)

| Type                       | Color      | Hex       |
| -------------------------- | ---------- | --------- |
| `image`                    | Orange     | `#ff8c3f` |
| `mask`                     | Purple     | `#d8a4fc` |
| `number` / `int` / `float` | Cyan       | `#22d3ee` |
| `string`                   | Green      | `#22c55e` |
| `boolean`                  | Yellow     | `#eab308` |
| `color`                    | Pink       | `#fc86bc` |
| `vector2`                  | Amber      | `#fb923c` |
| `vector3`                  | Indigo     | `#a5b4fc` |
| `vector4`                  | Teal       | `#2dd4bf` |
| `numeric`                  | Near-white | `#e2e8f0` |
| `any`                      | Slate      | `#aaaaaa` |

### Built-in Workflow Node Types

These four types are registered directly in `NodeEditor.svelte` (not from JSON). They appear in the pinned **Workflow** section of the Node Library and in the canvas context menu.

| Type                 | Role                                     | Deletable?                                   |
| -------------------- | ---------------------------------------- | -------------------------------------------- |
| `inputNode`          | Image source; each has its own filmstrip | Not if it's the last one                     |
| `imageOutputNode`    | Writes processed images to disk          | Not if it's the last output node of any type |
| `textOutputNode`     | Writes text/metadata values to a file    | Not if it's the last output node of any type |
| `flipbookOutputNode` | Assembles images into a flipbook atlas   | Not if it's the last output node of any type |

All output nodes require an explicit image wire on `in-0`. Execution is triggered from the **Run Workflow** button at the bottom of the Inspector panel (`RunWorkflowButton.svelte`). `traceInputNodeId` is used to determine which input node's image list feeds each output node.

Each workflow node also stores a **`cliName`** param — auto-assigned at creation (e.g. `input-1`, `output-image-1`) and editable in the inspector. This name becomes the CLI flag (`--<cliName>`) in exported shell scripts and is parsed by `imgplex-cli run`. See §7.6 CLI Export for details.

### JSON-Defined Node Categories

| Category        | Nodes                                                                                          |
| --------------- | ---------------------------------------------------------------------------------------------- |
| **Source**      | Process As Set                                                                                 |
| **Transform**   | Resize, Resize (Nearest Neighbor), Crop, Rotate, Flip, Auto Orient, Extend Canvas, Pixelate, Trim Alpha |
| **Adjustments** | Brightness/Contrast, Levels, Grayscale, Hue Offset, Saturation, Negate                         |
| **Color**       | Quantize, Threshold, Tint                                                                       |
| **Filters**     | Blur, Sharpen                                                                                   |
| **FX**          | Outline                                                                                         |
| **Channels**    | Channel Split, Channel Merge, Extract Alpha, Premultiply Alpha, Normal Map Flip                |
| **Format**      | Format Convert, Strip Metadata                                                                  |
| **Output**      | Rename                                                                                          |
| **Logic**       | Gate, Text Filter, Comparison, AND, OR, NOT, Branch                                            |
| **Properties**  | Name, Path, Dimensions, Size, Bit Depth, File Type, Resolution, EXIF, Power of Two              |
| **Values**      | Boolean, Float, String, Color, Vector2, Vector3, Vector4, Solid Image                          |
| **Math**        | Add, Subtract, Multiply, Divide, Power, Lerp, Mean Value                                        |
| **Vector**      | Append Vec, Split Vec, Dot Product, Length, Normalize                                          |
| **Graph**       | Comment (annotation node), Group (visual container)                                            |

---

## 7. Pipeline Engine

### 7.1 Execution Model

1. **Topological sort** (Kahn's algorithm) to determine execution order. A cycle (only reachable via a hand-edited/corrupt `.imgplex`) leaves nodes out of the sort; `topoSort` logs a warning rather than silently dropping them.
2. **Resolve parameters per node** - start from Inspector values, override with upstream wired values.
3. **Pure nodes** (no image ports): `computeNodeParams()` evaluates output values; skip ImageMagick.
4. **Image nodes**: build ImageMagick args from resolved params, execute `magick`, cache output.

### 7.2 Preview Pipeline

- Triggered on: node selection change, parameter edit, active image change, or after a graph edit.
- Graph trimmed to ancestors of the selected node (backward BFS via all edge types).
- Input downscaled proportionally to `PREVIEW_MAX_EDGE_PX = 1024px` before entering the pipeline.
- **Node-level caching:** output keyed by `(nodeId, hash(inputPath + params))`; only re-runs invalidated nodes.
- **Incremental invalidation:** when a node changes, it and only its **actual descendants** are cache-invalidated, via `findDescendants` (forward BFS over edges). Earlier code invalidated everything that merely sorted after the changed node in topological order, needlessly busting the cache for unrelated parallel branches.
- **Debounce:** 80ms after last param change (0ms when no nodes present).
- **Temp file race guard:** concurrent preview requests for the same input don't conflict - write errors are caught and re-checked for existence.
- Single image: only the currently selected filmstrip image.

### 7.3 Import Pipeline

Importing images uses a multi-stage streaming pipeline optimised for Windows (where `spawn()` costs ~200ms):

1. **Streaming push model** - results are sent to the renderer one at a time as they complete via `pipeline:load-images-streaming-result` IPC pushes; the renderer adds each image to the filmstrip immediately rather than waiting for the whole batch.
2. **Concurrent workers** - `os.cpus().length` workers run in parallel, each claiming an atomic chunk of `BATCH_SIZE = 8` images at a time.
3. **Native header parsing** - PNG, BMP, WEBP, JPEG, and TGA dimensions are read directly from file headers in Node.js (no magick spawn). These "fast-path" images are classified and batched together.
4. **Batch magick spawn** - all fast-path images in a chunk are processed in one `magick` invocation using `-write <thumbN> +delete` sequencing. This amortises ~200ms process-spawn cost over 8 images instead of paying it per image.
5. **Slow-path fallback** - formats requiring magick for dimensions (PSD, TIFF, RAW, GIF, …) fall back to individual `-print %w %h %m` + thumbnail spawns; these are rare.
6. **In-memory metadata cache** - `_metaCache` on `PipelineExecutor` stores `{width, height, format}` per path for the session; re-importing the same folder hits the cache and skips all magick calls.
7. **Thumbnail disk cache** - thumbnails are written to `%TEMP%/imgplex-preview/thumb_<hash>_<size>.png` and reused if the source file has not changed (mtime comparison).

Result: ~1.6s for 1983 mixed PNG/TGA/JPG/PSD images (vs ~44s before batching).

### 7.4 Batch Pipeline

- Separate from preview - no caching, full resolution.
- **Parallel execution:** runs N concurrent pipelines via `Promise.all` where N = `concurrency = Math.min(128, imagePaths.length)`. `MAGICK_THREAD_LIMIT` (= cores / N) is passed **per spawn** via the `ctx.spawnEnv` field — not by mutating `process.env` — so concurrent previews/thumbnails and overlapping batches don't interfere. This prevents ImageMagick's internal OpenMP pool from oversubscribing the CPU.
- **Fast path:** if no Properties nodes present, `buildOpArgsForImage` runs once to produce a shared plan applied to all images.
- **Slow path:** if Properties nodes present, plan re-evaluated per image with fresh `magick identify` metadata.
- **Multi-stream path:** activated when channel_split, channel_merge, or mean_value nodes are present. Uses a lazy-chain buffer model - consecutive standard ops accumulate into a `{base, args}` lazy chain; materialisation is deferred until a fork (multiple consumers) or a format change. This fuses multiple standard nodes into a single magick invocation.
- **Channel split optimisation:** all 4 channels extracted in one magick call via parenthesised `-write` branches. If all consumers are `mean_value` nodes, the channel PNGs are never written; per-channel mean is computed directly from the source.
- Gate node returns `null` plan → image silently skipped (no output file).
- Format Convert changes output extension and uses `FORMAT:path` prefix.
- Per-image errors are non-fatal; batch continues; error summary reported at end.
- Progress pushed to renderer via IPC event `pipeline:execute-batch:progress`.

### 7.5 Pure Node Executors (`computeNodeParams`)

| Executor key                                                                                                                      | Operation                                                           |
| --------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------- |
| `math_add` / `math_subtract` / `math_multiply` / `math_divide`                                                                    | `result = a OP b`                                                   |
| `math_power`                                                                                                                      | `result = base ^ exponent`                                          |
| `math_lerp`                                                                                                                       | `result = a + (b − a) × t`                                          |
| `mean_value`                                                                                                                      | `result = mean(inputs)`                                             |
| `logic_and` / `logic_or` / `logic_not`                                                                                            | Boolean operators                                                   |
| `logic_branch`                                                                                                                    | `result = condition ? value_true : value_false`                     |
| `logic_comparison`                                                                                                                | `result = a {==, !=, >, <, >=, <=} b`                               |
| `text_filter`                                                                                                                     | `result = string matches(prefix/suffix/contains) pattern`           |
| `split_vec`                                                                                                                       | `{x, y[, z[, w]]} = vec[0..N]`                                      |
| `append_vec`                                                                                                                      | `vec = [x, y[, z[, w]]]`                                            |
| `vec_math_dot`                                                                                                                    | `result = dot(a, b)`                                                |
| `vec_math_length`                                                                                                                 | `result = length(v)`                                                |
| `vec_math_normalize`                                                                                                              | `result = v / length(v)`                                            |
| `rename`                                                                                                                          | Builds filename from ordered blocks (text / number / original-name) |
| `prop_name` / `prop_path` / `prop_dimensions` / `prop_size` / `prop_bitdepth` / `prop_filetype` / `prop_resolution` / `prop_exif` / `prop_power_of_two` | Per-image metadata. The "light" subset (`prop_name`, `prop_path`, `prop_size`, `prop_filetype`, `prop_dimensions`, `prop_power_of_two` — see `LIGHT_META_EXECUTORS`) is satisfied with a cheap `fs.stat` + header read and **no** `magick identify` spawn |
| _(no executor)_                                                                                                                   | Value nodes - params are the output values directly                 |

### 7.6 CLI Export

- `IPC.EXPORT_CLI` saves two files side-by-side: a shell script and a companion `.imgplex` workflow file.
- `imgplex-cli.exe` is installed alongside the app and added to the user's PATH by the NSIS installer - no separate install step required.
- Supports three shell targets: **PowerShell** (`.ps1`), **Bash** (`.sh`), **CMD** (`.bat`).
- Menu items: File → Export CLI Script (PS / Bash / CMD).

**Named-flag system:**

Every Input node and output node has a user-editable **CLI Name** param (set in the inspector). The CLI Name becomes the flag name used in the exported script and passed to `imgplex-cli run`:

```
imgplex-cli run workflow.imgplex --input-1 ./photos --input-2 ./overlays --output-image-1 ./out
```

- **Auto-assignment at creation:** nodes get default names by type and creation order: `input-1`, `input-2`, `output-image-1`, `output-text-1`, `output-flipbook-1`, etc.
- **User-editable:** the inspector shows the name field with the resulting flag below it (e.g. `Flag: --input-1`). Input is sanitized to `[a-z0-9-]`. A conflict warning appears if two nodes share the same name.
- **Script generation is graph-aware:** `executor-cli.ts` reads the graph passed by the `IPC.EXPORT_CLI` handler and emits one param per named node. Input node flags are documented as required; output node flags are optional (defaults to baked-in paths or `./output`).
- **Execution:** `imgplex-cli run` parses all `--<flag> <path>` pairs from argv, builds a map from cliName → path, then iterates every output node in the graph:
  1. Traces back (BFS, skipping param-wires) to find the connected inputNode.
  2. Reads the input directory from the corresponding named flag; exits with a clear error if missing.
  3. Computes the output path: uses the CLI flag value if provided, otherwise falls back to the node's baked-in `customPath` / `outputPath` / `flipbookOutputPath` param (for textOutputNode and flipbookOutputNode, overrides are applied by patching a graph copy before calling `executeBatch`).
  4. Calls `executor.executeBatch(graph, outNode.id, inputNodeId, images, outputDir, overwrite, registry, onProgress)`.
- **Nodes without a cliName** are still executed (using their baked-in paths) but do not appear as flags in the exported script.

### 7.7 Format Definitions (data-driven encoding)

Output encoding settings live in `format-definitions/` — one JSON file per format (`jpeg`, `png`, `webp`, `avif`, `tiff`, `bmp`, `tga`). Each file is a `FormatDefinition`:

- `id` — uppercase format name (e.g. `"JPEG"`), the lookup key
- `extension` — canonical output extension (e.g. `".jpg"`)
- `params` — `ParamDefinition[]` rendered by `InspectorFormatConvertNode` (same widgets as node params: slider/dropdown/checkbox)
- `params_visibility` (optional) — conditional show/hide rules (e.g. hide WebP Quality when Lossless is on)
- `args_js` — JS function body receiving `params`, returns `string[]` of ImageMagick encoding args

The files are loaded eagerly via `import.meta.glob` in both `command-builder.ts` (main, via `buildFormatConvertArgs`/`getFormatExtension`) and `InspectorFormatConvertNode.svelte` (renderer). The `format_convert` node's own definition carries only the format dropdown; every encoding control comes from the matching format definition at runtime. Adding or changing a format is therefore a JSON edit with no recompile. `args_js` runs in the main process and is treated as app-controlled code (same trust boundary as `command_js`/`compute_js`).

---

## 8. Node Editor Behaviour

| Action                      | Shortcut                         |
| --------------------------- | -------------------------------- |
| Duplicate selected nodes    | Ctrl+D                           |
| Delete selected nodes/edges | Delete / Backspace               |
| Undo                        | Ctrl+Z                           |
| Redo                        | Ctrl+Y / Ctrl+Shift+Z            |
| Group selected nodes        | Ctrl+G                           |
| Ungroup selected group      | Ctrl+Shift+G                     |
| Open context menu           | Right-click canvas / Space / Tab |
| Zoom in / out               | Ctrl+Scroll or +/-               |
| Fit view                    | Ctrl+Shift+F                     |

Additional behaviours:

- **Single-input rule** - connecting to an occupied port displaces the existing wire
- **Cycle detection** - BFS prevents feedback loops
- **Type enforcement** - connection rejected if handle types are incompatible
- **Colored wires** - each data type has a distinct wire color; drag-preview wire matches source type
- **Wire-drop node creation** - dragging a wire onto empty canvas opens a filtered node menu; new node auto-connects to the correct typed handle
- **Context menu** - right-clicking empty canvas opens full node creation menu; right-clicking a group node offers ungroup
- **Node groups** - Ctrl+G wraps selected nodes in a resizable labelled container; children move with group and are constrained inside via `parentId` + `extent: 'parent'`; Ctrl+Shift+G restores children to absolute positions
- **Comment nodes** - resizable sticky note; text editable on double-click; no ports
- **Node tooltips** - hover header for 1s shows `description` field from node definition

---

## 9. Workflow Files

```json
{
  "version": "1.0",
  "graph": {
    "nodes": [
      /* GraphNode[] - includes group nodes with width/height/parentId */
    ],
    "edges": [
      /* GraphEdge[] */
    ],
    "viewport": { "x": 0, "y": 0, "zoom": 1 }
  }
}
```

- File extension: `.imgplex`
- Dirty-state tracking: unsaved changes prompt before close/new/open
- Group nodes saved with `width`, `height`; children saved with `parentId` and `extent: 'parent'`

---

## 10. IPC Channels

| Channel                                 | Direction | Purpose                                                                                                      |
| --------------------------------------- | --------- | ------------------------------------------------------------------------------------------------------------ |
| `registry:get-all`                      | invoke    | Load all node definitions                                                                                    |
| `registry:updated`                      | push      | Hot-reload notification                                                                                      |
| `pipeline:load-images-with-thumbnails`  | invoke    | Load images + generate thumbnails in one call                                                                |
| `pipeline:load-images-streaming-start`  | invoke    | Begin streaming import; results pushed per image                                                             |
| `pipeline:load-images-streaming-result` | push      | One loaded image result during streaming import                                                              |
| `pipeline:load-images-streaming-cancel` | invoke    | Cancel an in-progress streaming import                                                                       |
| `pipeline:generate-thumbnail`           | invoke    | Generate a single thumbnail (used for preview warm-up)                                                       |
| `pipeline:execute-preview`              | invoke    | Run preview pipeline                                                                                         |
| `pipeline:execute-batch`                | invoke    | Run batch pipeline; payload: graph, outputNodeId, inputNodeId, imagePaths, outputDir, overwrite, generateLog |
| `pipeline:execute-batch:progress`       | push      | Progress updates during batch                                                                                |
| `pipeline:execute-batch-cancel`         | invoke    | Cancel an in-progress batch run                                                                              |
| `pipeline:export-cli`                   | invoke    | Export CLI script (PS/Bash/CMD)                                                                              |
| `dialog:open-images`                    | invoke    | File open dialog for images                                                                                  |
| `dialog:open-folder`                    | invoke    | Folder picker (no scan)                                                                                      |
| `dialog:scan-folder`                    | invoke    | Pick folder via dialog then scan for matching extensions                                                     |
| `dialog:scan-folder-only`               | invoke    | Scan a given folder path for matching extensions (no dialog)                                                 |
| `atlas:generate`                        | invoke    | Generate a flipbook atlas from the connected images                                                          |
| `atlas:browse`                          | invoke    | Save-file dialog for the flipbook atlas output path                                                          |
| `text-output:browse`                    | invoke    | Save-file dialog for text output path                                                                        |
| `text-output:preview`                   | invoke    | Compute text output lines without writing (preview)                                                          |
| `text-output:write`                     | invoke    | Write text output file; sends per-image progress                                                             |
| `text-output:write-progress`            | push      | Progress during text output write                                                                            |
| `text-output:write-cancel`              | invoke    | Cancel an in-progress text output write                                                                      |
| `workflow:save`                         | invoke    | Save workflow JSON                                                                                           |
| `workflow:load`                         | invoke    | Load workflow JSON (shows open dialog)                                                                       |
| `workflow:open-path`                    | invoke    | Load workflow JSON from a given file path (no dialog)                                                        |
| `app:quit`                              | invoke    | Graceful quit with dirty-state check                                                                         |
| `app:open-file-path`                    | push      | Main → renderer: carry file path from OS file-association open                                               |
| `app:update-available`                  | push      | Main → renderer: a newer GitHub release exists (version, notes, url) → UpdateModal                           |
| `app:check-for-updates`                 | invoke    | Renderer-triggered update check                                                                             |
| `app:get-versions`                      | invoke    | App / Electron / Chromium / ImageMagick version strings for the About dialog                                |
| `shell:open-external`                   | invoke    | Open URL in default system browser                                                                           |
| `shell:open-path`                       | invoke    | Open a folder path in the system file manager                                                                |
| `timers:set-enabled`                    | invoke    | Toggle optional perf instrumentation (timings.enabled)                                                       |
| `log:entry`                             | push      | Main → renderer: one log line for the in-app log viewer                                                      |
| `log:get-entries`                       | invoke    | Fetch buffered log entries                                                                                  |
| `menu:*`                                | push      | Native menu action forwarding to renderer (new, open, save, duplicate, delete, run-workflow, check-for-updates, export-cli-\*, about, credits) |

---

## 11. Key Patterns & Gotchas

- **`$state.raw` for nodes/edges** - `@xyflow/svelte` requires `$state.raw` (not `$state`) on the nodes and edges arrays to avoid deep-reactivity performance overhead. Both are always fully reassigned, never mutated in place.
- **`useSvelteFlow()` scope** - Only works inside children of `<SvelteFlow>`; use `bind:viewport` for drop coordinates in the canvas wrapper. `useUpdateNodeInternals()` is a separate hook - it is **not** part of `useSvelteFlow()`.
- **`updateNodeInternals` effects must track `graphStore.nodes`, not the local SvelteFlow-bound `nodes`.** Tracking local `nodes` causes an infinite loop: `updateNodeInternals` → SvelteFlow updates node dimensions → `nodes` changes → effect re-fires. Use a `$derived` signature string from `graphStore.nodes` and read node IDs with `untrack()`.
- **`$derived` cannot reference itself.** Fallbacks in derived expressions must reference a different value - never the derived variable itself.
- **Viewport → canvas coords** - `x = (clientX - rect.left - vp.x) / vp.zoom`
- **Wire preview start point** - Query `[data-handleid]` via DOM and use `getBoundingClientRect()`; do not compute from canvas coordinates.
- **Store files** - Do not use `.svelte.ts` or `.ts` store files imported from `.svelte` components - Vite cannot resolve them. Use prop drilling or Svelte context instead.
- **Node definitions** - Loaded once in `App.svelte` via IPC `$effect`, passed as `{definitions}` prop to NodeLibrary and NodeEditor.
- **Dynamic paramDefs** - The Resize node updates its `paramDefs` array at runtime via `graphStore.updateResizeParamDefs()` when mode or preserve-aspect changes, removing stale wired edges automatically. `buildResizeParamDefs(mode, preserve)` is the pure function driving this.
- **Properties nodes in batch** - Require per-image evaluation; `hasPropNodes` flag in executor triggers slow path with `magick identify` per image.
- **Format conversion** - `FORMAT:path` prefix forces ImageMagick output format regardless of extension; `-format` is `identify`-only and must not appear in convert operations.
- **Group node ordering** - Group nodes must appear before their children in the nodes array for @xyflow/svelte to render them behind children.
- **`shell.openExternal`** - Must be called from main process via IPC (`shell:open-external`); do not import `shell` in the preload script.
- **NSIS installer patching** - `scripts/patch-nsis.cjs` runs as a postinstall hook and patches `common.nsh` (`ShowInstDetails show`) and `installSection.nsh` (`SetDetailsPrint both`) in electron-builder's templates to display file names and progress during installation.
- **Untrusted workflow files** - `.imgplex` files are shareable and double-click-openable, so their node `data.params` are untrusted. Runtime JS (`command_js` / `compute_js` / `args_js`) is only ever sourced from `node-definitions/` and `format-definitions/` and runs in the main process via `new Function`. To stop a malicious workflow from injecting executable JS, `__`-prefixed param keys (notably `__compute_js__`) are stripped in two places: `sanitizeWorkflowGraph` (`workflow-sanitize.ts`, on load) and `applyParamWires` (`graph-utils.ts`, at runtime). The legitimate `__compute_js__` is re-injected from the trusted node definition in `resolve-params.ts`.
- **Workflow version compatibility** - on load, `compatUtils.isCompatible(fileVersion, appVersion)` checks the saved version against `compat-ranges.json`; files outside the running app's range trigger `IncompatibleVersionModal` instead of loading silently.
- **Update check** - on startup (and via Help → Check for Updates) the main process fetches the latest GitHub release, compares with `compareSemver`, and pushes `IPC.UPDATE_AVAILABLE` to show `UpdateModal`. This is a notification only — no auto-download/install.

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

- `dist/` - renderer bundle
- `dist-electron/` - main + preload bundles
- `node-definitions/` - copied as-is (runtime-loaded, hot-reloaded in dev)
- `dist-cli/` - standalone CLI binary (via `@yao-pkg/pkg`)

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
- Data-driven output formats: `format-definitions/*.json` (JPEG/PNG/WebP/AVIF/TIFF/BMP/TGA) drive both the inspector encoding controls and the ImageMagick args via `args_js`
- Expanded node set: Auto Orient, Extend Canvas, Pixelate, Trim Alpha, Resize (Nearest Neighbor), Quantize, Threshold, Tint, Outline, Extract/Premultiply Alpha, Normal Map Flip, Strip Metadata, Power-of-Two property
- Workflow save / load (`.imgplex` JSON, dirty-state tracking, close confirmation, untrusted-param sanitization, version-compatibility check with `IncompatibleVersionModal`)
- Update-availability notification: startup + Help → Check for Updates fetch the latest GitHub release and show `UpdateModal` (notification only)
- In-app log viewer fed by the main-process logger over IPC
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
- Streaming import with native header parsing (PNG/BMP/WEBP/JPEG/TGA) and batch magick spawning; ~1.6s for ~2000 images (~27× faster than sequential per-image spawning)
- Multi-stream batch pipeline with command fusion (lazy chaining) and single-spawn channel split
- File association - double-clicking a `.imgplex` file opens the workflow in the app; single-instance enforcement on Windows/Linux brings the existing window to the front for subsequent opens
- **Multi-input / multi-output workflow refactor:**
  - Any number of `inputNode` instances, each with its own independent filmstrip; filmstrip tracks the last-selected Input node
  - Three separate output node types: `imageOutputNode` (writes images to disk), `textOutputNode` (writes per-image metadata to a .txt file), `flipbookOutputNode` (assembles images into an atlas)
  - All output nodes require an explicit image wire on `in-0`
  - `traceInputNodeId` BFS helper resolves which input node feeds a given output node
  - **Run Workflow** button (in Inspector panel) validates all output nodes, shows `RunWorkflowDialog` when some are invalid, runs valid nodes sequentially
  - Workflow nodes (Input, Image Output, Text Output, Flipbook Output) draggable from the Node Library **Workflow** section and available in the canvas context menu
  - Deletion guard: last Input node and last output node (of any type) cannot be deleted
  - `graphStore.canDeleteNode()` / `imageStore.removeNode()` wired into all delete paths
- **CLI named-flag system:**
  - Each Input and output node has a `cliName` param (auto-assigned at creation, user-editable in inspector)
  - Inspector shows the resulting flag (`Flag: --<name>`) and a conflict warning when two nodes share the same name
  - `executor-cli.ts` is graph-aware: reads the workflow graph and emits one typed param per named node in all three script formats (PS / Bash / CMD)
  - `imgplex-cli run` parses `--<flag> <path>` pairs, iterates all output nodes, traces each to its input node, and runs `executeBatch` per output; clear error if a required input flag is missing
- Unit tests: Vitest with 473 tests across 29 test files, split into two projects — a Node project for pure logic and a `jsdom` project for Svelte component tests (`@testing-library/svelte`, `*.svelte.test.ts`, resolved against svelte's browser/client build). Coverage via `npm run test:coverage` (`@vitest/coverage-v8`, thresholds enforced).

### Pending - Priority Order

#### P1 - Core usability gaps

1. **Copy / Paste nodes** - No clipboard implementation. Ctrl+C / Ctrl+V for selected nodes (single or multi).

2. **Multi-select operations** - @xyflow/svelte supports box-select, but multi-delete, multi-move, and multi-duplicate interactions need validation. Copy/paste of multi-node selections is the main gap.
3. **Additional Platforms** - imgplex is currently developed and tested primarily on Windows platform. OS X, Linux and Web versions need additional work to be on parity with Windows.

#### P2 - Additional image operations

7. **Watermark / Composite** - Overlay a second image or text onto the source image.
8. **Color Balance** - Separate highlight/midtone/shadow RGB adjustments.
9. **Noise Reduction** - `-despeckle` / `-enhance` / `-noise` operations.
10. **Vignette** - Radial darkening effect.
11. **Border / Padding** - Add solid-color or transparent border. (Partially covered by the Extend Canvas node.)

#### P3 - Advanced graph features

13. **Subgraph / macro nodes** - Save a node selection as a reusable compound node.
14. **Watch mode** - Monitor an input folder and auto-process new files.

#### P4 - Polish & infrastructure

15. **Settings panel** - ImageMagick path override, default output folder, theme (dark/light).
16. **Auto-update** - electron-updater integration for downloading/installing new releases. (Update _detection_ + notification already ships; only auto-download/install is pending.)

## 14. Alternatives Considered and Rejected

| Option                       | Why rejected                                                                                                                                   |
| ---------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| Tauri v2                     | Linux WebKitGTK instability; IPC bottleneck for image data (~200ms/3MB); sidecar lifecycle complexity                                          |
| Tauri + Bun sidecar          | I/O speedup irrelevant when bottleneck is ImageMagick processing time; sidecar adds complexity                                                 |
| LiteGraph.js                 | Canvas-based, 200+ node performance; but original repo 3+ years unmaintained, ComfyUI uses a divergent fork                                    |
| React + React Flow           | Most popular node editor, but Svelte chosen for less boilerplate, no virtual DOM, native Svelte Flow integration                               |
| C++ / Qt 6                   | Good node editors (QtNodes) but learning C++ alongside complex project would slow development                                                  |
| C# / AvaloniaUI              | Viable, but Photino's webview has no Node.js - all native work must go through C#                                                              |
| Godot                        | GPU shaders for preview would be great; rejected because desktop UI infrastructure (inputs, dropdowns, file dialogs) would take weeks to build |
| NW.js / Wails / NeutralinoJS | Older/thinner ecosystems; Wails uses OS webview (WebKitGTK issues on Linux)                                                                    |

---

## 15. Future Considerations

- **GPU-accelerated preview** - brightness, hue shift etc. could be GPU fragment shaders for instant feedback, without changing the architecture.
- **WASM-ImageMagick for web "lite" version** - ~3–5× slower than native but viable for single-image preview in a browser.
