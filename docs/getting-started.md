# Getting Started with imgplex

imgplex is a node-based image processing tool. You build a **workflow** by connecting processing steps (nodes) together visually, then run that workflow across a batch of images. No scripting required - but if you understand it well, it's incredibly fast.

---

## The Interface

```
┌──────────────┬─────────────────────────────┬──────────────┐
│              │                             │  Inspector   │
│    Node      │         Canvas              │  (settings)  │
│   Library    │      (node graph)           ├──────────────┤
│              │                             │   Preview    │
│              │                             │  (live view) │
├──────────────┴─────────────────────────────┴──────────────┤
│                   Filmstrip (your images)                 │
└───────────────────────────────────────────────────────────┘
```

### Left - Node Library

A searchable, categorized list of every available node (Resize, Blur, Flip, Color, Math, etc.). Drag any node from here onto the canvas to add it to your workflow.

### Center - Canvas

The main workspace. You connect nodes here using wires to define how your image gets processed. Every workflow starts with two fixed nodes already present:

- **Input** (`input-1`) - represents the image currently selected in the Filmstrip
- **Image Output** (`output-image-1`) - where the final result gets saved

These nodes cannot be deleted (at minimum one Input and one output node of any type must remain). Everything you build lives between them.

The names in parentheses are **CLI Names** — identifiers used when exporting a CLI script. You can change them in the Inspector.

### Right - Inspector + Preview

**Inspector** (top-right): Shows the parameters of whichever node you have selected. Change a value and the preview updates within milliseconds.

**Preview** (bottom-right): A live render of your workflow applied to the selected image. It automatically shows the result at the last connected node before Output, but you can double-click any node to pin the preview to that point in the pipeline.

### Bottom - Filmstrip

Your loaded images. Click any image to make it the active source (the Input node will use it). Drag image files directly onto the Filmstrip to load them, or use the File menu.

---

## Core Concepts

**Node** - A single processing step. Each node has a named set of inputs, outputs, and parameters. Example: a Resize node takes an image in, scales it, and passes an image out.

**Port** - The connection point on a node. Input ports are on the left side; output ports are on the right side.

