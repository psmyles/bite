# imgplex Pipeline Test-Workflow Suite

A set of 11 self-verifying workflows that together exercise every execution path of the
processing pipeline. Each workflow is built once in the GUI (specs below), saved into this
folder, and can then be run from **both** the GUI (Run Workflow button) and the **CLI**
(`imgplex-cli run`). `run-tests.ps1` generates deterministic fixture images, runs every
workflow through the CLI, and asserts the outputs.

## How to run

```powershell
# Full suite: generate fixtures, run all workflows via CLI, assert outputs
.\test-workflows\run-tests.ps1

# Run/assert a single workflow
.\test-workflows\run-tests.ps1 -Only wf-04

# After running a workflow manually from the GUI: skip execution, only assert outputs
.\test-workflows\run-tests.ps1 -Only wf-04 -AssertOnly
```

Requirements: `magick` on PATH, and the CLI bundle (`npm run build` produces
`dist-cli/cli-bundle.js` + `.exe`; the runner prefers `node dist-cli\cli-bundle.js`).

**GUI runs:** load the workflow's fixture folder (see each spec) into the Input node's
filmstrip, then press Run Workflow. Set all output paths to the absolute equivalents of the
paths listed per workflow (e.g. `D:\Dev\imgplex\test-workflows\out\wf-01`) so
`-AssertOnly` can verify a GUI run. The image lists are **not** stored in `.imgplex` files —
they must be re-loaded per session.

## Conventions (apply to every workflow)

| Node | Setting |
|---|---|
| Input node | CLI Name = `in` |
| Image Output node | CLI Name = `out` (WF-08 uses `out-<format>`), Output Path = Custom, Custom Path = `<repo>\test-workflows\out\<wf-name>` (absolute), Overwrite = **overwrite**, Generate Log = off |
| Text Output node | CLI Name = `report`, Output Path = `<repo>\test-workflows\out\<wf-name>\report.txt`, Separator = **comma**, Overwrite = **overwrite** |
| Flipbook Output node | CLI Name = `atlas`, Output Path = `<repo>\test-workflows\out\<wf-name>\atlas.png` |

Save each workflow as `test-workflows\wf-NN-<name>.imgplex` (exact filenames below — the
runner looks them up by name and reports missing ones as SKIP).

Wiring notation used below: `nodeLabel.handle → nodeLabel.handle`. Image handles are
`in-0`/`out-0`…; value wires are `param-out-<name>` → `param-in-<name>`; Text Output value
ports are `txo-0`, `txo-1`, … in the order you connect them.

## Fixture folders (created by the runner)

- `fixtures\main` — `alpha_grad_128.png` (white, linear alpha gradient, 128²),
  `blue_256.png`, `checker_100x80.png` (10 px black/white checker, mean exactly 0.5),
  `gray50_256.png` (solid gray(128)), `green_256.png`, `red_256.png` (solid 256²)
- `fixtures\meta` — `deep16_64.png` (16-bit gradient), `photo_300dpi.jpg` (300 DPI, EXIF Make=TestCam best-effort)
- `fixtures\sets` — `set_alpha_diffuse.png` gray(200), `set_alpha_normal.png` gray(100),
  `set_alpha_rough.png` gray(50), `set_beta_diffuse.png` gray(30), `set_beta_normal.png` gray(60),
  `set_beta_rough.png` gray(90) — all 64²
- `fixtures\flip` — `fb_a_red.png`, `fb_b_green.png`, `fb_c_blue.png` (solid 64²)

---

## WF-01 — Fast-path chain — `wf-01-fastpath.imgplex`

**Covers:** batch Path A (no meta / no multi-stream → op plan built once, single magick
spawn per image), `resize` executor, `command_template` nodes.

**Fixtures:** `fixtures\main`

**Graph:**

```
Input ─ out-0 → in-0 ─ Resize ─ out-0 → in-0 ─ Flip ─ out-0 → in-0 ─ Brightness/Contrast ─ out-0 → in-0 ─ Blur ─ out-0 → in-0 ─ Image Output
```

