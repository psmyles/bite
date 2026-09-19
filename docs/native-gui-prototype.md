# Native GUI prototype

The prototype uses the decided Dear ImGui docking + imgui-node-editor stack.
Vendor sources are Git submodules; use `git submodule update --init --recursive`.

- cimgui: `f6fb347cf11217d3ce59fd7956eb3aaf5724518d` (1.90.9 docking).
- Dear ImGui: cimgui's nested submodule `3369cbd2776d7567ac198b1a3017a4fa2d547cc3`.
- imgui-node-editor: the revision beginning `e78e447` (full commit is in the gitlink).
  Newer upstream `021aa0e` defines `ImGui::GetKeyIndex` unconditionally, causing
  duplicate symbols with 1.90.9; the previous revision builds without local patches.

`bite-imgui-sys` compiles vendor sources plus a small C ABI with C++17. Generated
bindings are checked in. Normal builds do not require libclang. To regenerate:

```
cargo run -p bite-imgui-sys --features bindgen --example generate_bindings
```

`bite-imgui` owns its context on one thread, scopes window/editor calls, and copies
render data into Rust-owned buffers. It has no BITE schema or pipeline dependency.
The Windows MSVC smoke test builds the font atlas, renders a fake node, and checks
that draw data remains valid after context destruction.

`bite-gui-prototype` adds a Winit window and an owned WGPU renderer, with 120 fake
nodes, ten types, typed link colors, a group/comment, docked library/inspector/
preview/filmstrip panels, keyboard/mouse input and file-drop display. It does not
depend on the workflow backend. Its event loop waits when untouched; a finite
redraw burst settles ImGui layout after input. The initial canvas fit waits until
docking has settled and uses zero-duration navigation to avoid fitting against
the initial tiny window bounds.

```
cargo run -p bite-gui-prototype
cargo run -p bite-gui-prototype -- --smoke test-workflows/out/native-prototype.png
```

Windows offscreen smoke rendered all 120 nodes and uploaded textures on an
NVIDIA RTX 4080 (WGPU Vulkan); the exported PNG was visually inspected. This is
rendering evidence, not an interactive acceptance result.

Pending: creation/context menus, layout persistence, clipboard, non-Latin font
coverage and IME preedit/candidate positioning, live DPI font-atlas rebuild,
texture updates, interactive drag/group/selection checks, measured idle CPU/GPU,
mixed-monitor validation, macOS/clean-clone builds, and ImGui Test Engine
evaluation. The GUI go/no-go gate has not passed.
