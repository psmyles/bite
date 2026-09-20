# Native editor parity inventory

The Electron application is the specification. Each row names the Electron source that
defines the behavior and what the native editor now does. Items marked pending still need
hands-on confirmation; items marked missing are not implemented yet.

## Foundations

| Area | Electron source | Native status |
| --- | --- | --- |
| Interface library | Browser layout and CSS | Dear ImGui through full generated cimgui bindings; the hand-written C shim and the node editor add-on are removed |
| Fonts | `assets/fonts.css` | Atkinson Hyperlegible Next and JetBrains Mono are embedded in the binary at every size and weight `theme.css` asks for, with a platform fallback merged in for text fields so input methods render. Each face is rasterized so its em equals the token's pixel size, which is what `font-size` means; Dear ImGui's own parameter is the ascender-to-descender distance instead. The atlas is built at the nearest whole pixel and drawn at the exact one, and advances are left unsnapped, so a measured string is the width the stylesheet gives it |
| Menu dropdowns | `MenuBar.svelte` | Rows padded five pixels above and below with the accelerator on the right, inside a dropdown padded four pixels above its first row and below its last |
| Inspector dropdowns | `Dropdown.svelte` | The open list gives each row the five by ten padding `.dd-item` names, pads four pixels top and bottom, and scrolls after eight rows |
| Design tokens | `assets/theme.css` | Transcribed into `crates/bite-gui-prototype/src/theme.rs`; drawing code reads tokens, never literals |
| Canvas | `@xyflow/svelte` | A Rust-owned canvas drawn on ImGui draw lists, so gestures and visuals are not constrained by an add-on |
| Panels | `App.svelte` | Fixed three-column shell with invisible six-pixel drag gaps; docking is removed |

## Shell and layout

| Requirement | Native status |
| --- | --- |
| Panel arrangement and sizes | Left 220 (160–400), right 280 (200–480), filmstrip 120 (80–220), inspector 0.65 (0.30–0.80) of the right column, all from the tokens |
| Splitters | The gap is the hit target, with the column and row resize cursors and the same clamps, including the inverted right and filmstrip drags |
| Inspector and preview split | A fraction of the right column, so it rescales with the window |
| Window title | `*name - Bite`, `Untitled - Bite` |
| Panel headers | 36 pixels, header background, 13 pixel semi-bold with the stylesheet's letter spacing |
| Window bounds and panel sizes persist | Implemented, and recorded as a deliberate improvement: Electron persists nothing |

## Graph and editing

| Requirement | Electron behavior | Native status |
| --- | --- | --- |
| Pan | Left or middle drag on empty canvas | Implemented |
| Zoom | Wheel, clamped 0.5 to 2.0, about the pointer | Implemented |
| Rubber band | Shift and drag, selecting anything it touches | Implemented |
| Multiple selection | Platform modifier adds and removes | Implemented |
| Drag threshold | One pixel before a move counts | Implemented |
| Snap to grid | None | Matches: no snapping |
| Double click a node | Toggles the preview target, excluding endpoints, comments and disabled nodes | Implemented |
| Selecting a node | Does not change the preview target | Implemented: only a double click sets it |
| Preview target with nothing chosen | The node feeding an output, or the end of the chain | Implemented, with the Input itself standing in when no processing node is wired, so the selected image always shows |
| Double click empty canvas | Resets zoom to one, keeping the pan | Implemented |
| Right click | Creation menu on empty canvas and on a group; nothing on an ordinary node | Implemented |
| Typed wires | Type-colored, validated before the graph changes | Implemented through `bite_core::graph::validate` |
| Single input replacement | A new wire replaces the previous one on that handle | Implemented |
| Cycle rejection | Rejected before mutation | Implemented |
| Wire drop on empty canvas | Filtered creation menu, auto-connect, one undo entry | Implemented, with the dropped wire still drawn from its port to the menu until the menu closes |
| Port snapping | Twenty pixel radius, scaled by zoom | Implemented |
| Edge selection and deletion | Click to select, Delete to remove | Implemented with a six-pixel curve hit test |
| Wire geometry | Cubic bezier, two pixels, three plus a glow when selected | Implemented |
| Wire color after reopening a file | Electron loses it and draws grey | Deviation: the native editor always derives the color from the source handle |
| Undo and redo | 100 entries, drag and compound transactions | Implemented |
| Parameter edits are undoable | Electron does not record them | Deviation: the native editor records them |
| Copy, paste, duplicate, delete | Endpoint guards, group children included | Implemented, with the system clipboard on every platform |
| Groups and comments | Create, resize, move, ungroup, delete restores absolute positions | Implemented |
| Shortcut suppression while typing | Electron guards only Delete | Deviation: every shortcut stands down while a field has focus |

