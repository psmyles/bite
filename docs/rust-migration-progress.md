# Rust migration progress

## Execution policy

The 2026-09-19 user decision permits progression after Windows checks pass.
macOS was stubbed until 2026-09-21 and now builds, runs and packages; its
remaining checks are the interactive ones, listed under Status. Use
`test_images` and generated deterministic fixtures for benchmarks. Keep Electron
usable until parity and cutover gates pass.

## Status

- Phase 0: Windows automated gate passed — 87 reference tests covering all 68
  nodes, seven formats, 11 workflows, and naming/set behavior; 11 real workflows
  passed twice, including recorded ImageMagick calls and CLI contract.
- Phase 1: schema parser and validation CLI implemented; six Rust tests pass.
  Formal schemas derive from serde contracts and include builtin parameter rules.
- Phases 2–3: bounded expression engine and converted definitions implemented;
  all 68 shipped nodes and seven formats compile, with 591 node golden cases
  and format checks. Missing/invalid raw parameter behavior follows documented
  v2 policies. Native executors are explicitly classified.
- Phase 4: all 11 legacy workflows migrate without source mutation and match
  traversal goldens. Typed connections, cycles, containment cycles and supplied
  parameter validation implemented. Regression tests cover constrained value
  wires, downstream consumers, invalid constants and group containment cycles.
- Phases 5–6: Rust CLI runs all 11 workflows and passes differential output
  checks against the original CLI (lossless AE=0, JPEG/AVIF normalized RMSE
  <=0.005). CLI diagnostics/flags, gates, rename, channels, sets, text and atlas
  fixtures pass. Atomic output replacement, Ctrl+C cancellation, deferred plans,
  provenance-bound planning facts and non-ASCII natural atlas ordering are
  implemented. **M2 passes on Windows.** macOS M2 execution is pending under
  the user's platform policy.
- Phase 7: five-format header readers, operation fusion, per-process thread
  limits, progress, parallel scanning, metadata/thumbnail caches, batched
  thumbnails, configurable bounded workers and initial benchmark commands exist.
  **M3 passes on Windows:** Node comparison, peak process-tree memory,
  cancellation latency and the 2,000-image import gate are recorded. macOS is pending.
- Phase 8: pinned C++ submodules, owned C ABI/bindings, safe Rust wrapper and
  Winit/WGPU prototype compile on Windows. Offscreen rendering of 120 nodes,
  docking, inspector and uploaded preview/filmstrip textures was visually
  verified on an RTX 4080. Interaction/DPI/IME acceptance remains pending; see
  `docs/native-gui-prototype.md` for commands and the exact remaining checks.
- Phase 9: **Windows gate passed**; real workflow open/save, graph,
  library, image import/filmstrip, direct transformed preview and background
  execution/progress/cancellation and basic editable inspector controls exist.
  The user confirmed a workflow runs through the native GUI; automated coverage
  supplies save/reopen and zero-pixel-difference CLI comparison evidence.
- Phase 10: editor-parity implementation is complete; the Windows hands-on
  interaction checklist remains before M4 can pass. Electron remains intact.
- Phases 11–13: pending.
- macOS: **builds, runs and packages** as of 2026-09-21 — see the entry at the end of this
  file. Automated checks pass there (workspace tests, the Metal render-stack test, all 25
  capture scenes, a real workflow through the packaged CLI) and
  `packaging/build-mac-dmg.sh` produces a signed image. Still pending: anything needing a real
  pointer (display-scale changes, live resize, full screen, dialogs, drag-and-drop), a
  notarized build (`packaging/build-mac-release.sh` has not been run), and a clean-machine
  install.
- Windows GUI, mixed-DPI/IME, packaging, and clean-machine checks: pending.

Current source, rather than stale fixture documentation, is authoritative: `flip`
already handles both axes, and CLI workflow loading already sanitizes parameters.

## Verification

Windows: `npm test` passed 575 tests in 33 files; `npm run typecheck` passed;
`npm run test:workflows` passed all 11 workflows; `cargo test --offline --workspace`
passes schema, expression, definition parity, graph, header, native-wrapper and
atomic-output tests. Goldens come from the original TypeScript implementation.
Differential run: `test-workflows/out/reference-BIWdO8/results.json`, compared with
legacy `test-workflows/out/reference-EfDzOI`. These run artifacts are ignored by Git;
the runner regenerates deterministic fixtures. macOS remains pending.

## Next work and continuation

Use this log as the continuation checkpoint; do not treat it as completion of
the migration.

1. Complete `docs/phase10-windows-acceptance.md`; then record M4 on Windows.
2. Keep macOS M2/M3 and GUI platform checks explicitly pending.
3. Begin Phase 11 only after the Windows M4 gate. No cutover, release or legacy
   removal has occurred. Work is on branch `rust-migration`.

## Continuation 2026-09-19 — CLI edge cases

Fixed missing set-suffix fallback to the first available input, numeric atlas
filename sorting, and collision-safe numbered output names. Confirmed separate
input branches produce identical results. All five new differential cases pass
with exact pixels (collision images are compared as a multiset because legacy
worker completion order is nondeterministic). Latest case run:
`test-workflows/out/compat-gs9x6x/results.json`. Original goldens unchanged.
The concrete command planner remains the next significant M2 gap.

## Continuation 2026-09-19 — concrete read-only planning

Shared execution now emits typed image/copy/text output operations. Normal runs
retain atomic writes; `bite plan` records the same resolved operations and final
argument arrays with topology, defaults and incoming bindings. It accepts the
run path flags, `--overwrite`, and optional `--facts <json>`. Missing process
analysis or heavy metadata yields `complete: false`, a recorded dependency and
no fabricated results. Facts contain `metadata` keyed by input path and
`captures` entries with exact `args` and string `value`. Fact provenance/type
validation and complete deferred downstream plans remain gaps.

Verified fast-path plan against the untouched TypeScript command golden, and
mean/gate suspension plus replay with explicit facts. CLI smoke planned six
images with BITE_MAGICK pointing to a nonexistent executable and created no
output directory. Workspace tests (24), strict Clippy and all 11 differential
workflow checks pass; latest results: `test-workflows/out/reference-hOd8Cr/results.json`.
M2 remains open; full GUI has not started. Electron and original goldens remain
intact. macOS and interactive Windows acceptance remain pending.

## Continuation 2026-09-19 — planner format/atlas parity and fact validation

Added exact planner comparisons for all format outputs (wf-08) and flipbook
atlas (wf-10), in addition to wf-01. Comparisons use the original TypeScript
command goldens; only the previously documented PNG override is applied to the
expected tokens in memory. All verify planning creates no output directories.

Planning facts now reject unknown/mistyped metadata, relative metadata paths,
negative/nonfinite numeric metadata, duplicate capture argument arrays and
oversized strings. Mean captures reject NaN, infinity, nonnumeric and out-of-range
values. Metadata events distinguish supplied facts. Caller facts still lack
file fingerprints and version provenance; they are not authenticated evidence.
See `docs/rust-planning.md` for the CLI contract and explicit limitations.

Windows workspace tests now pass 26 tests; strict workspace Clippy passes.
These edits only affect planning and its tests; execution was not changed.
Remaining Phase 5 gaps: channel/set fused structural comparisons, native
copy/text/gate/solid coverage, complete downstream representation after missing
analysis, and metadata/capture provenance. Phase 6 compatibility gaps listed
above remain. M2 is still open, full GUI remains gated, and macOS checks are
pending. Electron and original goldens are untouched.

## Continuation 2026-09-19 — native planning and bypass compatibility

Added read-only planner tests for gate-selected copies (wf-07), numbered rename
copies (wf-09), and computed text reports plus image arguments (wf-11). Command
arrays still compare with untouched original goldens; copy sources and report
contents match the deterministic fixture assertions. No output files/directories
are created during planning.

Five new disabled-node differential cases exposed a real rename mismatch.
Legacy batch execution selects the first rename node in global topological
order, even when disabled or disconnected, and applies it only once. Rust now
preserves that naming behavior instead of applying enabled renames along image
streams. Added disconnected/chained rename cases to protect this compatibility.
All 12 edge cases pass with exact pixels: `test-workflows/out/compat-zxVVvE/results.json`.
Workspace tests (27), strict Clippy and runner ESLint pass.

Remaining M2 work: channel/set fused structural-plan comparison, solid-image
plan coverage, full downstream deferred representation after unresolved image
analysis, fact fingerprint/version provenance, shared downstream multi-input
behavior, migration shim/coercion and broader failure/partial-output coverage.
Disabled linear, gate, format, channel and rename behavior is now verified.
Full GUI remains gated; macOS and interactive Windows checks remain pending.

Full 11-workflow differential regression also passes after the rename change:
`test-workflows/out/reference-j5W8Xr/results.json`. Original goldens unchanged.

## Continuation 2026-09-19 — shared-input and solid-source fixes

