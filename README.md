# imgplex

Imgplex is a node-based batch image workflow creator and processor. All the processing is handled by ImageMagick, imgplex just makes the string of commands needed to pass onto ImageMagick.

Built with [ImageMagick](https://imagemagick.org/), [Electron](https://electronjs.org/), and [Svelte](https://svelte.dev/).

A functional web version of the application is available at [psmyles.github.io/imgplex](https://psmyles.github.io/imgplex/) that can be used for creating workflows but it can't process images.

---

## Features

- **Visual node graph** - drag nodes from the library, connect typed wires, see results live
- **Multiple inputs and outputs** - any number of Input nodes, each with its own image list; multiple typed output nodes (Image Output, Text Output, Flipbook Output) in a single workflow
- **Batch processing** - run a pipeline against an entire folder; gate node conditionally skips images
- **Set processing** - group images by naming convention (stereo pairs, bracketed exposures, etc.) and process them together as a unit using the `Process as Set` node
- **Live preview** - real-time pipeline output on the selected image, node-level cached
- **CLI export** - export any workflow as a standalone script (PowerShell, Bash, or Windows Command Prompt)
- **Workflow files** - save and load pipelines as `.imgplex` JSON; double-clicking a `.imgplex` file opens it directly in the app; version compatibility is checked on open
- **Per-format export settings** - format-specific encoding controls (JPEG quality/chroma/progressive, WebP lossless, PNG compression, AVIF effort, …) driven by data files in `format-definitions/`
- **Extensible** - add new nodes by dropping a JSON file into `node-definitions/`, no recompile needed
- **Pure-value graph** - math, logic, and value constant nodes with typed wires route parameters without touching the image pipeline
- **Node groups & comments** - visually organise your graph with resizable containers and sticky notes
- **Undo / redo** - full history for all graph edits

---

## Documentation

- [Getting Started](https://github.com/psmyles/imgplex/blob/main/docs/getting-started.md)
- [Node Authoring Guide](https://github.com/psmyles/imgplex/blob/main/docs/node-authoring-guide.md)
- [Project Spec](https://github.com/psmyles/imgplex/blob/main/docs/spec.md)

---

### Requirements

- [Node.js](https://nodejs.org) 20+
- [ImageMagick](https://imagemagick.org) 7+ — Windows bundles a binary; macOS/Linux require `magick` on your PATH

### Development

```bash
npm install
npm run dev        # Electron + Vite hot reload
npm test           # Run the unit test suite (Vitest)
```

### Build

```bash
npm run build      # Production build + electron-builder packaging
npm run build:web  # Renderer-only build (browser testing)
```

---