| Node | Params |
|---|---|
| Resize | mode `absolute`, width `128`, height `128`, **preserve_aspect off**, filter `Lanczos` |
| Flip | axis `horizontal` |
| Brightness/Contrast | brightness `10`, contrast `10` |
| Blur | sigma `1` |

**CLI:** `imgplex-cli run wf-01-fastpath.imgplex --in fixtures\main --out out\wf-01`

**Expected:** 6 output PNGs with the original filenames, all exactly **128×128**.

> Known quirk found during spec design: the `flip` node's command template is hardcoded to
> `-flop`; the *axis* dropdown is never read, so "vertical" also mirrors horizontally.
> Use `horizontal` here. (Candidate app bug, not a workflow problem.)

---

## WF-02 — Light-meta properties → text report — `wf-02-props-light.imgplex`

**Covers:** batch Path B trigger with **light** meta only (`buildEmptyImageMeta` — header
reads, no magick spawn), `textOutputNode` → `executeTextBatch`, multiple `txo-*` ports.

**Fixtures:** `fixtures\main`

**Graph:** `Input.out-0 → Text Output.in-0`, plus value wires in this port order:

| Port | Source |
|---|---|
| txo-0 | Name (`prop_name`, strip_extension off) `param-out-value` |
| txo-1 | File Type (`prop_filetype`) `param-out-value` |
| txo-2 | Size (`prop_size`, unit `KB`) `param-out-value` |
| txo-3 | Dimensions (`prop_dimensions`) `param-out-width` |
| txo-4 | Dimensions (same node) `param-out-height` |
| txo-5 | Power of Two (`prop_power_of_two`) `param-out-result` |

**CLI:** `imgplex-cli run wf-02-props-light.imgplex --in fixtures\main --report out\wf-02\report.txt`

**Expected `report.txt`** (6 lines, alphabetical by filename):

```
alpha_grad_128.png,png,<size>,128,128,true
blue_256.png,png,<size>,256,256,true
checker_100x80.png,png,<size>,100,80,false
gray50_256.png,png,<size>,256,256,true
green_256.png,png,<size>,256,256,true
red_256.png,png,<size>,256,256,true
```

---

## WF-03 — Heavy-meta properties — `wf-03-props-heavy.imgplex`

**Covers:** heavy meta (`loadImageMeta` magick spawn per image): bit depth, DPI, EXIF.

**Fixtures:** `fixtures\meta`

**Graph:** same shape as WF-02. Ports:

| Port | Source |
|---|---|
| txo-0 | Name `param-out-value` |
| txo-1 | Bit Depth (`prop_bitdepth`) `param-out-value` |
| txo-2 | Resolution (`prop_resolution`) `param-out-dpi_x` |
| txo-3 | EXIF (`prop_exif`) `param-out-camera_make` |

**CLI:** `imgplex-cli run wf-03-props-heavy.imgplex --in fixtures\meta --report out\wf-03\report.txt`

**Expected:** `deep16_64.png` line contains `,16,`; `photo_300dpi.jpg` line contains
`,8,300` and (soft — depends on ImageMagick EXIF write support) ends with `TestCam`.

---

## WF-04 — Channel split → transform → merge — `wf-04-channels.imgplex`

**Covers:** batch Path C (multi-stream): visual channel split, lazy command fusion
(`negate` chained onto a split channel), **param-wired constant channel** inlined into
`channel_merge`, single-spawn merge.

**Fixtures:** `fixtures\main`

**Graph:**

```
Input.out-0            → Split Channels.in-0
Split Channels.out-0 (R) → Merge Channels.in-0 (R)
Split Channels.out-1 (G) → Negate.in-0 ; Negate.out-0 → Merge Channels.in-1 (G)
Float.param-out-value    → Merge Channels.in-2 (B)      ← value wire into an image port
Merge Channels.out-0     → Image Output.in-0
```

| Node | Params |
|---|---|
| Float (`value_float`) | value `0.5` |
| Merge Channels | channels `3` (alpha port left unconnected) |

**CLI:** `imgplex-cli run wf-04-channels.imgplex --in fixtures\main --out out\wf-04`