A differential shared-merge case found Rust emitted no images when branches
originated at different input nodes. Legacy selects one batch input (the traced
input nearest the output) and its lazy materializer reuses that current image
for unseeded input streams. Rust now seeds those streams consistently; it does
not zip separate input folders. The shared-merge case passes exact pixels.

A new solid-image planning test exposed an unreachable native fill: solid_image
has no input port, but the executor required an incoming stream. It now creates
a source stream using the selected batch image dimensions, with enabled fill
and disabled pass-through. Verified in a channel merge with three generated
32x32 fixtures: blue-channel mean is 0.25 within half an 8-bit quantization step.
This checks the documented intentional legacy deviation, not legacy equality.
Solid-only graphs without a traceable batch input still need explicit input
selection semantics/coverage; this test covers a solid branch sharing a merge
with the workflow input.

All 13 differential cases plus the independent solid-fill check pass:
`test-workflows/out/compat-uB5mgq/results.json`. All 11 original workflow image
comparisons pass: `test-workflows/out/reference-WTZMx6/results.json`. Workspace
Rust tests (28), strict Clippy and runner ESLint pass. Original goldens unchanged.

Remaining M2 gaps: channel/set fused structural comparisons, complete deferred
plans after missing analysis, fact provenance, solid-only input selection,
migration shims/coercion and broader failure/partial-output compatibility.
Shared-merge input fallback and solid branches are now covered. Full GUI remains
gated; macOS and interactive Windows acceptance remain pending.

## Continuation 2026-09-19 — partial image failure handling

Characterized a corrupt PNG followed by a valid PNG. Legacy logs the failed
image, continues the batch and returns CLI exit status 0; Rust previously aborted
with status 1 and produced no valid output. Per-image work now records failed
counts and contextual error strings, continues to later images, and reports the
failures through the CLI summary/stderr while preserving the legacy partial-batch
exit status. Workflow/setup errors still fail. Cancellation still aborts. The
read-only planning host explicitly retains fail-fast/suspend behavior so missing
analysis never becomes a falsely complete plan.

New differential case verifies the bad-file diagnostic, matching exit status and
exact valid output pixels. Failure injection verifies existing output preservation,
no temporary-file leaks, continued success after failure, and cancellation before
the next image. Text/atlas final-write failures remain fatal and need separate
legacy characterization; this change covers per-image processing failures.

Windows checks: existing workspace suite plus new failure regression (29 tests
total) pass, strict workspace Clippy and runner ESLint pass. Fourteen differential
cases plus independent solid fill pass in `test-workflows/out/compat-vwjXbD/results.json`.
All 11 workflow comparisons pass in `test-workflows/out/reference-LUVytt/results.json`.
Original goldens unchanged; Electron remains usable.

M2 remains open: fused channel/set structural-plan comparisons, deferred
planning after unresolved analysis, fact provenance, solid-only input selection,
migration/coercion coverage, and aggregate report/atlas error compatibility.
Full GUI remains gated. macOS and interactive Windows acceptance are pending.

## Continuation 2026-09-19 — fused plans and migration shims

Closed the fused native-plan coverage gap. The planner now has exact structural
assertions for channel split/negate/constant-fill/merge and for two process-as-set
groups. The assertions cover source selection, parentheses, channel ordering,
operation ordering, output naming and the absence of intermediate-file writes.
They complement the original TypeScript process goldens, whose temporary-file
shape intentionally differs from Rust's fused operation.

Added a synthetic v1 migration regression alongside all eleven fixture checks.
It covers workflow-input/workflow-output/processNode aliases, structured set
suffixes, numeric enum coercion, removal of executable `__*` parameters and
edges, dynamic set outputs, folder/prefix/suffix handle shims, generic parameter
and text-slot handles, legacy format quality mapping, rename/text structures and
string flipbook color fallback. Unknown legacy UI fields are discarded while
the original input JSON remains unchanged.

Windows verification: the Rust workspace now passes 31 tests, strict workspace
Clippy passes, and 48 focused original TypeScript CLI/connection/traversal tests
pass. Original goldens remain unchanged. Electron remains usable.

M2 remains open for complete deferred planning after unresolved analysis, fact
fingerprint/version provenance, solid-only input selection, runtime parameter-wire
coercion/default edge cases, and aggregate text/atlas failure compatibility.
Full GUI remains gated. macOS and interactive Windows acceptance remain pending.

## Interactive review 2026-09-19 — native prototype typography and theme

The Windows prototype review found that the technical UI still used Dear
ImGui's small bundled pixel font and stock dark colors. The native wrapper now
loads Inter at 16 px from the normal per-user or system font locations, with a
`BITE_UI_FONT` override and the bundled font retained as a portable fallback.
This keeps font selection on the Rust side and lets later packaging provide an
explicit font asset without another C ABI change.

The initial atlas is rasterized at 16 px times the window DPI scale and uses
the reciprocal ImGui font scale for 16 logical-pixel layout. This avoids
bilinear enlargement of a low-resolution atlas on scaled Windows displays.
Live atlas rebuilding after moving a window between unlike-DPI monitors remains
future full-GUI work.

Applied Fire-aligned metrics and colors to ImGui and the node editor: roomier
frames and items, restrained rounding, charcoal surfaces, subtle borders and
grid, and a consistent blue interaction accent. The generated 1600x1000 smoke
render is `test-workflows/out/native-prototype-inter.png`; it confirms Inter is
in the atlas and the stock blue title bars and purple canvas are gone.

Windows verification: the `bite-imgui` and `bite-imgui-sys` tests pass, strict
workspace Clippy passes, the 120-node GPU smoke render completes on Vulkan, and
`git diff --check` reports no patch errors. The recurring incremental-cache
cleanup warning remains non-fatal. This is prototype polish within the existing
Phase 8 spike; the full GUI remains gated on M2. macOS font discovery and visual
acceptance remain pending.

## Continuation 2026-09-19 — solid-only batches, parameter wires, and reports

GUI polish is paused at the user's direction until the remaining migration
gates are complete.

Closed the solid-only input-selection gap. When a Solid Image is connected
directly to an output, the planner and executor now use the sole workflow Input
as the batch and dimension source even though it is not connected by an image
edge. Missing and ambiguous multiple Inputs produce explicit errors. A concrete
plan regression verifies the selected input, native fill arguments, output path,
and the absence of planning writes.

Runtime parameter wires now preserve the compatible legacy Boolean/number
coercions before expression evaluation, including numeric node inputs and the
`_enabled` bypass port. Text Output conditions receive the same truthiness
conversion. Enum parameters are now correctly classified as string wires rather
than number wires. Coverage also verifies unwired defaults remain active.

Text Output now matches two legacy aggregate cases: a fully filtered batch
creates no report, while included rows whose values are all empty fail with no
file written. The earlier Rust path incorrectly created an empty file in both
cases. An injected flipbook montage failure is also verified as fatal, with an
existing atlas preserved atomically and no temporary file leaked.

Windows verification: all 35 Rust tests and strict workspace Clippy pass. All
15 live compatibility cases pass with exact pixels or their documented solid
fill assertion in `test-workflows/out/compat-Tw7UnD/results.json`. The complete
11-workflow legacy run is `test-workflows/out/reference-jm97Yx/results.json` and
the Rust comparison is `test-workflows/out/reference-pfvDyT/results.json`; all
pass and original goldens remain unchanged. Focused test-runner ESLint passes.
The repository-wide `npm run lint` still reports three pre-existing renderer
issues in `RunWorkflowButton.svelte` and `UpdateModal.svelte`.

M2 remains open for complete downstream deferred plans after unresolved
analysis and authenticated planning facts with file/ImageMagick provenance.
Locale-sensitive non-ASCII atlas ordering is also unverified. The full GUI
stays paused; macOS acceptance remains pending.

## Continuation 2026-09-19 — deferred plans and provenance-bound facts

`bite plan` now continues across independent inputs and outputs after missing
analysis. It returns deduplicated stable dependency IDs and structured
`deferred_outputs` records containing the blocked input, output, dependency IDs
and remaining topology operations. Invalid supplied facts remain fatal; no
placeholder value is used to choose a gate or branch.

Added `bite observe`, which executes only analysis dependencies, iterates until
the plan is concrete, and writes a versioned facts document. Facts are sealed
to the workflow plus loaded definition versions, the ImageMagick binary
contents, and analyzed source files using size, nanosecond modification time
and a deterministic content digest. `bite plan --facts` rejects missing
provenance, modified inputs, changed workflow/definitions and a different
ImageMagick binary.

Windows verification: all 36 Rust tests and strict workspace Clippy pass. An
end-to-end observation of wf-05 over all 26 `test_images` collected 78 channel
captures, and replay produced a complete plan without creating its configured
image-output directory or text report. `git diff --check` passes. Original
TypeScript goldens and Electron sources remain unchanged. All 15 live
compatibility cases still pass in
`test-workflows/out/compat-dNeLZU/results.json`, and focused runner ESLint passes.

The two recorded planning gaps are closed. Remaining M2 acceptance work is the
locale-sensitive non-ASCII atlas ordering check and final Windows CLI review;
macOS execution remains recorded as pending. GUI work stays paused.

