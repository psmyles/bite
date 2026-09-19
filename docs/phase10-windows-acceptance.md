# Phase 10 Windows interaction acceptance

Run the native editor from the repository root:

```powershell
cargo run --offline -p bite-gui-prototype
```

Use a copy of a workflow while checking save behavior. Electron remains the
reference and stays usable throughout these checks.

## File and application flow

- [ ] Cancel Open, Save As, each output browse dialog and each folder dialog;
      the editor stays usable and no field changes.
- [ ] Open a `.bite` workflow, edit it, Save As, close/reopen it and confirm the
      graph, groups, comments, viewport and custom parameter values persist.
- [ ] With unsaved edits, exercise New, Open and window Close. Verify Save and
      continue, Discard and Cancel each perform exactly the selected action.
- [ ] Export PowerShell, Bash and CMD scripts. Confirm each script has a sibling
      `.bite` workflow and exposes every non-empty CLI name once.
- [ ] Run a workflow with two Input and two Image Output nodes using four
      different per-node run folders. Confirm each output follows its own path.

## Clipboard, focus and keyboard

- [ ] Copy connected nodes, start a second BITE process and paste. Confirm node
      parameters and internal links survive the system-clipboard round trip.
- [ ] Type in workflow paths, CLI names, comments, search and inspector text
      fields. While a field owns focus, Space, Tab, Delete, Ctrl+A/C/V/Z/Y/D/G/S
      must edit the field or do nothing to the graph as appropriate.
- [ ] With canvas focus, verify undo/redo, duplicate, delete, copy/paste, group,
      ungroup, save and the Space/Tab creation menu shortcuts.
- [ ] Enter non-Latin text through an IME in a comment and text parameter, then
      save/reopen and confirm it is unchanged.

## Canvas and media interaction

- [ ] Drop a `.bite` file on the window and confirm it opens. Drop an image
      folder and confirm it becomes the default input folder.
- [ ] Give two Input branches different folders. Switch between branches and
      confirm each restores its own filmstrip, selection and preview.
- [ ] Select several filmstrip items in sequence and confirm Preview follows the
      current selection and the selected image-producing branch.
- [ ] Drop a wire on empty canvas, choose a compatible node and confirm creation
      plus connection undo in one step. Invalid types and cycles must be rejected.
- [ ] Create a group around several nodes, move and resize it, save/reopen, then
      ungroup. Children must move with the group and retain absolute positions
      after ungrouping or deleting the group.
- [ ] Resize and edit a comment, save/reopen and confirm both size and text.
- [ ] Connect and reorder Text Output ports, refresh generated text preview and
      compare the shown lines with a real run's text file.

## Display and idle checks

- [ ] At 100%, 125%, 150% and 200% scale, controls remain readable, clickable
      and unclipped after resizing the window.
- [ ] Move the live window repeatedly between monitors with different scale
      factors. Fonts and textures must rebuild cleanly without stale sizing.
- [ ] Leave the editor untouched for five minutes. CPU usage should settle near
      zero and memory should remain stable; input must wake rendering promptly.

The first clean-machine packaged run is a Phase 11 cutover check because no
native package exists during Phase 10.

Automated evidence already recorded on Windows: the 54-test Rust workspace,
strict Clippy, 575 Electron reference tests, graph save/reopen tests, independent
runtime-path tests, exact Text Output preview test, font-atlas rebuild test,
offscreen GUI renders at 1× and 2× scale, and a 10-second idle sample (0.0781 CPU
seconds, -1.40 MiB working-set delta). macOS acceptance remains pending under
the project policy.