**Expected** (channel means, 0–1, ±0.02): `red_256.png` → (1, 1, 0.5) · `green_256.png` →
(0, 0, 0.5) · `blue_256.png` → (0, 1, 0.5). All 6 inputs produce an output.

---

## WF-05 — Analysis-only means + logic gate → report + images — `wf-05-meanlogic.imgplex`

**Covers:** analysis-only channel split (all split consumers are `mean_value` → no channel
files written, `loadMultipleChannelMeans`), `logic_comparison`/`logic_and`, gate suppression
driven by computed values, **two output nodes in one run** (text + image).

**Fixtures:** `fixtures\main`

**Graph:**

```
Input.out-0 → Split Channels.in-0
Split Channels.out-0 (R) → Mean R.in-0        (mean_value)
Split Channels.out-1 (G) → Mean G.in-0        (mean_value)
Split Channels.out-2 (B) → Mean B.in-0        (mean_value)
Mean R.param-out-value   → Compare R.param-in-a   (operator: greater than)
Float 0.9.param-out-value→ Compare R.param-in-b
Mean G.param-out-value   → Compare G.param-in-a   (operator: less than)
Float 0.1.param-out-value→ Compare G.param-in-b
Compare R.param-out-result → And.param-in-a
Compare G.param-out-result → And.param-in-b
And.param-out-result     → Gate.param-in-condition
Input.out-0              → Gate.in-0
Gate.out-0               → Image Output.in-0

Input.out-0              → Text Output.in-0
txo-0 ← Name.param-out-value
txo-1 ← Mean R.param-out-value
txo-2 ← Mean G.param-out-value
txo-3 ← Mean B.param-out-value
txo-4 ← And.param-out-result
```

**CLI:** `imgplex-cli run wf-05-meanlogic.imgplex --in fixtures\main --out out\wf-05\images --report out\wf-05\report.txt`

**Expected report** (exact):

```
alpha_grad_128.png,1,1,1,false
blue_256.png,0,0,1,false
checker_100x80.png,0.5,0.5,0.5,false
gray50_256.png,0.502,0.502,0.502,false
green_256.png,0,1,0,false
red_256.png,1,0,0,true
```

**Expected images:** exactly one file in `out\wf-05\images` — `red_256.png` (the only image
where R-mean > 0.9 AND G-mean < 0.1).

---

## WF-06 — Set-mode texture packing — `wf-06-setmode.imgplex`

**Covers:** batch Path D (`process_as_set` → `groupBySetPattern`, one multi-stream run per
set), per-suffix buffer seeding, set output naming (`setOutputPrefix`/`setOutputSuffix`),
a lazy op (`negate`) inside a set run.

**Fixtures:** `fixtures\sets`

**Graph:**

```
Input.out-0 → Process As Set.in-0
Process As Set.out-0 (_diffuse) → Merge Channels.in-0 (R)
Process As Set.out-1 (_normal)  → Merge Channels.in-1 (G)
Process As Set.out-2 (_rough)   → Negate.in-0 ; Negate.out-0 → Merge Channels.in-2 (B)
Merge Channels.out-0 → Image Output.in-0
```

| Node | Params |
|---|---|
| Process As Set | prefix `set_`, suffixes `_diffuse`, `_normal`, `_rough` (in that order) |
| Merge Channels | channels `3` |
| Image Output | set output prefix `packed_`, set output suffix *(empty)* |

**CLI:** `imgplex-cli run wf-06-setmode.imgplex --in fixtures\sets --out out\wf-06`

**Expected:** exactly **2** outputs — `packed_alpha.png` with channel means ≈
(200, 100, 255−50)/255 = (0.784, 0.392, 0.804) and `packed_beta.png` ≈ (0.118, 0.235, 0.647),
±0.02.

---

## WF-07 — Gate + text filter (conditional file output) — `wf-07-gate.imgplex`

**Covers:** per-image gate suppression on the single-command path (null plan → file not
written), `text_filter`, `prop_name` → logic → gate value chain.

**Fixtures:** `fixtures\main`

**Graph:**