## Continuation 2026-09-19 — M2 Windows gate complete

Added deterministic accented, mixed-case and numeric atlas fixtures to the live
Electron/Rust differential suite. The first run exposed a real mismatch because
Rust compared accented Unicode code points directly while Electron uses base-
sensitive locale collation. Rust now folds common Latin filename characters to
their base forms before numeric comparison and retains stable import order for
collation ties.

Both ascending and descending non-ASCII atlases now match Electron exactly, as
do the prior 15 compatibility cases. The 17-case result is
`test-workflows/out/compat-n1ufZy/results.json`; the new focused unit regression
also passes. This closes M2 on Windows. macOS M2 execution remains explicitly
pending, and full GUI work remains paused while Phase 7 proceeds.

## Continuation 2026-09-19 — Phase 7 import services and bounded workers

Added parallel deterministic folder scanning and a native thumbnail importer.
It uses the PNG/JPEG/WEBP/BMP/TGA header fast path, JPEG decode-size hints,
batched ImageMagick commands, a source-mtime-validated WebP disk cache, and a
TTL/source-fingerprint-validated memory metadata cache. The execution host now
also caches workflow metadata with the same expiry and source-change behavior.

Image-output operations run through a bounded worker pool. `bite run --jobs N`
configures it; the default is half the available hardware threads, clamped to
one through eight. Atomic staging, deterministic collision assignment, progress,
per-image failures and cancellation remain intact. Aggregate text and flipbook
outputs retain their ordered execution path.

Added `bite bench-import` for scan plus cold/warm thumbnail timing and
`bite bench-workflow` for repeated workflow throughput, ImageMagick process
counts and metadata cache statistics. On the 26 checked-in `test_images`, a
128-pixel import used seven batched processes in 1633 ms and the warm pass used
zero processes in 2.7 ms. The fast-path workflow took 7208 ms with one worker
and 2433 ms with four workers on this Windows machine (debug build); all 26
outputs succeeded in both runs. These figures establish the harness rather than
the release-performance gate.

Windows verification: all 41 Rust tests and strict workspace Clippy pass. The
17 live Electron/Rust compatibility cases still pass in
`test-workflows/out/compat-uhcFwP/results.json`, including both non-ASCII atlas
orders. `git diff --check` passes. Remaining M3 work is an automated Node/Rust
release benchmark matrix, peak-memory and cancellation-latency measurement,
large deterministic/real workload runs, and macOS execution (pending).

The first automated release comparison is now available through
`npm run bench:rust-migration`. On the same 26-image fast-path workload, Node
completed in 1713/1704 ms and Rust with eight workers in 1895/1918 ms; both
produced all 26 files. Cold thumbnail import took 1137 ms in Node and 867 ms in
Rust; warm import took 3.6 ms and 2.2 ms respectively, with Rust spawning zero
processes on the warm pass. Warm no-work CLI startup was 38–41 ms for Node and
7–8 ms for Rust. Results are in
`test-workflows/out/benchmark-2026-09-19T09-28-31-413Z/results.json`. This closes
the small-workload startup/import/throughput harness item. Peak memory,
cancellation latency, the approximately 2,000-image workload, and macOS remain
before M3 can pass.

## Continuation 2026-09-19 — M3 Windows gate complete

The deterministic 2,000-image mixed-format import workload is derived from the
checked-in `test_images`. Node completed cold/warm import in 22.07 s / 276 ms;
Rust completed it in 18.01 s / 193 ms using 250 batched processes cold and zero
warm. All 2,000 images were returned by both implementations.

Added cooperative cancellation benchmarking. The first large-queue run exposed
workers draining cancelled queue entries; workers now stop dequeuing immediately
after cancellation. Five 2,000-image runs returned 9–43 ms after cancellation,
with at most the eight already-running ImageMagick children needing termination.

Windows process-tree sampling and native child working-set tracking now provide
peak-memory evidence. In the final 26-image release matrix, Node used about
2.66 GB at peak and completed in 1672–1725 ms; Rust used 1.31–1.39 GB and
completed in 1858–1886 ms. Rust cold thumbnail import used about 338 MB. Startup,
warm cache, import throughput, workflow throughput, process count, cache-hit
rate, peak memory and cancellation latency are now tracked. The final report is
`test-workflows/out/benchmark-2026-09-19T09-39-44-764Z/results.json`.

Rust is faster for startup and import, uses roughly half the peak process-tree
memory, and is 8–11% slower on the representative fast-path workflow. This is
comparable with no major Windows regression, so M3 passes on Windows under the
user's platform policy. macOS M2/M3 remains pending. The user's prototype review
accepted the GUI direction; Phase 9 can proceed without further visual polish.

## Continuation 2026-09-19 — Phase 9 functional GUI begins

The native application now loads the real node/format registry and workflow
model instead of displaying only fake nodes. It opens and migrates `.bite`
files, draws their actual nodes and edges, updates persisted node positions,
adds Input/Image Output and any of the 68 processing definitions, and validates
new links through `bite-core` before accepting them. The studio model has an
automated regression that creates the primary Input → Resize → Sharpen →
Format Convert → Image Output workflow, saves it, reopens it and verifies its
five nodes and four edges.

Workflow execution calls `bite_core::execution::run_workflow` directly on a
background thread, with progress-independent window responsiveness and a Cancel
button wired to the shared cancellation token. Open, Save, input/output path
fields and drag-and-drop workflow/folder selection are functional. A headless
GUI-path run processed all 26 `test_images` outputs successfully.

Image import uses the Phase 7 scanner and thumbnail cache, converts thumbnails
to uploaded WGPU textures, and fills the native Filmstrip. The new direct
`bite_core::preview::render` API executes a selected image through the graph,
returns an in-memory PNG plus resolved pure-value contexts, and feeds the
Preview and Inspector without spawning the CLI.
The functional offscreen result is
`test-workflows/out/native-functional-preview.png`; it shows the migrated wf-01
graph, 26 real imported images and a transformed preview. The original 120-node
technical smoke mode still passes. All 42 workspace tests, strict workspace
Clippy and the studio regression pass.

The GUI direct-core run path and `bite run` each processed all 26 checked-in
images through `wf-01-fastpath.bite`. Twenty-three outputs were byte-identical;
the three PNG files with differing encoder metadata had ImageMagick absolute
pixel error (AE) zero. This completes the automated CLI comparison portion of
the Phase 9 gate.

Scalar, integer, string and Boolean inspector values are now editable, while
structured/vector/custom editors remain Phase 10 work. Filmstrip selection now
reruns the direct preview for the chosen source. The Phase 9 feature set is
implemented. Its remaining exit gate is the interactive Windows
create-preview-save-reopen-run sequence. Background runs display per-file
progress and support cancellation. Visual polish remains intentionally deferred.

## Continuation 2026-09-19 — Phase 9 acceptance fixes

The first interactive review found that filmstrip selection appeared stuck,
enum parameters were rendered as text boxes, and single-line text fields were
incorrectly rendered as 100-pixel-high multiline controls with labels clipped
at the panel edge. The ImGui wrapper now has native combo and checkbox controls,
single-line text input and full-width item sizing. The inspector uses definition
labels and enum options, and workflow paths use compact labeled fields.

Filmstrip previews now use a texture id specific to the selected image and show
the selected source path. Parameter changes also refresh the current preview
without requiring another import. The functional smoke explicitly selects the
second imported image; `test-workflows/out/native-functional-controls.png`
shows that image in Preview and as the selected Filmstrip item, alongside the
corrected Resize controls. This prepared the corrected build for interactive
confirmation.

## Continuation 2026-09-19 — Phase 9 Windows gate passed

The user confirmed that a workflow runs through the native GUI after the
acceptance fixes. Together with automated workflow creation, save/reopen,
transformed preview, 26-image execution and zero-pixel-difference CLI comparison,
this satisfies the Phase 9 Windows exit gate. The current interface remains a
technical functional shell; Electron-level editor organization, interaction
flow and daily-use behavior are the explicit scope of Phase 10. macOS Phase 9
execution remains pending under the user's platform policy.

## Continuation 2026-09-19 — Phase 10 editor parity foundation

Phase 10 now has a source-backed parity inventory in
`docs/native-editor-parity.md`. The native editor implements a bounded 100-step
undo/redo history, drag and compound transaction boundaries, dirty-state
restoration, copy/paste/duplicate/delete, endpoint deletion guards, groups,
ungrouping and comments. Groups retain child-relative positions and deleting a
group restores absolute child positions. Keyboard commands are suppressed while
text input owns focus.

The canvas has a searchable right-click creation menu and supports dropping a
wire on empty canvas, creating a node and connecting its first compatible port
as one undo operation. Links use their actual wire type for color. Native
creation now covers multiple inputs and image outputs, Text Output, Flipbook
Output, comments and all processing definitions with collision-free CLI names.
The default dock layout gives Workflow and Library a stable left column instead
of floating Workflow over the canvas.

