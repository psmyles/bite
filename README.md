# imgplex

A node-based batch image processing desktop app. Build visual pipelines by connecting nodes on a canvas, then run them against a folder of images.

Built with Electron, Svelte 5, and ImageMagick.

---

## Features

- **Visual node graph** — drag nodes from the library, connect typed wires, see results live
- **Batch processing** — run a pipeline against an entire folder; gate node conditionally skips images
- **Live preview** — real-time pipeline output on the selected image, node-level cached
- **CLI export** — export any workflow as a standalone script (PowerShell, Bash, or Windows Command Prompt)
- **Workflow files** — save and load pipelines as `.imgplex` JSON; double-clicking a `.imgplex` file opens it directly in the app
- **Extensible** — add new nodes by dropping a JSON file into `node-definitions/`, no recompile needed
- **Pure-value graph** — math, logic, and value constant nodes with typed wires route parameters without touching the image pipeline
- **Node groups & comments** — visually organise your graph with resizable containers and sticky notes
- **Undo / redo** — full history for all graph edits

---

## Requirements

- [Node.js](https://nodejs.org) 20+
- [ImageMagick](https://imagemagick.org) 7+ (either installed and on your PATH, or place a portable copy in `resources/imagemagick/`)

---

## Development

```bash
npm install
npm run dev        # Electron + Vite hot reload
```

---

## Build

```bash
npm run build      # Production build + electron-builder packaging
npm run build:web  # Renderer-only build (browser testing)
```

---

## Adding nodes

Nodes are plain JSON files in `node-definitions/`. The app watches this folder and reloads automatically in dev — no restart required.

```json
{
  "id": "posterize",
  "label": "Posterize",
  "category": "Color",
  "inputs":  [{ "type": "image", "label": "Input" }],
  "outputs": [{ "type": "image", "label": "Output" }],
  "params": [
    { "name": "levels", "label": "Levels", "type": "int", "widget": "slider", "default": 4, "min": 2, "max": 256 }
  ],
  "command_template": "-posterize {{levels}}"
}
```

`command_template` is a fragment of ImageMagick arguments with `{{param_name}}` placeholders. For nodes with conditional logic, use `command_js` (returns `string[]`). For pure-value computation nodes, use `compute_js`.

## Tech stack

| | |
|---|---|
| [Electron](https://electronjs.org) | Desktop shell |
| [Vite](https://vitejs.dev) + [vite-plugin-electron](https://github.com/electron-vite/vite-plugin-electron) | Build tooling |
| [Svelte 5](https://svelte.dev) | UI framework |
| [@xyflow/svelte](https://svelteflow.dev) | Node graph canvas |
| [ImageMagick](https://imagemagick.org) | Image processing backend |
| [electron-builder](https://www.electron.build) | Packaging and installers |
