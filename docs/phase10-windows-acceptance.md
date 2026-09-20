# Native editor interaction acceptance

Run the editor from the repository root:

```powershell
cargo run --offline -p bite-gui-prototype
cargo run --offline -p bite-gui-prototype -- test-workflows\wf-01-fastpath.bite
```

Use a copy of a workflow while checking save behavior. Electron stays the reference and
stays usable throughout. Keep both applications open side by side for the visual checks.

## Canvas gestures

- [ ] Left drag on empty canvas pans. Middle drag also pans. Right drag does not.
- [ ] The wheel zooms about the pointer and stops at 50% and 200%.
- [ ] Shift and drag draws a selection rectangle, and merely touching a card selects it.
- [ ] Click selects one node; Ctrl and click adds and removes; clicking empty canvas clears.
- [ ] Clicking a node that is already part of a multiple selection keeps the whole selection,
      so the group can be dragged together.
- [ ] Dragging a node moves every selected node, and a group carries its children.
- [ ] A child of a group cannot be dragged outside the group frame.
- [ ] Double clicking a processing node moves the Previewing badge; double clicking an Input,
      an output, a comment or a bypassed node does nothing.
- [ ] Double clicking empty canvas resets the zoom to 100% without changing the pan.
- [ ] Clicking a wire selects it, Delete removes it, and the selected wire is thicker and glows.
- [ ] Dragging from a port snaps to a compatible port within about twenty pixels.
- [ ] An invalid connection is rejected and the status line explains why.
- [ ] Connecting to an input that already has a wire replaces the old one.
- [ ] Dropping a wire on empty canvas opens a filtered menu; choosing a node creates and
      connects it, and one undo removes both.
- [ ] The bypass tick in a card header toggles the node and dims the card.

## Creation menu and library

- [ ] Right click, Space and Tab each open the menu at the pointer.
- [ ] With no search text the menu browses by category and each category opens a flyout.
- [ ] Typing filters to a flat list with the category on the right, and no match reads
      `No matching nodes.`
- [ ] Up and down move the highlight without wrapping; Enter creates; Escape closes.
- [ ] Clicking outside the menu closes it without creating anything.
- [ ] Group Selection and Ungroup appear only when the selection allows them.
- [ ] Dragging a library entry onto the canvas creates it centred on the drop point.
- [ ] Library categories collapse and expand, and a search forces them all open.
- [ ] Hovering a library entry or a menu result shows its description after a short delay.

## File and application flow

- [ ] Cancel Open, Save As, every browse dialog and every folder dialog. The editor stays
      usable and no field changes.
- [ ] Open a workflow, edit it, Save As, reopen it, and confirm the graph, groups, comments,
      viewport and parameter values all persist.
- [ ] With unsaved edits, exercise New, Open and window Close. Save and continue, Discard and
      Cancel each do exactly what they say.
- [ ] The window title carries an asterisk exactly while there are unsaved edits.
- [ ] Export PowerShell, Bash and Windows Command Prompt scripts. Each has a sibling workflow
      file and exposes every non-empty command line name once.
- [ ] Run a workflow with two Input and two Image Output nodes using four different per-node
      folders, and confirm each output follows its own path.
- [ ] Run with one ready and one unready output, and confirm the choice dialog lists both with
      the correct markers and reasons.
- [ ] Cancel a running batch and confirm it stops promptly.
- [ ] The summary dialog reports the counts, the elapsed time and opens the output folder.

## Clipboard, focus and keyboard

- [ ] Copy connected nodes, start a second editor process and paste. Parameters and internal
      links survive the round trip.
- [ ] Type in workflow paths, command line names, comments, search and inspector fields. While
      a field has focus, Space, Tab, Delete and every Ctrl shortcut edit the field and leave the
      graph alone.
- [ ] With canvas focus, verify undo, redo, duplicate, delete, copy, paste, cut, group, ungroup,
      select all, save and the Space and Tab creation menu.
- [ ] Enter non-Latin text through an input method in a comment body and a text parameter, then
      save, reopen and confirm it is unchanged.
- [ ] Every menu accelerator does the same thing as its menu item.

## Media and panels

- [ ] Drop a workflow file on the window and confirm it opens, prompting first when dirty.
- [ ] Drop an image folder and confirm it becomes the scan folder; drop an image and confirm it
      imports.
- [ ] Import a folder and confirm the progress dialog counts up and can be cancelled.
- [ ] Give two Input branches different folders, switch between them, and confirm each restores
      its own filmstrip, selection and preview.
- [ ] Select several filmstrip items and confirm the preview follows the current selection.
- [ ] Scroll the filmstrip with a vertical wheel gesture and confirm it moves sideways.
- [ ] Resize the filmstrip and confirm the thumbnails grow and shrink with the panel.
- [ ] The Info toggle shows and hides the preview overlay, and the overlay reports the file
      name, format, dimensions and size.
- [ ] Drag each splitter to both limits and confirm the clamps and the cursor shapes.
- [ ] Resize the window and confirm the inspector and preview keep their proportion while the
      other panels keep their pixel sizes.

## Inspector

- [ ] Each control type edits its parameter and the card updates immediately.
- [ ] The reset control appears only when a value differs from its default, and the row does
      not shift when it is hidden.
- [ ] A wired parameter shows the source value instead of a control.
- [ ] A computed parameter shows its live value after a preview run.
- [ ] A duplicate command line name is reported in red and a unique one shows its flag.
- [ ] Rename blocks add, edit and remove, and the preview table updates.
- [ ] Process As Set suffixes add and remove, and the matched set list reports the counts.
- [ ] The flipbook atlas summary reports the size, the cell count and the over or under fill.
- [ ] Text Output reports the generated lines and they match a real run's file.
- [ ] Convert Format shows the chosen format's own options.

## Display and idle

- [ ] At 100%, 125%, 150% and 200% scale, controls stay readable, clickable and unclipped
      after resizing the window.
- [ ] Move the live window repeatedly between monitors with different scale factors. Fonts and
      textures rebuild cleanly with no stale sizing.
- [ ] Close and reopen the editor and confirm the window bounds and panel sizes return.
- [ ] Leave the editor untouched for five minutes. Processor use settles near zero, memory
      stays stable, and input wakes it promptly.
- [ ] While a batch runs, progress updates without the pointer having to move.

## Automated evidence already recorded

The Rust workspace passes 177 tests and strict Clippy with no warnings. The offscreen capture
mode renders the seed document, a selected node, the creation menu and every dialog at both
1x and 2x scale:

```powershell
cargo run --offline -p bite-gui-prototype -- --capture test-workflows\out\parity
cargo run --offline -p bite-gui-prototype -- --capture test-workflows\out\parity-wf test-workflows\wf-05-meanlogic.bite
cargo run --offline -p bite-gui-prototype -- --capture test-workflows\out\parity-2x test-workflows\wf-05-meanlogic.bite 2
```

The first clean-machine packaged run belongs to Phase 11, because no native package exists yet.
macOS acceptance remains pending under the project platform policy; the code paths for dialogs,
the clipboard, the update check and fonts are now cross-platform rather than Windows-only.
