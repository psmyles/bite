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
  fixtures pass. Atomic output replacement and Ctrl+C cancellation implemented.
  M2 is NOT complete: concrete planning now shares execution and records analysis dependencies; broader planner goldens and
  broader multi-input, failure and migration compatibility coverage remains.
- Phase 7: header readers, operation fusion, per-process thread limits and
  progress exist. Caches, configurable bounded workers and benchmark gate pending.
- Phase 8: pinned C++ submodules, owned C ABI/bindings, safe Rust wrapper and
  Winit/WGPU prototype compile on Windows. Offscreen rendering of 120 nodes,
  docking, inspector and uploaded preview/filmstrip textures was visually
  verified on an RTX 4080. Interaction/DPI/IME acceptance remains pending; see
  `docs/native-gui-prototype.md` for commands and the exact remaining checks.
- Phases 9–13: pending; Electron remains intact.
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

A task heartbeat named `Continue BITE Rust migration` resumes work every 15
minutes when eligible, from this log. Continue substantive implementation; do
not treat this checkpoint as the completion of the migration.

1. Finish command-planner coverage: `bite plan` now emits concrete per-image
   arguments and native output operations without spawning magick or writing.
   Fast-path commands match the original golden. Expand comparisons across all
   native structures and formats, validate supplied analysis facts, and represent
   downstream work after unresolved analysis (currently planning pauses there).
2. Broaden CLI compatibility tests beyond the eleven fixtures: multiple input
   branches sharing downstream processing, bypass behavior, all v1 shims,
   parameter-wire coercion/defaults and failure/cancellation behavior. Inspect
   existing TypeScript before choosing behavior; document deliberate changes.
   Separate input branches, partial sets, numeric ascending/descending atlas
   ordering and rename collisions now pass five live Node/Rust differential
   cases in `test-workflows/compat-edgecases.mjs` (`npm run test:compat`).
   Locale-sensitive non-ASCII atlas ordering is still unverified.
3. After M2, implement the remaining performance services and benchmark Node
   versus Rust on deterministic fixtures and `test_images`.
4. Complete the independent prototype's interaction/platform glue and Windows
   acceptance gate before the full GUI. Keep macOS explicitly pending.
5. Continue phases 9–13 in plan order. No cutover, release or legacy removal has
   occurred. Work is on branch `rust-migration`; changes are not yet committed.

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