## Node cards

| Requirement | Native status |
| --- | --- |
| Card width | 150 for process and compare cards, 190 for the built-in workflow cards, 210 for comments, each a floor: the card grows to fit its header, rows and footer as the browser element shrink-wraps, up to a 320 ceiling |
| Enum parameters | Neither a body row nor a port, matching the `type !== 'enum'` filter in `nodeEditorHelpers.ts` |
| Channel count | A definition with a `channels` parameter shows only that many image inputs, from the node's value or the definition default |
| Layout arithmetic | Header 28, port rows 20 with 5 padding, parameter rows 22 with 4 padding, one-pixel separators, footer 22, transcribed from `ProcessNode.svelte` |
| Header tints | 18 percent accent mix per built-in kind, 20 percent for Process As Set, plain for process cards |
| Ports | Ten-pixel circles in the wire color with a two-pixel border and monospaced labels |
| Inline values | Shown only when they differ from the definition default, with the same rounding and truncation rules |
| Computed rows | Value left, label right |
| Output slots | Right-aligned; combined slots show only their label |
| Bypass tick | 14 pixels in the header, hidden when the port is wired, with the same truthiness rule |
| Footers | Image counts, output paths, atlas grid, folder basename, matched set counts |
| Badges | Previewing in neon green, Processing in amber with the current file beneath |
| Selection | White ring, two pixels |
| Bypassed appearance | The card dims to 45 percent |
| Groups and comments | Group frame with a floating label band; comment in the sticky-note palette |
| Comment text | Heading at fourteen pixels bold and body at fourteen pixels, wrapped to the card and clipped at its lower edge |
| Text at zoom | Every label on a card is drawn at the zoom level, so wording keeps its proportion to the card |

## Creation menu

| Requirement | Native status |
| --- | --- |
| Triggers | Right click, Space, Tab, and a dropped wire |
| Placement | Centred on the cursor, or anchored at the drop point for a wire; the panel is only as tall as its rows |
| Search | Case-insensitive substring over label, category and aliases |
| Browse mode | Categories with hover flyouts, all sorted by name | Sorted as `localeCompare` sorts, so `FX` follows `Format` rather than jumping ahead on its capital; a flyout lists one category, so its name is not repeated on every row |
| Keyboard | Up and down move without wrapping; Enter selects; Escape closes |
| Wire filtering | Compatible candidates only, with the parameter type aliases |
| Auto-connect | First matching handle, as one undo entry |
| Group actions | Group Selection and Ungroup rows with their shortcuts |
| Tooltips | Descriptions after 200 milliseconds, wrapped at the 320 pixels `NodeLibrary.svelte` reserves |

## Panels

| Panel | Native status |
| --- | --- |
| Node Library | Filter, pinned Workflow section hidden while searching, collapsible categories forced open during a search, drag to canvas, hover descriptions, both empty states |
| Inspector | Header with the node name, the full dispatch table, and the run action in a bordered footer |
| Preview | Letterboxed on black, Info toggle defaulting to on, gradient overlay with the name and `FORMAT · W x H · size`, both empty states. The chain runs over the cached thumbnail while the overlay measures the original, as `preview-pipeline.ts` does, and the thumbnail stands in until the first render arrives |
| Filmstrip | Status count shown at every count including none; thumbnail size derived from panel height, horizontal scrolling with a vertical wheel, virtualized with overscan, selection border, status count, clickable empty prompt |

## Inspector