**Wire** - A connection between an output port and an input port. Wires carry data between nodes. The color of a wire tells you what type of data it carries (see [Port Types](#port-types) below).

**Workflow** - The complete set of nodes and wires, from Input through to Output. Workflows are saved as `.imgplex` files.

**Batch run** - Processing every image in the Filmstrip through the workflow and saving the results. Configured via the Output node.

---

## Your First Workflow

Here's the simplest possible workflow: load an image, resize it, save it.

1. **Load an image** - drag an image file onto the Filmstrip, or use File → Open Images.

2. **Select the image** - click it in the Filmstrip. The Input node now represents that image.

3. **Add a Resize node** - find "Resize" in the Node Library and drag it onto the canvas.

4. **Wire it up** - drag from the output port (right side) of the Input node to the input port (left side) of Resize. Then drag from Resize's output to the Output node's input. You should see: `Input → Resize → Output`.

5. **Adjust settings** - click the Resize node to select it. The Inspector shows its parameters. Change the width or mode as desired.

6. **Check the Preview** - the Preview panel updates automatically. If it doesn't look right, tweak the parameters.

7. **Configure the Image Output node** - click the Image Output node to select it. The Inspector shows the output folder and filename settings.

8. **Run the batch** - click the **Run Workflow** button at the bottom of the Inspector panel (or use the menu). imgplex will process every image in the Filmstrip and save the results.

---

## Working on the Canvas

### Adding Nodes

| Method            | How                                                                                                       |
| ----------------- | --------------------------------------------------------------------------------------------------------- |
| Drag from Library | Drag a node definition from the left panel; drops where you release                                       |
| Context menu      | Press **Space** or **Tab** anywhere on the canvas; click the node you want                                |
| Wire-to-empty     | Drag from any port into empty canvas space; a filtered context menu appears showing only compatible nodes |

### Wiring Nodes

- Drag from an **output port** (right side) to an **input port** (left side)
- Wire colors must match - you can't connect a number port to an image port
- Each input port accepts **only one wire**. To fan out, connect one output to multiple inputs.
- To remove a wire, click it and press Delete, or drag the end off its port

### Selecting and Moving

- **Click** a node to select it (Inspector updates)
- **Ctrl+click** or **drag a box** to select multiple nodes
- **Drag** any selected node to move it; multi-selected nodes move together
- Click empty canvas to deselect

### Other Operations

| Action             | How                                          |
| ------------------ | -------------------------------------------- |
| Delete             | Select node(s) → **Delete** or **Backspace** |
| Duplicate          | **Ctrl+D**                                   |
| Undo               | **Ctrl+Z** (100 levels)                      |
| Redo               | **Ctrl+Y**                                   |
| Reset zoom         | **Double-click** the canvas background       |
| Set preview target | **Double-click** a node                      |

---

## The Inspector

When you click a node, the Inspector shows its parameters. Changes are immediate - the Preview updates as you type or drag sliders.

Some parameters have a small port icon, meaning they can be **driven by a wire** instead of typed manually. For example, you could connect a Float value node to the Blur node's sigma parameter to control blur strength from outside the node.

**Format-specific settings.** The **Convert Format** node's inspector adapts to the selected output format: JPEG exposes Quality, Chroma Subsampling, and Progressive scan; WebP exposes a Lossless toggle, Quality, and Method; PNG exposes Compression Level and Bit Depth; AVIF exposes Quality and Effort; TIFF exposes the Compression type. BMP and TGA are lossless with no options. Switching the format dropdown updates the controls instantly.

**Thumbnail size.** The Input node's inspector has a thumbnail-size setting (128–2048 px, default 256). It controls the filmstrip thumbnail resolution **and** the resolution the live Preview runs at - lower it for faster imports and snappier previews, raise it for more detail. Batch runs always process at full resolution regardless of this setting.

---

## Groups and Comments

**Comment nodes** are free-text sticky notes for the canvas. Find "Comment" in the Library or use the context menu. Double-click to edit the text.

**Groups** let you collapse related nodes into a tidy box:

1. Select the nodes you want to group (drag-box or Ctrl+click)
2. Press **Ctrl+G** - they're wrapped in a labeled group container
3. Press **Ctrl+Shift+G** on a group to ungroup and restore the nodes flat

Groups can be renamed from the Inspector.

---

## Bypass

Every process node has a **bypass toggle** (a small switch on the node). When bypassed:

- The node is skipped during preview and batch processing
- The image passes through unchanged, as if the node weren't there
- The node stays in the graph so you can re-enable it later

This is useful for A/B testing: bypass a sharpening node to compare before/after without deleting your settings.

You can also wire a **Boolean value node** to a node's enable port to control bypass programmatically.

---

## Port Types

Wire colors indicate what type of data flows through a connection. imgplex will reject connections between incompatible types.

| Color       | Type      | What it carries                          |
| ----------- | --------- | ---------------------------------------- |
| Orange      | `image`   | Full pixel data (the image itself)       |
| Purple      | `mask`    | Grayscale mask image                     |
| Cyan        | `number`  | A numeric value (int or float)           |
| Green       | `string`  | Text                                     |
| Yellow      | `boolean` | True / false                             |
| Pink        | `color`   | A color value                            |
| Amber       | `vector2` | A 2-component vector                     |
| Indigo      | `vector3` | A 3-component vector                     |
| Teal        | `vector4` | A 4-component vector                     |
| Grey        | `numeric` | Any numeric value (number or vector)     |
| Light green | `path`    | A file or folder path string             |
| White       | `any`     | Flexible - adapts to what's connected    |

If you try to connect mismatched types, the wire snaps back. Check the colors on both ports.

---

## Saving and Loading Workflows

- **Save** - File → Save Workflow (or Ctrl+S). Saves as a `.imgplex` file.
- **Open** - File → Open Workflow. Loads a `.imgplex` file and restores the full graph.
- An unsaved change is indicated by a dot (`•`) in the title bar.

Workflow files are plain JSON. You can version-control them.

---

## Exporting as a CLI Script

imgplex can export your workflow as a shell script so you can run it from the command line — no UI needed. Use **File → Export CLI Script** and choose PowerShell, Bash, or CMD. Two files are saved side-by-side:

- **The script** — contains the shell code and usage comments
- **A companion `.imgplex` file** — the workflow the script will run

### Named flags

Each Input and output node in your workflow has a **CLI Name** (visible and editable in the Inspector). This name becomes a command-line flag:

```
# Example: workflow with two inputs and one image output
imgplex-cli run workflow.imgplex \
  --background ./photos \
  --overlay ./overlays \
  --output-image-1 ./out
```

- Input node flags are **required** — the CLI will error if they're missing.
- Output node flags are **optional** — they override the path set in the workflow; omitting them uses the baked-in path.

The exported script documents every flag with its default value and a description. Open the script in any text editor to see the full usage block.

### Changing CLI Names

Select any Input or output node on the canvas. In the Inspector you'll see a **CLI Name** field at the top. Names are auto-assigned at creation (`input-1`, `output-image-1`, etc.). You can rename them to anything using only lowercase letters, digits, and hyphens — the Inspector shows a preview of the resulting flag and warns you if two nodes share the same name.

### Running the script

`imgplex-cli` is installed alongside imgplex and added to your PATH automatically. The Bash and CMD scripts take **positional** values (in the order documented in the script's usage comments); the PowerShell script takes named parameters:

```bash
bash imgplex-batch.sh ./my-photos ./output
```

```powershell
.\imgplex-batch.ps1 -Background .\photos -Overlay .\overlays
```

You can also skip the wrapper script and call the CLI directly with named flags:

```bash
imgplex-cli run workflow.imgplex --background ./photos --overlay ./overlays
```

See the comments inside the exported script for the exact syntax for your chosen shell.

---

## Best Practices

**Build left-to-right.** Start from Input, add transforms in the order you want them applied, end at Output. This keeps the graph readable.

**Use Comment nodes liberally.** Label sections, explain non-obvious parameter choices, mark work-in-progress areas.

**Group sub-pipelines.** If you have a cluster of nodes that together do one logical thing (e.g. "normalize exposure"), group them. Collapsed groups keep the canvas clean.

**Use Bypass for experimentation.** Instead of deleting a node you're unsure about, bypass it. You can re-enable it in one click.

**Set the preview target as you build.** Double-click the last node you added to keep the preview focused on your work in progress, rather than waiting for the full pipeline to complete each time.

**Save frequently.** The undo history lives in memory and is lost if the app closes.

**Test on one image first.** Load a single representative image, build and tune your workflow until the Preview looks right, then load the full batch and run.

---

## Gotchas and Common Mistakes

**You can't delete the last Input node or the last output node.** A minimum of one Input and one output node (Image Output, Text Output, or Flipbook Output) must always remain. If you need to change what they do, use the Inspector or add nodes before/after them.

**Each input port only accepts one wire.** If you need the same value in multiple places, wire the _source_ output to each destination separately - fan out from the source, not into the destination.

**Loops are blocked.** imgplex processes nodes in order, so circular wiring is not allowed. The editor will reject a wire that would create a cycle.

**The Preview runs at thumbnail resolution, not full resolution.** The live preview reuses the Input node's thumbnail (configurable size, default 256 px) as its source, so effects that depend on absolute pixel dimensions can look slightly different from the final batch output, which always runs at full resolution.

**The Preview only renders up to the preview target.** By default this is the last node before Output. If you add nodes after the current preview target, you won't see them until you double-click to update the target (or let it auto-detect).

**Bypassed nodes affect Preview auto-detection.** The preview target walks backward through the chain and skips bypassed nodes. If your last few nodes are all bypassed, the preview may jump further back than expected.

**Wire a value node vs. type a number - they don't mix.** If a parameter has a wire connected to it, the typed value in the Inspector is ignored. Disconnect the wire first if you want to go back to manual entry.

**`.imgplex` files must keep their extension.** Don't rename them to `.json` or anything else - imgplex uses the extension to identify workflow files.

---

## Keyboard Shortcuts Quick Reference

| Key                 | Action                                    |
| ------------------- | ----------------------------------------- |
| Space / Tab         | Open node context menu at cursor position |
| Ctrl+R              | Run Workflow                              |
| Ctrl+S              | Save workflow                             |
| Ctrl+Z              | Undo                                      |
| Ctrl+Y / Ctrl+Shift+Z | Redo                                    |
| Ctrl+D              | Duplicate selected node                   |
| Ctrl+G              | Group selected nodes                      |
| Ctrl+Shift+G        | Ungroup                                   |
| Delete / Backspace  | Delete selected nodes or wires            |
| Double-click node   | Set as preview target (toggle)            |
| Double-click canvas | Reset zoom to 100%                        |
