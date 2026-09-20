# Native editor stack

The native editor is built on Dear ImGui. The node canvas, every panel and every dialog are
drawn by BITE itself; there is no node editor add-on and no docking. The Electron renderer is
the specification, and the parity inventory lives in `docs/native-editor-parity.md`.

## Crates

- `bite-imgui-sys` vendors cimgui and Dear ImGui as Git submodules and compiles them with
  C++17. Fetch them with `git submodule update --init --recursive`.
  - cimgui: `f6fb347cf11217d3ce59fd7956eb3aaf5724518d` (1.90.9 docking branch).
  - Dear ImGui: cimgui's nested submodule `3369cbd2776d7567ac198b1a3017a4fa2d547cc3`.
  - The `imgui-node-editor` submodule is no longer compiled or used. It is left in the
    working tree so existing checkouts keep working; it can be removed in a later pass.

  Bindings cover the whole cimgui surface and are checked in, so an ordinary build needs no
  libclang. To regenerate them:

  ```
  cargo run -p bite-imgui-sys --features bindgen --example generate_bindings
  ```

- `bite-imgui` is the safe wrapper. It owns its context on one thread, copies render data
  into Rust-owned buffers, and exposes fonts, style, draw lists, input and the widget set. It
  has no BITE schema or pipeline dependency. Its test builds the atlas, measures text, renders
  frames and rebuilds the atlas for a new display scale.

- `bite-gui-prototype` is the editor: a winit window, an owned wgpu renderer, and the modules
  that mirror the Electron renderer. `theme.rs` transcribes `src/renderer/assets/theme.css`,
  `shell.rs` reproduces the panel layout from `App.svelte`, `canvas/` is the node canvas,
  `panels/` holds the four panels, `modals.rs` holds the dialogs, and `commands.rs` carries
  the menu and inspector actions.

## Fonts

Atkinson Hyperlegible Next and JetBrains Mono are embedded in the executable at every size
and weight the stylesheet uses. Each face is rasterized so that its em equals the size the
token names, because that is what a stylesheet's `font-size` means, while Dear ImGui's own
size parameter is the distance from ascender to descender. A platform font is merged into the text-entry sizes so that
text typed through an input method renders. The atlas is rasterized at the display scale and
rebuilt when the scale changes.

## Event loop

The loop waits when nothing is happening, with a short redraw burst after input so layout and
hover states settle. Imports, previews, text previews, workflow runs and the update check run
on background threads and wake the loop when they have something to report, so progress
appears without the pointer moving.

## Running

```
cargo run -p bite-gui-prototype
cargo run -p bite-gui-prototype -- workflow.bite
```

## Offscreen capture

The capture mode renders fixed scenes without opening a window, for side-by-side comparison
with Electron. It writes one image per scene: the seed document, a selected node, the creation
menu, a comment card, a slider row, a colour row, a column of process cards, an open menu, an open inspector list, and every dialog.

```
cargo run -p bite-gui-prototype -- --capture test-workflows/out/parity
cargo run -p bite-gui-prototype -- --capture test-workflows/out/parity-wf test-workflows/wf-05-meanlogic.bite
cargo run -p bite-gui-prototype -- --capture test-workflows/out/parity-2x test-workflows/wf-05-meanlogic.bite 2
```

The third argument is the display scale, so the same scenes can be checked at high density.

## Environment

- `BITE_DEFINITIONS_ROOT` overrides where node and format definitions are read from. A
  packaged build looks beside the executable first and falls back to the source tree.
- `BITE_SESSION_PATH` overrides the window bounds and panel size file.
- `BITE_LOG_PATH` overrides the session log.

## Remaining work

The hands-on checklist is `docs/phase10-windows-acceptance.md`. The known gaps are listed
under "Still to do" in `docs/native-editor-parity.md`: the log viewer window, the interface
showcase, inline comment editing on the canvas, dragging Text Output ports to reorder them,
and the macOS system menu. The minimap is deliberately absent; a Fit View control replaces it. macOS acceptance is pending under the project platform policy,
although dialogs, the clipboard, the update check and fonts are now cross-platform.