The generic inspector now implements definition ordering, `visible_when`,
`enabled_when`, read-only state, `portOnly`, `noPort`, sliders, vectors, colors,
dropdowns and checkboxes. Convert Format loads the selected format's own
parameters and visibility rules. Structured controls edit Rename blocks,
Process As Set suffixes and Text Output port slots. Process As Set outputs track
the suffix list and remove stale connections safely, while Text Output keeps one
trailing connection port automatically. Rename live filename preview,
matched-set preview and richer output-specific inspectors were the next gaps.

CLI export moved into `bite-core` and generates escaped PowerShell, Bash and CMD
scripts plus a companion workflow from the native UI. The current controls
derive the export filename from the workflow path; native save/open/folder
dialogs remain to be added.

The creation menu now also opens from Space or Tab, groups definitions by
category, filters wire-drop results to compatible ports, and auto-connects the
selection. Definition files reload transactionally when the application regains
focus, with a manual reload control for immediate feedback. Dock layout now
persists in the user's application-data directory.

Native groups and comments persist node-editor resize operations; comments can
edit their body inline. Rename shows live filename results for imported images
or deterministic examples, and Process As Set shows matched/complete set
counts. Core execution now honors `generateLog` by writing a nonfatal output
summary next to produced files.

All 54 Rust tests and strict workspace Clippy pass. The offscreen acceptance
image is `test-workflows/out/native-phase10-outputs.png`. Remaining Phase 10
work is the hands-on Windows interaction gate: native dialogs, system clipboard,
keyboard focus, drag/drop, group containment/resize and live mixed-DPI movement.
The packaged clean-machine run belongs to Phase 11. macOS Phase 10 acceptance
remains pending.

Windows native dialogs now cover workflow open/save-as, input and output folder
selection, Folder Path browsing and CLI export destinations. Copy/paste uses a
versioned JSON graph fragment on the Windows system clipboard while retaining
the in-process fallback. Fragment round-trip and internal-edge preservation are
covered by the Rust suite. macOS native dialogs and system clipboard remain
pending under the platform policy.

Input and Image Output nodes now accept independent runtime folders, so a GUI
run can supply different paths to each CLI-named endpoint. The selected Input's
thumbnail size controls import generation. Image Output exposes the Folder Path
port and hides its local folder controls while wired; set naming appears only
when Process As Set is upstream. Text and Flipbook outputs have native file
dialogs, readable choice labels, ordered connected-port labels and an atlas
size/capacity summary. Each Input branch now retains its own image paths,
thumbnail textures, selection and preview state. Preview follows the selected
image-producing node, and Text Output can evaluate the first ten active images
through the normal core execution path into an isolated temporary report.

The functional GUI smoke passes at both 1× and 2× scale; the latter is captured
in `test-workflows/out/native-phase10-hidpi.png`. The binding can rebuild and
re-upload its font atlas when Winit reports a scale change. Moving a live window
between mixed-DPI monitors still requires hands-on Windows acceptance. A hidden
Windows idle sample consumed 0.0781 CPU seconds over 10 seconds while working
set fell from 182.03 MiB to 180.63 MiB, consistent with event-driven idle.

The native Windows build now checks the GitHub releases API over WinHTTP on a
background thread, compares the release tag against the product version and
shows an Update Available dialog without blocking editor input. Manual checks
also report the up-to-date and error states. macOS update checking remains
pending.

## Continuation 2026-09-20 — Electron visual parity in progress

The native editor now treats the checked-in Svelte components and
`src/renderer/assets/theme.css` as its visual specification. The first shared
native primitives reproduce the Electron application menu, dark surface and
field tokens, 150-pixel node cards, colored node headers, typed port labels and
circular port handles. File/Edit/View/Help commands are exposed through the
menu bar, while the prototype-only Workflow controls are hidden by default and
remain available through File > Workflow settings.

The default arrangement now places Library at left, Canvas in the center,
Inspector above Preview at right, and Filmstrip across Library plus Canvas. A
versioned layout file prevents saved prototype docks from restoring the obsolete
arrangement. Library has a filter and category grouping, and an empty Filmstrip
no longer displays generated placeholder thumbnails. The native color and grid
tokens now come from the Electron theme rather than generic Dear ImGui defaults.

All 54 Rust tests and strict workspace Clippy pass after this slice. Remaining
visual work includes replacing the visible docking tab treatment with Electron
panel headers and gap cards, completing the menu command set, matching filmstrip
status/selection presentation, and tightening Inspector row layout. A final
Windows screenshot comparison and interaction pass remains required; macOS stays
pending under the user's platform policy.

The next parity slice fixes port interaction at the node-editor level: each
colored circle now owns an explicit 14-pixel hit rectangle and point pivot, so a
wire begins at the port instead of its text label. The create-node popup uses
Workflow and definition-category submenus while browsing, switches to flat
compatible results only after text is entered, and retains wire-drop filtering.

Dock tabs are hidden behind Electron-style panel headers. The default widths now
track the Svelte 220-pixel Library and 280-pixel right column at the reference
window size, Inspector/Preview uses the source 65/35 split, Filmstrip uses the
source 120-pixel default, and six-pixel separators reproduce the shell gaps.
Built-in Input and Image Output cards gained their source footers. Empty Preview
and Filmstrip surfaces no longer render technical placeholder images. Filmstrip
items now show their file names, active border and total image count, and the
Inspector exposes the primary Run Workflow action.

`test-workflows/out/native-parity-current.png` is the current source-backed
offscreen comparison. Remaining work is interactive Windows confirmation of the
new pin and submenu hit behavior, plus further inspector control-row refinement
where individual specialized node editors still differ from Svelte.

## Continuation 2026-09-20 — native editor rebuilt for Electron parity

The native editor was rebuilt against the Electron renderer as its specification, because
the previous shell could not reach parity from where it stood. The user's scope decisions
for this pass were: a fixed Electron layout with drag splitters instead of docking, the
bundled Electron fonts, replicate intended behavior while fixing obvious bugs, a Rust-owned
canvas drawn on ImGui draw lists instead of the node editor add-on, full generated cimgui
bindings instead of a hand-written shim, persisted window bounds and panel sizes, and
keeping two places where the native editor is already better than Electron.

### Foundation

`bite-imgui-sys` now generates bindings for the whole cimgui surface, so measurement, style,
draw lists, popups, tooltips, tables, child windows and multiple fonts are all reachable from
Rust. The 80-function hand-written C shim and the imgui-node-editor submodule are gone; the
only C++ compiled is Dear ImGui itself plus cimgui. `bite-imgui` is a new safe wrapper split
into fonts, style, drawing, input and widgets.

The shell underneath it changed in September 2026 (`docs/sokol-migration-plan.md`): winit still
owns the window, the event loop and input, but the drawing API is sokol_gfx on a D3D11 device
the shell creates, and Dear ImGui - now 1.92.9b - is drawn by sokol_imgui rather than by a
renderer of ours. wgpu, pollster, `renderer.rs` and `shader.wgsl` are gone with it. A warm start
reaches its first frame in 145 ms rather than 424, and settles at 57 MB rather than 168.

Atkinson Hyperlegible Next and JetBrains Mono are embedded in the binary at every size and
weight `theme.css` uses, with a platform fallback merged into the text-entry sizes so input
methods render. Inter and the `BITE_UI_FONT` override are removed. `theme.rs` transcribes
every token from `src/renderer/assets/theme.css`, and drawing code reads tokens rather than
literals.

### Editor

The shell reproduces `App.svelte`: a left library, a centre canvas with a filmstrip beneath,
and a right column holding the inspector above the preview, with the six-pixel gaps as the
drag targets and the same clamps, including the inverted right and filmstrip drags and the
inspector's fractional split. Docking and its layout file are removed.

The canvas is Rust-owned. Pan, wheel zoom about the pointer clamped to 50–200%, shift
rubber-band selection with partial hit testing, modifier multiple selection, the one-pixel
drag threshold, double-click preview targeting, wire selection and deletion, twenty-pixel
port snapping, single-input replacement, cycle rejection, wire-drop creation with
auto-connect as one undo entry, groups, comments, the minimap and the zoom label all follow
the Svelte Flow configuration the Electron editor uses. Node cards are measured from the
`--node-layout-*` arithmetic in `ProcessNode.svelte`, including header tints, port rows,
inline value formatting, computed rows, output slots, footers, badges and the bypass tick.

The creation menu, node library, inspector, preview and filmstrip are ported component by
component, including every empty state, hint and validation message. All nine dialogs are
ported with the shared modal chrome and the exact wording. The menu matches
`electron/main.ts` label for label.

File dialogs, the clipboard and the update check are now cross-platform rather than
Windows-only, through `rfd`, `arboard` and `ureq`. Import, preview, text preview, runs and
the update check run off the interface thread and wake the event loop, so progress no longer
waits for pointer movement.

### Deliberate deviations