```
Name.param-out-value        → Text Filter.param-in-input
Text Filter.param-out-result → Gate.param-in-condition
Input.out-0 → Gate.in-0 ; Gate.out-0 → Image Output.in-0
```

| Node | Params |
|---|---|
| Text Filter | contains `red` (prefix/suffix empty, match_case off) |

**CLI:** `imgplex-cli run wf-07-gate.imgplex --in fixtures\main --out out\wf-07`

**Expected:** exactly one file — `red_256.png`. The other five are suppressed.

---

## WF-08 — All-formats fan-out — `wf-08-formats.imgplex`

**Covers:** `format_convert` + all seven format definitions (`args_js`, extensions), seven
output nodes executed sequentially in a single run (GUI and CLI both iterate all output
nodes).

**Fixtures:** `fixtures\main`

**Graph:** one Input; seven parallel branches, each
`Input.out-0 → Convert Format.in-0 ; Convert Format.out-0 → Image Output.in-0`:

| Branch | Convert Format params | Image Output CLI Name / Custom Path |
|---|---|---|
| PNG | format `PNG`, compression `9`, depth `16` | `out-png` / `…\out\wf-08\png` |
| JPEG | format `JPEG`, quality `85`, sampling `4:4:4`, progressive on | `out-jpeg` / `…\out\wf-08\jpeg` |
| WEBP | format `WEBP`, **lossless on** | `out-webp` / `…\out\wf-08\webp` |
| AVIF | format `AVIF`, quality `60`, speed `8` | `out-avif` / `…\out\wf-08\avif` |
| TIFF | format `TIFF`, compression `LZW` | `out-tiff` / `…\out\wf-08\tiff` |
| BMP | format `BMP` | `out-bmp` / `…\out\wf-08\bmp` |
| TGA | format `TGA`, RLE on | `out-tga` / `…\out\wf-08\tga` |

**CLI:**

```
imgplex-cli run wf-08-formats.imgplex --in fixtures\main --out-png out\wf-08\png --out-jpeg out\wf-08\jpeg --out-webp out\wf-08\webp --out-avif out\wf-08\avif --out-tiff out\wf-08\tiff --out-bmp out\wf-08\bmp --out-tga out\wf-08\tga
```

**Expected:** per format: 6 files with the right extension (`.png .jpg .webp .avif .tif
.bmp .tga`), `magick identify` reports the matching codec, and the converted `red_256`
stays red (mean R > 0.95, mean G < 0.05). WEBP lossless round-trips `red_256.png` with
zero pixel difference (`magick compare -metric AE`). The `png_depth = 16` setting is
reported as a soft note only: ImageMagick's PNG encoder reduces the stored bit depth for
flat-color content despite `-depth 16` (forcing it would need `-define png:bit-depth=16`
in `format-definitions/png.json`).

---

## WF-09 — Rename blocks + overwrite semantics — `wf-09-rename.imgplex`

**Covers:** `rename` node (`computeNewName` with Text / Number / Old-Name blocks,
per-image auto-increment), copy fast path (no image op at all), skip-vs-overwrite modes.

**Fixtures:** `fixtures\main`

**Graph:** `Input.out-0 → Rename.in-0 ; Rename.out-0 → Image Output.in-0`

| Node | Params |
|---|---|
| Rename | blocks in order: **Text** `test_` → **Number** start `1`, pad `3` → **Text** `_` → **Old Name** (no find/replace) |

**CLI:** `imgplex-cli run wf-09-rename.imgplex --in fixtures\main --out out\wf-09`

**Expected:** `test_001_alpha_grad_128.png`, `test_002_blue_256.png`,
`test_003_checker_100x80.png`, `test_004_gray50_256.png`, `test_005_green_256.png`,
`test_006_red_256.png`. The runner then plants a sentinel in one output file and re-runs
**without** `--overwrite` (skip mode leaves the sentinel intact) and again **with**
`--overwrite` (the sentinel is replaced by real image data). Timestamps are not usable
here: Windows `CopyFile` preserves the source mtime.

---

## WF-10 — Flipbook atlas — `wf-10-flipbook.imgplex`

**Covers:** `flipbookOutputNode` → montage path, name sorting, grid fill, **param-wired
background color** (`value_color.rgba → param-in-bgColor`).