| Requirement | Native status |
| --- | --- |
| Row layout | Label above a full-width control, 7 by 12 padding, 25 percent row border |
| Badges | `wired` in cyan, `out` in muted white |
| Reset control | Keeps its space when the value is already default |
| Wired rows | The source value replaces the control |
| Computed rows | The live value from the preview run |
| Widgets | Slider with its numeric box, number, text, dropdown, checkbox, color and vector |
| Slider | Three pixel track, twelve pixel round accent thumb, centred against the number box; not ImGui's own slider, which prints the value on the track |
| Color | Full-width swatch opening a picker. Deviation: Electron draws the saturation square, hue bar and channel sliders inline in the row |
| `visible_when` | Evaluated from the node's parameters |
| Input | Naming, thumbnail size, folder card, subfolder toggle, ten format chips with at least one enforced, scan count, import, individual images, loaded file list |
| Image Output | Naming, output path with browse, overwrite, set naming when Process As Set is upstream, log toggle |
| Text Output | Output file, overwrite, separator with a custom field, port order, log toggle, processing source, generated preview with all five titles |
| Flipbook Output | Output file, overwrite, grid fields, sort order, background color with its wired badge, atlas summary, log toggle |
| Process As Set | Prefix with its wired state, suffix rows, add and remove, matched sets with per-slot markers |
| Rename | Text, number and old-name blocks with their badges, add bar, preview table with example names |
| Format Convert | The chosen format's own parameters, and the no-options message |
| Folder Path, Comment, Group | Implemented |

## Dialogs

Every dialog uses the shared chrome: a dimmed backdrop that also blocks input, a panel with
a two-pixel border and eight-pixel radius, a titled header, and a right-aligned footer.

| Dialog | Native status |
| --- | --- |
| Unsaved changes | Title `Bite`, the three exact messages, Cancel before OK |
| Run Workflow | Ready count, per-node rows with round and square markers, `Run N nodes` |
| Batch progress | Large counter, six-pixel bar, percentage and elapsed time, cancel, error state |
| Batch summary | Processed, skipped and failed statistics, total time, error list, open output folder |
| Import progress | Progress and completed states with the tick badge, the completed line naming how long the import took. Cached thumbnails decode in process, so re-importing a folder costs no ImageMagick at all |
| About | Description, version, dependency table |
| Credits | Library and font sections with licenses and links |
| Update | Checking, available, up to date and failed states |
| Incompatible version | Both message variants and the releases link |

## Application

| Requirement | Native status |
| --- | --- |
| Menu | File, Edit, View, Debug and Help with the Electron labels, accelerators and separators; developer items only in a debug build |
| Shortcuts | Every accelerator plus the canvas editing keys |
| File dialogs | Open, save, folders, images and export destinations on every platform |
| Clipboard | System clipboard on every platform |
| Update check | Silent at startup unless newer; every state from the menu; cross-platform |
| Logging | Session banner, 1000-entry buffer, same file format |
| Cache | Temporary folder, clear action, startup pruning with the same rules |
| Background work | Import, preview, text preview, run and update check all run off the interface thread and wake the event loop. Previews go to one long lived worker that keeps its ImageMagick session, its thumbnail cache and its last two dozen results, and renders only the newest request |
| Drag and drop | A workflow file opens, a folder becomes the scan folder, an image imports |
| Fullscreen | Implemented |

## Still to do

- The minimap is removed. A Fit View control beside the zoom reading replaces it, which is a
  deliberate deviation: the minimap was unreliable and the framing action is what it was used
  for.
- The log viewer opens the file in the platform viewer rather than drawing the Electron log
  window, so the level filters and the `[tag]` highlight are not reproduced.
- The interface showcase window is not implemented; the capture scenes cover the same ground
  for comparison purposes.
- Inline editing of a comment's heading and body on the canvas is not implemented; both are
  edited from the inspector.
- Dragging a node into a group does not re-parent it, which matches Electron.
- Text Output port reordering is display only; the rows cannot yet be dragged.
- macOS uses the same in-window menu bar as Windows rather than the system menu.

## Verification

Offscreen comparison shots are produced by:

```powershell
cargo run --offline -p bite-gui-prototype -- --capture test-workflows/out/parity
cargo run --offline -p bite-gui-prototype -- --capture test-workflows/out/parity-wf test-workflows/wf-05-meanlogic.bite
cargo run --offline -p bite-gui-prototype -- --capture test-workflows/out/parity-2x test-workflows/wf-05-meanlogic.bite 2
```

Each run writes one image per scene: the seed document, a selected node, the creation menu,
a comment card, a slider row, a colour row, a column of process cards, an open menu, an open inspector list, a wire-drop menu, a tooltip, and every dialog. The hands-on checks are listed in `docs/phase10-windows-acceptance.md`.