Window bounds and panel sizes persist; Electron persists nothing. Inspector parameter edits
are undoable; Electron records only canvas operations. Wire colors always derive from the
source handle; Electron loses them on save and reload. Every shortcut stands down while a
text field has focus; Electron guards only Delete. A dropped workflow file opens; Electron
ignores it.

### Verification

The Rust workspace passes 177 tests, up from 54, and strict Clippy passes with no warnings.
An offscreen capture mode renders the seed document, a selected node, the creation menu and
every dialog at 1x and 2x into `test-workflows/out/parity*`; the shots were reviewed against
the Electron sources and the remaining differences are recorded in
`docs/native-editor-parity.md`. The editor was launched on a real workflow and ran without
error. Electron and the original goldens are untouched.

Remaining work is listed under "Still to do" in the parity inventory: the log viewer window,
the interface showcase, inline comment editing on the canvas, dragging Text Output ports to
reorder them, and the macOS system menu. The hands-on checklist in
`docs/phase10-windows-acceptance.md` has been rewritten around the new gestures and is the
remaining M4 gate; macOS acceptance stays pending under the platform policy.

## Native editor: correction pass after first hands-on use (2026-09-20)

The first interactive session with the rewritten editor found eleven faults. Each is fixed
and, where it could be, covered by a test or a capture scene.

Wrapper gaps that caused visible faults:

- Hover tooltips rendered one character a line. `text_wrapped` pushes a wrap position of
  zero, which means "the window's right edge"; in an auto-sized tooltip that edge is itself
  derived from the content, so the text collapsed. `Ui::text_wrapped_at` names the width
  instead, and the tooltip uses the 320 pixels `NodeLibrary.svelte` reserves for it.
- Dragging a library entry onto the canvas did nothing. `BeginDragDropTarget` binds to the
  last item, and a panel that paints itself has no such item, so the drop was never seen.
  `Ui::drag_target_rect` wraps `BeginDragDropTargetCustom` and names the canvas rectangle.
- Card text did not scale with the zoom, so labels overflowed their cards away from 100%.
  `DrawListRef::scaled` multiplies every face size, and the node, group, comment, port, row
  and badge painters all draw through a scaled list.

Editor faults:

- A comment's heading and body were never painted; only the sticky-note shape was. Both are
  drawn now at the fourteen pixel sizes `CommentNode.svelte` uses, wrapped to the card.
- Selecting a node retargeted the preview. That moved the PREVIEWING badge onto whatever was
  selected and, when the selection produced no image, left the preview panel empty, which is
  why a filmstrip selection appeared to do nothing. Only a double click sets the target now,
  as in Electron.
- The creation menu forced the first category's flyout open and closed a flyout as soon as
  the pointer left its row, so no entry in any flyout could be reached. The open category is
  explicit state that outlives the row hover.
- The menu's search field never took the keyboard: the window carried `NoNav`, which makes
  ImGui skip it when placing focus. The flag is gone and the window takes focus on the frame
  it opens.
- A created node was centred using a guessed card width and a fixed 58 pixel height, which
  put it somewhere the pointer had not been. The card's corner now lands on the pointer, for
  both a library drop and a menu choice.
- The minimap is removed at the user's request. A Fit View control sits beside the zoom
  reading, and the canvas ignores gestures inside it.
- The inspector slider was ImGui's own, which prints the value on the track. It is now the
  three pixel track and twelve pixel round thumb from `InspectorParamEditor.svelte`, level
  with its number box. The colour parameter was ImGui's inline editor, four drag fields in a
  250 pixel panel; it is a full-width swatch that opens a picker.
- Every per-role face was re-checked against `theme.css`. One discrepancy surfaced:
  `--text-thumb-name-size` says ten pixels but `Filmstrip.svelte` overrides it with
  `--font-size-xs`, so the rendered size is eleven. The native editor follows the component,
  and the token now carries a note saying why. Badges also gained the six pixel gap and the
  vertical centring `.param-label` gives them.

Three capture scenes were added, for the comment card, a slider row and a colour row, so the
next visual comparison covers them. The workspace is at 186 tests and strict Clippy is clean.
The hands-on checklist in `docs/phase10-windows-acceptance.md` gained the items these faults
would have been caught by.

### Card layout and ports, from the second side-by-side (2026-09-20)

A screenshot of the two editors beside each other showed node cards that were too narrow and
carried ports Electron does not draw. Three rules were missing.

- **Cards did not grow.** `card_width` returned a constant per node kind. In the browser a
  node is an absolutely positioned element, so it shrink-wraps to its content and
  `--node-min-width` is only a floor; every longer label was being cut instead. The card is
  now measured against its header, port rows, parameter rows, output slots and footer, with
  the header's twelve pixel inset, or thirty-four when it carries the bypass tick. A 320
  pixel ceiling keeps one long value from stretching a card across the canvas.
- **Enum parameters appeared as rows and ports.** `nodeEditorHelpers.ts` filters
  `type !== 'enum'` when it builds `paramDefs`, so a list-valued parameter belongs to the
  inspector alone. That is why Electron's Premultiply Alpha and Convert Format cards carry
  nothing but their image ports.
- **The channel count did not trim the inputs.** A definition with a `channels` parameter
  shows only that many image inputs, taken from the node's value and falling back to the
  definition default, which is why Merge Channels has no alpha port at three.

Measuring text for the card width exposed a second fault. `controls::measure` went through
`GetWindowDrawList`, and card measurement happens before the canvas window begins; asking
for a draw list outside a window left the context in a state that drew a stray menu popup
over the library and clipped unrelated text. Measurement now goes straight to the font
through `Fonts::measure`, which needs no window.

The filmstrip status bar also hid its count when empty; Electron's `countLabel` shows
`0 images`, so the bar no longer goes blank.

A `cards` capture scene draws a column of the definitions that exercise each rule. The
workspace is at 193 tests.

### Font sizing and menu rows (2026-09-20)

Node card text was still noticeably smaller than Electron's, and the menu dropdowns had no
room between their rows.

**The em.** `ImFontConfig::SizePixels` is fed to `stbtt_ScaleForPixelHeight`, which scales a
font so that ascender minus descender equals that many pixels. A stylesheet's `font-size` is
the em size. The two differ by `(ascender - descender) / unitsPerEm`, which the bundled fonts
report as 1.32 for JetBrains Mono and 0.957 for Atkinson Hyperlegible Next. Passing the
token's number straight through therefore drew monospaced text at about three quarters of the
size the stylesheet asked for, while interface text came out four percent large. That is
exactly why the panels looked right and only the node cards, which are monospaced throughout,
looked small. Each face now reads `head` and `hhea` from its own file and rasterizes at
`size * (ascender - descender) / unitsPerEm`, rounded to a whole physical pixel so glyphs are
not resampled, with the logical size kept beside it for drawing and measuring. A test asserts
that JetBrains Mono advances six tenths of its token size, which it did not before.

**Menu rows.** A menu row's height is its text and nothing else, so with no vertical item
spacing the rows sat line against line. Dear ImGui grows a row's highlight into half the item
spacing on each side, so the five pixels `.dropdown li button` pads with are set as ten
pixels of spacing, and the dropdown carries the twelve by four padding `.dropdown` gives it.

**A dangling shortcut.** `Ui::menu_item` built the accelerator's `CString` inside the `if`
that chose between it and a null pointer, so it was dropped at the end of that block and Dear
ImGui was handed freed memory. Every accelerator was missing from every dropdown as a result.

Faces differ in line height, so two of them sharing a row can no longer both sit at a fixed
offset from its top. `controls::draw_in_row` centres each against the row instead, and the
Credits table uses it.

A `menu` capture scene holds a dropdown open by driving the pointer, so the row spacing and
the accelerators are visible in the capture set.

### Popup padding (2026-09-20)

Two follow-ons from the menu spacing work, both the same shape: a row in a Dear ImGui popup
is as tall as its text and nothing more, so a list built from plain rows arrives with its
options stacked line against line.

- A menu dropdown had no room above its first row or below its last. Dear ImGui places the
  first row's text at the window padding and only grows its highlight into half the item
  spacing above it, so the window padding now carries the row's five pixels as well as the
  dropdown's four.
- The inspector's dropdowns were still collapsed, because `controls::dropdown` pushed a zero
  item spacing that the open list inherited. The list now gives each row an explicit height
  of the label plus the five pixel padding `.dd-item` names, rather than leaning on item
  spacing, which also lets the list size itself correctly. A combo otherwise caps its list at
  eight of Dear ImGui's own rows, which is shorter than eight of these and cut the last option
  in half, so the height constraint is set from the caller's row height instead.

`Ui` gained `begin_combo`, `end_combo` and `set_next_window_size_constraints` so the list can
be styled on its own terms rather than the closed control's. A `dropdown` capture scene opens
the Compare node's operator list by driving the pointer, beside the `menu` scene.

### Wire-drop menu, category order and tooltip wrapping (2026-09-20)

