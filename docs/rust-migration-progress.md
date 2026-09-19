# Rust migration progress

## Execution policy

The 2026-09-19 user decision permits progression after Windows checks pass;
macOS checks remain pending and do not constitute verified parity. Use
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
- macOS automated, GUI, packaging, and clean-machine checks: pending.
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
