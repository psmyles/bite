# Manual GUI checks

These cover behavior that only exists in the GUI (preview, cancellation, dialogs,
sanitize-on-load) and cannot be asserted by `run-tests.ps1`. Run them against the built
test workflows after the automated suite passes.

## Preview & caching

- [ ] Open `wf-01-fastpath.imgplex`, load `fixtures\main`, select an image. Click through the
      chain nodes — each shows a preview. Edit **Blur sigma**: only Blur re-renders
      (upstream nodes hit the `PreviewCache`; watch for lag / console spam).
- [ ] Toggle `_enabled` off on Brightness/Contrast — preview downstream shows the bypass
      (image passes through unchanged).
- [ ] Open `wf-04-channels.imgplex` — previews on Split Channels (per-channel grayscale),
      Negate, and Merge Channels (yellow-ish for red input) all render.
- [ ] Open `wf-05-meanlogic.imgplex` — Mean/Compare/And nodes show live computed values
      on-canvas (`propParams`); selecting `red_256.png` flips And to `true`.
- [ ] Open `wf-06-setmode.imgplex`, select any `set_alpha_*` file — companion suffixes are
      seeded automatically and the Merge preview shows the packed color.

## Run Workflow dialog & validation

- [ ] In `wf-08-formats.imgplex`, disconnect one Convert Format → Image Output wire. Run
      Workflow: the dialog lists that output node as ⚠️ invalid with a reason; the other six
      run. Reconnect afterwards.
- [ ] With an Input node whose filmstrip is empty, Run Workflow reports the output nodes as
      not ready instead of crashing.

## Cancellation

- [ ] Point `wf-08-formats.imgplex` at a folder with many large images (not the fixtures),
      run, and cancel mid-run. Expect: partial outputs on disk, no error dialog, progress UI
      resets, and a following run works normally.

## Output log

- [ ] Enable **Generate Log** on `wf-01`'s Image Output node, run from the GUI, and confirm
      an `outputlog_YYMMDD_HHmm.log` file appears in the output folder (GUI-only feature —
      the CLI never writes it).

## Workflow load & sanitize

- [ ] Re-open every `wf-*.imgplex` fresh: graph, params, and wires intact; no
      version-incompatibility modal.
- [ ] Copy a workflow, hand-edit the JSON to add `"__compute_js__": "return {}"` inside some
      node's `params`, open it in the GUI, save, and re-inspect the JSON: the `__`-prefixed
      key must be stripped (`sanitizeWorkflowGraph`).
      _Note: the CLI loads workflow JSON without this sanitize pass — flagged as a hardening
      gap during suite design (`command_js` runs only from trusted node-definitions, so the
      strip is defense-in-depth, but GUI and CLI should behave the same)._

## CLI script export

- [ ] File → Export CLI Script for `wf-01` (PowerShell). Run the exported `.ps1`: it invokes
      `imgplex-cli run` with `--in`/`--out` flags and produces the same outputs as
      `run-tests.ps1 -Only wf-01`. Verify a path with spaces survives the escaping.

## Known quirks found while designing the suite (not workflow bugs)

- `flip` node: the command template is hardcoded `-flop`; the _axis_ dropdown is never read,
  so "vertical" also mirrors horizontally.
- `solid_image` declares `executor: "solid_image"` but no handler is registered — the node
  is a pass-through in both preview and batch.
- The CLI does not run `sanitizeWorkflowGraph` on loaded workflows (see above).
- `format-definitions/png.json` uses `-depth 16` for the 16-bit option, which ImageMagick
  ignores for flat-color content (writes 1-bit PNGs). Forcing a true 16-bit file needs
  `-define png:bit-depth=16` instead.