- **The dropped wire vanished.** The line was drawn only while the connect gesture was live,
  so opening the creation menu erased it and left no sign of what the new node would attach
  to. The canvas now also draws it from the port to the drop point for as long as the menu
  holds a pending wire, which is what Electron does. The filtering by wire type was already
  correct; the missing line was what made it look otherwise.
- **Categories sorted by byte value.** `FX` came before `Filters` and `Format` because a
  capital `X` sorts below a lower-case `i`. Electron sorts with `localeCompare`, so the
  comparison is case-insensitive now, in both the creation menu and the library panel.
- **A flyout repeated its category on every row.** The flyout lists one category, so the name
  is drawn only in the flat search results, as in the Svelte menu.
- **The menu panel was a fixed height**, leaving an empty band below the last row. It is now
  measured from its rows and clamped by the ceiling and the room on screen.
- **Tooltips ran off in one long line.** The earlier fix passed a screen coordinate to
  `PushTextWrapPos`, which takes a window-local one, so the wrap position landed far beyond
  the window and nothing wrapped. Descriptions wrap at 320 pixels now.

Two capture scenes were added: `wire-menu`, which drops a wire from the seed Input's image
port, and `tooltip`, which rests the pointer on a library entry past the delay.

### Import timing, text metrics and the preview pipeline (2026-09-20)

- **The import dialog always reported `0.0 sec`.** Only a workflow run advanced the elapsed
  clock; an import never started one. The editor now stamps the moment an import begins and
  freezes the figure when it ends, and the completion line pluralizes the count and formats
  the duration as `ImportProgressModal.svelte` does, minutes included.
- **Text measured a few per cent wide.** Two causes, both in the atlas. `PixelSnapH` rounded
  every advance to a whole pixel, and the size a face was drawn at was the rounded pixel
  height rather than the exact one, which put an eleven pixel em out by almost five per cent.
  The atlas is still built at a whole pixel, because Dear ImGui truncates the size it is
  given, but each face now carries a `Scale` that brings it back to the exact height, and
  snapping is off. `PREVIEWING` measured 71 pixels where the stylesheet gives it 65.6; it
  measures 65.6 now. The badge was also drawn at a fixed size while the cards around it
  zoomed, so its chrome, its type and its letter spacing all follow the zoom.
- **The preview only worked with a connected graph.** Electron picks a preview target on its
  own when the user has chosen none: the node feeding an output, or the end of the chain,
  walking back past bypassed nodes. That is implemented, and when a workflow has no
  processing node at all the Input stands in, so the selected image appears on its own. A
  double click still sets an explicit choice.
- **Switching images felt slow.** The native preview rendered the original file through the
  chain every time. `preview-pipeline.ts` runs it over the cached import thumbnail instead
  and resolves parameters against the original's metadata, so `preview::render_from` takes
  the two files separately and the editor passes the thumbnail. The overlay reports the
  original's measurements, which travel beside the rendered image, and the thumbnail is put
  in the panel as a stand-in until the first render lands.

A `previewing` capture scene wires a processing node between the seed's Input and Image
Output, so the badge is in the parity set.

### The preview worker (2026-09-20)

Rendering over the thumbnail was not enough: every switch still felt slow, because each one
spawned a thread that built a new ImageMagick session and a new thumbnail cache and threw
both away. The session caches what it has measured, so a fresh one meant paying for the same
two `identify` processes on every visit to an image.

- **One long lived worker** now does every preview, keeping its session and its thumbnail
  cache between them. Requests queue, and only the newest is rendered: the ones behind it are
  for images the filmstrip has already moved off.
- **A rendered preview is kept**, keyed by the file, the node it renders up to, and the shape
  and parameters of the graph, so going back to an image costs nothing. Positions are not
  part of the key, as they are not part of the key `Preview.svelte` re-renders on. The last
  twenty four are held.
- **Nothing to process, nothing to run.** With no chain, Electron's pipeline returns the
  downscaled file itself. The panel now shows the thumbnail the filmstrip already holds, on
  the interface thread, with no ImageMagick at all: switching is as immediate as the pointer.

### Import and metadata costs (2026-09-20)

Re-importing a folder already in the cache took a second where Electron takes none, and
every preview opened the source file twice over. Four costs, all of them process spawns:

- **Every cached thumbnail went back through ImageMagick.** The cache stores WebP, which is
  a tenth the size of the same picture as a portable network graphic, and the browser decodes
  those for nothing. Here each one was converted to a PNG in a process of its own before it
  could be decoded: twenty six images, twenty six spawns. `image-webp` decodes them in
  process instead. A folder of twenty six re-imports in 0.03 seconds where it took 1.1.
- **Dependencies were unoptimized in a development build.** The same folder took 0.41 seconds
  with a debug decoder against 0.03 with a release one. `profile.dev.package."*"` now builds
  third party code with optimizations while our own crates stay as they were, which brings a
  development build to 0.11.
- **The preview always asked for the full inspection of the file**, which is two more
  `identify` processes on every switch of the filmstrip. Definitions already record which
  metadata keys they reference, and the batch executor already used that to decide; the
  preview now shares the same predicate, so the file is only opened when a node asks about
  its bit depth, its resolution or its EXIF block. A format whose header cannot be read
  still gets the full treatment, so nothing resolves against a measurement of zero.
- **A format the header parser does not read cost an `identify` of its own.** That
  measurement now rides along with the thumbnail command, as `thumbnail-service.ts` does,
  so a folder of TIFFs is one process per batch rather than two per image.

The header parser also names the format from the signature now, so a `.jpg` reads as `JPEG`
rather than `JPG`, matching both `identify` and Electron's own header path.

`cargo run -p bite-gui --example import_bench -- <folder>` times a cold and a warm
import of a folder, which is how the figures above were taken.

### What the Electron pipeline optimizes, and where the native one stands (2026-09-20)

A pass over `src/main/pipeline` to make sure nothing the Electron build learned was lost in
the port. Taken:

- **A share of the machine per pipeline.** A batch ran at most eight images at once, each
  pinned to one ImageMagick thread, which left most of a sixteen core machine idle.
  `batch-pipeline.ts` runs as many pipelines as there are images and divides the hardware
  threads between them, so the total stays near one thread per core whether it is running
  two images or two hundred. `emit_many` now does the same, and the default job count is one
  per core rather than half of them capped at eight.
- **The whole set of files read at once.** The import read each file's header one after
  another; `thumbnail-service.ts` reads them all together. Both the header probe and the
  thumbnail decoding now run across the job threads. A warm folder of twenty six went from
  0.11 seconds to 0.02 in a development build.

Considered and not taken, with the reason:

- **The node level preview cache.** `preview-pipeline.ts` writes a PNG between every pair of
  nodes and caches each one against a hash of its input and parameters, so editing the last
  node of a chain re-runs only that node. It needs that because it spawns one ImageMagick
  process per node. The native executor composes the whole chain into a single command and
  runs it once, with no files in between, so there is nothing between nodes to cache and one
  process to pay for rather than six. The cache that does apply here, of the finished
  preview against the file and the graph, is the one the preview worker keeps.
- **The eighty millisecond preview debounce.** The editor coalesces requests to one per
  frame and the worker drops all but the newest, so a parameter drag costs one render in
  flight and one queued, however fast it is dragged. The debounce exists to avoid rendering
  during a drag at all; the native editor renders through it, which is what makes the
  preview follow a slider.

Still open: the Debug menu's Performance Timers toggle sets a flag that nothing reads.
Electron's `TimingCollector` is what the figures above would be measured with.

### The three Debug windows (2026-09-20)

- **Performance Timers** now has something behind it. An import reports its total, its time
  per image and how many of its thumbnails were already cached; a preview reports its time,
  the size of its chain and whether the worker reused a result; a run reports what it
  processed, skipped and failed. Each also reports the number of ImageMagick processes it
  took, which is the number worth watching here: Electron spawns one per image and one per
  node, so it times each of those, while the native pipeline composes a chain into a single
  command. The flag is global rather than a field on the editor, because the work that
  reports is on background threads. Everything goes to the session log, where the log window
  shows it.
- **View Log** opens a window in the editor rather than handing the file to the platform.
  It has the level filters, the timestamp, badge and message columns, the `[tag]` highlight
  in `#02ccff` and the following of the newest line that `public/log-viewer.html` has. It
  reads the log file rather than the ring buffer, so earlier sessions are in it, and it
  re-reads only what has been appended since it last looked. Clear hides what is there, as
  the Electron viewer's does, rather than emptying the file; an Open File button still hands
  it to the platform.
- **Show All UI Elements** draws the type scale, the palette, the port colours and every
  control in one window, seeded with the values `Showcase.svelte` opens with. The dialogs are
  not redrawn there: its buttons open the real ones, which is the only way to be sure they
  still match.

Both windows wear the editor's own chrome through `controls::debug_window`, because Dear
ImGui's default title bar is blue and reads as a different program. `StyleColor` gained the
title bar entries and `Ui` gained `window_closable` and `scroll_max_y` for them. Two capture
scenes, `log-window` and `showcase`, cover the result.