**Fixtures:** `fixtures\flip` (3 images into a 2×2 grid → 4th cell shows the background)

**Graph:**

```
Input.out-0 → Flipbook Output.in-0
Color.param-out-rgba → Flipbook Output.param-in-bgColor
```

| Node | Params |
|---|---|
| Color (`value_color`) | color `(1, 0, 1, 1)` — magenta |
| Flipbook Output | rows `2`, cols `2`, cell `64×64`, sort by `name`, output path `…\out\wf-10\atlas.png` |

**CLI:** `imgplex-cli run wf-10-flipbook.imgplex --in fixtures\flip --atlas out\wf-10\atlas.png`

**Expected:** `atlas.png` is 128×128; quadrant means: top-left red, top-right green,
bottom-left blue, bottom-right magenta (±0.02 per channel).

---

## WF-11 — Math / vector / value plumbing — `wf-11-compute.imgplex`

**Covers:** every compute family (math, lerp, power, vector length/normalize, split,
comparison, branch, float/vector/string values), a **value wire driving an image node's
param** (blur sigma), and compute results in a text report.

**Fixtures:** `fixtures\main`

**Graph:**

```
Float 0.5.param-out-value → Lerp.param-in-t          (Lerp: a=0, b=10 → result 5)
Lerp.param-out-result     → Blur.param-in-sigma
Input.out-0 → Blur.in-0 ; Blur.out-0 → Image Output.in-0

Vector3.param-out-xyz → Length.param-in-vec           (Vector3: (3, 0, 4) → length 5)
Vector3.param-out-xyz → Normalize.param-in-vec
Normalize.param-out-result → Split.param-in-vec       (x = 0.6)
                                                      (Power: base 2, exponent 10 → 1024)
Length.param-out-result   → Compare.param-in-a        (operator: greater than)
Float 4.9.param-out-value → Compare.param-in-b
Compare.param-out-result  → Branch.param-in-condition (value_true "OK", value_false "BAD")

Input.out-0 → Text Output.in-0
txo-0 ← Lerp.param-out-result
txo-1 ← Length.param-out-result
txo-2 ← Split.param-out-x
txo-3 ← Power.param-out-result
txo-4 ← Branch.param-out-result
```

**CLI:** `imgplex-cli run wf-11-compute.imgplex --in fixtures\main --out out\wf-11\images --report out\wf-11\report.txt`

**Expected:** 6 blurred images in `out\wf-11\images`, and `report.txt` = six identical
lines: `5,5,0.6,1024,OK`.

---

## CLI behavior tests (runner phase 3, no extra workflows needed)

- `imgplex-cli run wf-01-fastpath.imgplex` **without** `--in` → non-zero exit, error
  mentions the missing flag.
- WF-09 triple-run skip/overwrite check (described above).

## Coverage map

| Pipeline area | Workflow |
|---|---|
| Path A: fused single command | WF-01, WF-09 (pure copy) |
| Path B: per-image plan, light meta | WF-02, WF-07 |
| Path B: heavy meta (`loadImageMeta`) | WF-03 |
| Path C: multi-stream, visual split + lazy fusion + constant merge channel | WF-04 |
| Path C: analysis-only split (`loadMultipleChannelMeans`) | WF-05 |
| Path D: set-mode | WF-06 |
| Gate suppression | WF-05 (computed), WF-07 (name filter) |
| `executeTextBatch` | WF-02, WF-03, WF-05, WF-11 |
| `executeFlipbookBatch` | WF-10 |
| format_convert × 7 format definitions | WF-08 |
| rename / `computeNewName` | WF-09 |
| Overwrite skip vs overwrite | WF-09 (runner) |
| Param wires (value→param, value→image port, value→txo, value→bgColor) | WF-04, WF-05, WF-10, WF-11 |
| Multiple output nodes per run | WF-05, WF-08, WF-11 |
| CLI flag mapping / missing flag / `--overwrite` | runner |
| Preview cache, cancellation, dialogs, sanitize-on-load | `MANUAL_CHECKLIST.md` |
