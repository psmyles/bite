# Native editor parity inventory

This is the Phase 10 implementation and acceptance checklist. The Electron
application is the behavioral reference; visual details may change when the
native control provides the same workflow more clearly.

## Reference surfaces inspected

- `src/renderer/App.svelte`: application shell, resizable panels, persistence,
  dirty-state prompts, menus, CLI export and update flow.
- `src/renderer/nodeEditor/NodeEditor.svelte`: graph gestures, creation menu,
  grouping, history, keyboard commands, drag/drop and connection flow.
- `src/renderer/nodeEditor/NodeContextMenu.svelte`: searchable/category node
  creation, keyboard navigation, wire filtering and group actions.
- `src/renderer/nodeEditor/undoRedoManager.ts` and `graphTransforms.ts`:
  transaction boundaries, deletion, duplication and CLI-name allocation.
- `src/renderer/components/Inspector*.svelte`: generic and custom inspectors.
- `src/renderer/stores/graph.svelte.ts`: dirty tracking, workflow-node guards,
  selection, viewport and live computed values.

## Graph and editing behavior

| Requirement | Electron behavior | Native status |
| --- | --- | --- |
| Typed wires | Type-colored ports; invalid links rejected | Implemented with type-colored links and core validation |
| Single-input replacement | New incoming link replaces the previous link | Complete |
| Cycle prevention | Link rejected before graph mutation | Complete |
| Background creation | Right-click, Space or Tab opens searchable categorized menu at cursor | Implemented |
| Wire-drop creation | Menu filters compatible nodes and auto-connects | Implemented with one-step undo |
| Selection | Click and rubber-band multi-select | Node-editor selection available; multi-selection drives editing commands |
| Delete | Delete/Backspace; protects final input/output; group deletion ungroups | Implemented |
| Duplicate | Ctrl+D; excludes workflow endpoint nodes; includes group children | Implemented for selected non-endpoint nodes and internal edges |
| Copy/paste | Clipboard graph fragment with internal connections | Windows system clipboard plus in-process fallback implemented; macOS system clipboard pending |
| Undo/redo | Ctrl+Z, Ctrl+Shift+Z/Ctrl+Y; maximum 100 snapshots | Implemented with 100-entry history and redo invalidation |
| Transaction grouping | Drag and compound mutations create one history entry | Drag and wire-create compound transactions implemented |

## Organization

| Requirement | Electron behavior | Native status |
| --- | --- | --- |
| Groups | Ctrl+G, named/resizable group, child-relative positions | Creation, resize/move persistence and relative positioning implemented; interactive acceptance pending |
| Ungroup | Ctrl+Shift+G restores absolute child positions | Implemented |
| Comments | Editable heading/body and resizable card | Creation, inline body editing and resize persistence implemented |
| Panel organization | Resizable library, canvas, inspector/preview and filmstrip | Docked default and per-user layout persistence implemented |

## Inspector

| Requirement | Native status |
| --- | --- |
| Number, text, dropdown and checkbox | Implemented |
| `visible_when` | Implemented |
| `enabled_when` | Implemented |
| Read-only values and live pure-node results | Displayed; read-only presentation needs parity pass |
| `portOnly` and `noPort` | Implemented in inspector and port generation |
| Slider, vector and color controls | Implemented |
| Rename block editor | Text/number/old-name add, edit, reorder, remove and live preview implemented |
| Process As Set suffix editor | Add/edit/remove and matched-set preview implemented |
| Format Convert per-format controls | Active format controls and visibility implemented |
| Folder Path editor | Path editing implemented; native browse dialog pending |

## Workflow and application behavior

| Requirement | Native status |
| --- | --- |
| Multiple inputs and image outputs with CLI names | Implemented with collision-free default CLI names |
| Text and flipbook outputs | Creation, ports and generic/custom values implemented; Text Output maintains one trailing dynamic port; richer output inspectors pending |
| Process As Set | Creation, dynamic typed ports and suffix editing implemented; matched-set preview pending |
| Save/open/new and dirty prompts | Implemented with Save and continue, Discard and Cancel |
| Run, progress and cancellation | Implemented |
| PowerShell/Bash/CMD CLI export | Core generator and native save dialog implemented with companion `.bite` file |
| Output logs | `generateLog` writes a nonfatal summary beside produced files |
| Per-input thumbnail size | Editable and used by import; dedicated presentation pass pending |
| Definition hot reload | Reloads transactionally when the app regains focus; manual reload is also available |
| Update check/dialog | Pending |
| File dialogs and file association | Windows open/save/folder dialogs implemented; file association is Phase 11 and macOS dialogs remain pending |

## Platform acceptance

- Windows: automated context creation and graph persistence pass; native dialog,
  clipboard, keyboard focus, drag/drop, grouping, high DPI, mixed DPI, idle usage
  and clean-machine interaction checks remain.
- macOS: all native GUI checks pending under the project platform policy.