### Eight reported defects (2026-09-20)

- **The first category of the creation menu was always lit.** The keyboard highlight started
  at row zero; `NodeContextMenu.svelte` starts it at minus one, so nothing is lit until an
  arrow key is pressed. It is an `Option` now, and Enter with nothing highlighted takes a
  lone search result, as the Svelte menu does.
- **The filmstrip had a vertical scrollbar.** A thumbnail was sized by taking Electron's
  fifty two pixels of chrome off the panel, which does not cover the horizontal scrollbar
  Dear ImGui lays out inside the strip. The chrome is now added up from its parts, that
  scrollbar included, so an item is exactly as tall as the strip. A thumbnail is smaller at
  a given panel height than Electron's by the width of that scrollbar.
- **Scrollbars were too thin.** Dear ImGui insets its grab inside the track by
  `trunc((width - 2) / 2)` on each side, capped at three, so the bar that is seen is the
  track less six for any track from eight pixels up. The stylesheet's five drew a three
  pixel bar; widening the track to nine drew exactly the same three, which is why the first
  attempt changed nothing. The track is the bar plus six now, for a six pixel bar, and the
  arithmetic is in a test so it is not tuned by eye again.
- **A Folder Path node had no folder picker.** Its inspector had one all along; the node
  never reached it. Every node created from a definition was made an ordinary processing
  node, so a Folder Path was not a `folderPathNode` and fell through to the generic
  parameter editor. `Studio::node_kind` now gives the three definitions that have cards of
  their own the kinds they are saved as, which also restores the Process As Set and Compare
  inspectors and their cards.
- **The wordmark is gone from the menu bar**, which now starts at File.
- **Dragging the window stuttered.** Every move event asked for a frame, so a present that
  waits for the vertical blank sat inside the drag loop. Moving the window needs no new
  frame; the compositor carries the one already presented.
- **Text in a field sat low.** The frame padding was guessed from the token's pixel size,
  but Dear ImGui sizes a frame as the current font's line height plus twice its padding, and
  a line height is the em times the ascender-to-descender distance, not the em. It is
  measured from the face now, so a field is exactly its token height with the text centred.
- **The library's plus and minus were small and off centre.** `.collapse-icon` is twelve
  pixel mono centred in a ten pixel box, with the panel gap after it, and the row centres
  both the glyph and the label. All of that was eyeballed offsets; it is measured now.

A `folder-path` capture scene covers the card and the inspector that were wrong.

### Two text roles in the inspector (2026-09-20)

A checkbox row's wording was drawn in the label colour, so `Generate .log file` read as
another section title rather than as the thing its row says. The stylesheet gives
`.log-toggle` and `.checkbox-label` the bright text at full strength, with no opacity;
only `.section-title` and `.param-label` take it at six tenths.

The two roles are named now, `INSPECTOR_LABEL` and `INSPECTOR_VALUE`, and every label in the
inspectors goes through one of them rather than repeating the literal. A test asserts they
differ, which is the class of mistake rather than the one instance.

Checked across the inspectors by sampling a capture: section titles come out around 150 and
field contents, dropdown choices and checkbox wording around 245, against Electron's 161 and
255 for the same panel. An `output-inspector` capture scene holds the panel that has a
section title, a text field, two lists and a checkbox row all at once.

### Mixing with transparent, the colour picker, and the inspectors it turned up (2026-09-20)

**`color-mix` with `transparent` was mixing the wrong thing.** CSS multiplies each colour by
its own alpha before interpolating and divides the result back out, so
`color-mix(in srgb, white 30%, transparent)` is white at three tenths. `Color::mix` blended
the channels straight, which dragged them towards transparent's black and gave a dark grey
at three tenths instead. Over the disabled button's background that drew 43 where the
browser drew 96 — the Run Workflow button, and about twenty other places that all mix with
`transparent`: every primary and danger border, the inspector's row separators, the
filmstrip's rule, the slider track, the run dialog's tinted rows and the node cards' bypass
tick, whose amber wash had been all but invisible. One fix in `bite-imgui` corrects them
all; a test pins the white case and the ninety-six it draws.

**The colour picker was Dear ImGui's, not the editor's.** `ColorPicker.svelte` is a
saturation square, a hue bar, a mode drop-down, a ramped slider and number box per channel,
an alpha row and a hex row, laid out inline in the inspector; ours was a swatch that opened
ImGui's built-in wheel. `color_picker.rs` draws the Svelte one, with the conversions ported
from `colorConversions.ts` so RGB 0-1, RGB 0-255, HSV, LAB and CMYK read the same numbers.

Two things the port needed that the draw list does not give:

- Gradient fills take no corner radius, so a ramp drawn inside a rounded box spills into the
  corners. Each corner is painted back to the surface behind it with a fan of triangles
  between the square corner and the arc the fill should have followed.
- The hue and saturation are kept beside the colour rather than derived from it each frame.
  Black and white have no hue of their own, so dragging the square to the bottom edge and
  back would otherwise come up red. The Svelte component holds them the same way, and a test
  covers it.

**The Tint node's red default was drawn as black.** Electron stores a `color-picker`
parameter as a hex string and a `vector`-widget colour as four numbers; we read both as
vectors, so `#ff0000` came back as transparent black, and editing one wrote a vector the
pipeline would not have understood. The two shapes have separate arms now.

Auditing the rest of `Inspector*.svelte` against ours, by listing every string each component
can put on screen and checking it exists on this side:

- **Resize had no inspector of its own.** It ran the generic parameter editor, which gave it
  the definition's raw option names, sliders where the component has number boxes, no `%`
  or `px` unit, no disabled computed dimension and no Resize Preview. It is written out now:
  the aspect ratio drives whichever dimension the anchor does not, and the preview says what
  the selected image would come out as. The arithmetic is in `resize_preview` with tests
  rather than in the drawing code.
- **A rename block could not be removed.** The row had no delete control at all. It has the
  cross now, along with the six-dot grab handle, the filled `.block-badge` tags, the compact
  `.field-input` fields the blocks use rather than the thirty-pixel ones, the `start`/`pad`
  sub-labels with the zero-padded preview beside them, and the subtle mono add bar.
- The rename preview gained its `ORIGINAL` / `NEW NAME` column heads, counts files rather
  than `file(s)`, and puts an unchanged name in dim text rather than a faded accent.
- The text preview has two empty states, not one: with images loaded, nothing to show means
  no port is wired.
- The comment body carries its `Notes...` placeholder, drawn where the first line starts
  because a multiline field takes no hint.
- The `...` browse buttons and the two delete controls carry the tooltips their `title`
  attributes give them, using Dear ImGui's own hover delay rather than another timer.

Not carried over: the Input folder button's `Choosing...` label. The native folder dialog
blocks, so no frame is drawn while it is open and the state cannot be seen.

`resize-inspector` and `rename-inspector` capture scenes were added, bringing the set to
twenty-four.

## Correction 2026-09-21 — the badge, and the measurement behind it

The previewing badge sat too high, and its letters were spaced too far apart. Three causes,
of which the third is the one that mattered beyond the badge.

- **Only the regular interface face exists.** `fonts.css` declares one `@font-face` for
  `--font-ui`, Atkinson Hyperlegible Next Regular, so `font-weight: 700` on the badge is a
  browser-synthesized bold: the outlines thicken and the advances stay regular, which is why
  the stylesheet gives `PREVIEWING` 65.6 pixels. Drawing it in the real bold face ran 69.3
  instead, and the extra three and a half pixels read as letter spacing. `face::BADGE` is the
  regular face now and the weight is drawn on, twice a twenty-fourth of an em apart, which is
  Skia's own fake bold.
- **`top: -22px` is a top edge, not a centre.** The badge was centred on that point, so it
  floated half its height above where the stylesheet puts it, and its box was built from Dear
  ImGui's line box rather than `line-height: normal` - 13.17 pixels here, since the face sets
  `USE_TYPO_METRICS` and a browser then lays out on the `OS/2` typographic metrics.
- **Measurement did not describe what was drawn.** `Fonts::measure` rounded the size it asked
  about, on the belief recorded above that Dear ImGui draws at a rounded size. It does not:
  `GetFontBaked` rounds only the size a glyph is *rasterized* at, and `CalcTextSizeA` and
  `RenderText` both scale their output by `size / baked->Size`, so the draw path was already
  exact. The earlier note that `PREVIEWING` "measures 65.6 now" was measuring the rounded bake;
  it measured 68.6. Nothing about the atlas needed changing - the rounding came out of
  `measure` and the line went with it.

That last one was never only the badge. Measurement feeds every box drawn around a run, every
centred and right-aligned label, every ellipsis decision, and every glyph position in
letter-spaced text, and the error was not a constant: the interface face came out 4.5 per cent
wide at ten and eleven pixels and 4.2 per cent narrow at twelve, so `--font-size-xs` and
`--font-size-sm` measured the same while drawing a size apart. It was worst under zoom, where
the eleven pixel face rounded up at 100 per cent and down at 200, so a card's type changed
proportion as it was zoomed.

`crates/bite-imgui/tests/font_metrics.rs` holds the invariants: twelve pixel mono advances
exactly 0.6 em per character, the badge string measures the stylesheet's 65.6, measuring at a
scale is proportional to it, and each step of the type scale measures wider than the one below.
All four fail against the rounded measurement.

No atlas cost came with it. Bakes are still keyed on the rounded rasterizer size, so the set of
rasterized sizes is exactly what it was.

## macOS 2026-09-21 — the platform the migration had left stubbed

The editor shipped on Windows and did not compile on macOS. `render/mod.rs` gated the D3D11
device, swapchain and readback on `cfg(windows)` while `platform.rs` and `smoke.rs` imported
them unconditionally, which is the state the sokol migration deliberately stopped at: its
Phase 8 could not be written, let alone run, on the Windows machine that work was done on.

It builds, runs, captures and packages now. `docs/sokol-migration-plan.md` records what the
renderer half cost and where it diverged from what that plan predicted — a `Bgra8` swapchain
that reaches into the capture goldens, a blit readback because sokol's attachments are private
storage, and the objc2 0.5/0.2 family rather than the newer one. What follows is the rest.

**What a Mac needed that no `cfg` had stood in for.**

- **The menu bar, because of Cmd+Q.** winit installs a default macOS menu whose Quit is AppKit's
  `terminate:`, which ends the process where it stands: no unsaved-changes prompt, no
  `save_session`, so the window bounds and panel sizes of that session are gone. The editor
  suppresses that menu (`with_default_menu(false)`) and builds its own, in which Quit is an
  ordinary item carrying Cmd+Q and routed through the event loop to `Command::Exit` — the same
  path the File menu's Exit and the red close button take. Everything else in it is a muda
  predefined item, mapping onto AppKit's own selectors.

  It carries exactly **one** accelerator, and that is the design rather than an omission. A muda
  accelerator intercepts the key before winit sees it and sends it down a responder chain winit's
  view does not forward to Dear ImGui, so a File menu with Cmd+N or an Edit menu with Cmd+C would
  have produced shortcuts that look bound and do nothing. ImGui already implements those chords,
  and with Cmd rather than Ctrl, because sokol_imgui sets `ConfigMacOSXBehaviors`.

- **Finder opens, which are not arguments.** On Windows a double-clicked workflow is `argv[1]`.
  On macOS Launch Services delivers it as an Apple event, so a fresh launch gets no arguments and
  a running editor gets no new process. `openfiles.rs` adds `application:openURLs:` to winit's
  delegate class at run time — the only one of the three available approaches that leaves winit's
  own dispatch intact — and re-sets the delegate so AppKit re-reads which methods it has. A
  launch-by-open arrives before `resumed`, so those are queued and handed to the window as it is
  created rather than opening blank and loading a frame later.

- **The bundle layout.** `definitions_root` probed only beside the executable; a bundle keeps its
  payload in `Contents/Resources`, and a bundle launched from Finder has working directory `/`,
  so without the `../Resources` probe the fallback pointed at a source tree that is not on the
  user's machine. `Magick::discover` gained the matching candidate.

- **`timing.rs` reported NaN off Windows**, which the sokol plan noted and left. It has real arms
  now: `proc_pidinfo(PROC_PIDTBSDINFO)` for the kernel's process-creation time, which is the true
  equivalent of `GetProcessTimes` because it includes dyld, and `proc_pid_rusage` for resident
  size and physical footprint. Note that `rusage_info_t` is a `void *` typedef and the parameter
  is declared `rusage_info_t *`, but the pointer passed is the destination, not a pointer to one;
  passing the address of a `rusage_info_t` variable type-checks and smashes the stack. The test
  that insists the counters read non-zero is what caught it.

**Packaging.** There was none on this branch — the Electron-era `scripts/build-mac.sh` went with
the rest of that tree. `packaging/build-mac-dmg.sh` and `packaging/build-mac-release.sh` differ
only in whether the notary service is involved, and share `packaging/mac-common.sh`. The two
inputs that are not in git are unchanged and still correct: `scripts/bundle-magick-mac.sh` and
`scripts/build-icon-mac.sh`.

Three things in there are worth keeping rather than rediscovering:

- **The `.dmg` needs its own signature, ticket and staple.** The one on the `.app` is what lets it
  launch offline after a drag-install; the one on the `.dmg` is what the download itself is
  checked against. So the app is notarized and stapled *before* the image is built around it —
  stapling rewrites the bundle, and an image built first would carry the unstapled copy. This is
  the same lesson the Electron script had learned, where electron-builder notarized the app and
  left the container it shipped unsigned.

- **`magick --version` is not a smoke test.** It prints happily from a tree whose coder modules
  cannot be loaded, which is a bundle that ships and then fails on the first image opened.
  Homebrew builds ImageMagick with loadable coders, so the script encodes a real PNG under
  `env -i` with exactly the four `MAGICK_*` variables `Magick::discover` sets, from inside the
  signed bundle — which is also where library validation would reject a mis-signed module.

- **`set -o pipefail` and `grep -q` do not mix.** `codesign -dvv | grep -q` reports *failure* on
  a match: grep leaves as soon as it has one, codesign takes a SIGPIPE writing the rest, and
  pipefail surfaces that. It read as "the bundled ImageMagick is not hardened" about a tree that
  was. Read once into a variable and match against that.

**One bug this found in shared code.** `save_session` ran twice on exit, so the log said "Editor
closed" twice and the session was written twice. `event_loop.exit()` is a request rather than a
stop: winit still delivers what it has already queued, so the redraw in flight when the editor
decided to leave arrives afterwards and finds `should_exit` still set. The second write happened
while the window was going away, which is not a state worth reading the bounds in. It is guarded
now, and the guard covers all four exit paths on both platforms.

**Verified here, automated.** `cargo test --workspace` and `cargo clippy --workspace
--all-targets` clean. `tests/render_stack.rs` runs on Metal — device, sokol_gfx, sokol_imgui, an
offscreen pass and the blit readback, with the channel order checked. All 25 `--capture` scenes
render, from the source tree and from inside the bundle with `env -i` and working directory `/`.
The packaged CLI runs a real resize workflow end to end with no Homebrew and nothing on `PATH`,
64x64 in and 32x32 out, which is `Magick::discover` and the relocated coder modules together.
The bundled editor reaches its first frame in 163 ms. The system menu bar reads `Bite` and
`Window` with the expected items; Cmd+Q on a clean document quits and writes the session once;
Cmd+Q on a dirty one raises "You have unsaved changes. Exit anyway?"; a double-clicked workflow
opens both on launch and into a running instance, and the running instance asks before replacing
a dirty document. `packaging/build-mac-dmg.sh` produces a signed, verified 20 MB image.

**Not verified here, and needing hands on a Mac.** Everything that wants a real pointer:
AppleScript's synthetic clicks do not reach a Dear ImGui surface, so no mouse interaction was
exercised. Specifically still open — dragging the window between a Retina and a 1x display
(`set_scale_factor` is written and unexercised), live resize and minimise/restore, full screen,
the file dialogs and clipboard, drag-and-drop, and a clean-machine install from a notarized
image. `packaging/build-mac-release.sh` has not been run: it needs the notary service, so it is
the user's to run first.

**The Electron tree went with it.** The branch carried no tracked Electron sources any more, but
1.2 GB of its build output was still in the working tree — `node_modules`, `dist-cli`,
`dist-electron`, `dist-web`, the Vite bundle left inside `dist/`, and `release/0.5.0` with the
last signed 0.5.0 macOS installer. All removed. That installer was never published (GitHub
releases stop at v0.4.4, Windows-only, under the old `imgplex` name) and cannot be rebuilt now,
which is why it was a decision rather than a tidy-up; the user made it.

Two live dependencies went too. `scripts/build-icon-mac.sh` validated the compiled `.icns` by
walking its chunk table in `node -e`, which put a JavaScript runtime on the macOS release path;
it now converts the icns back to an iconset with `iconutil` — already required three lines above —
and checks for `icon_512x512@2x.png`, which is the same `ic10` entry by another name. A tracked
`imgui.ini` was also deleted: the editor sets `IniFilename` to null and keeps its layout in
`persist::Session`, so nothing had written that file for some time.

What stayed is the parity record. Forty-five comments across twenty-eight files still name
Electron, and they are the reason each module is shaped as it is — `README.md` keeps
`docs/spec.md` as the behavioural contract precisely so those can be checked against something.
None of them points at a path that no longer exists; the one that did, `menu.rs`'s reference to
`electron/main.ts`, was reworded.

**Known and deliberate.** Full screen is still F11, which is a Windows binding — macOS expects
Cmd+Ctrl+F, and changing it touches the key map rather than only a label, so it was left. The
`.bite` association claims `LSHandlerRank Owner`, since the editor defines the type. The CLI ships
at `Contents/MacOS/bite` with no PATH entry, where the Windows installer offers one.
