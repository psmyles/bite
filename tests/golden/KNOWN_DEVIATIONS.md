# Intentional migration deviations

Goldens capture the legacy behavior. Do not regenerate them to hide a Rust change.

| Reference golden                                                                     | Legacy behavior                                                                                            | Required Rust behavior                                                                                                                                                                                                        |
| ------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `nodes/solid_image.json`                                                             | Unregistered executor; no image operation (pass-through)                                                   | Generate the requested solid image                                                                                                                                                                                            |
| `formats/png.json`                                                                   | `-depth 16` may produce lower stored PNG depth for flat content                                            | Use `-define png:bit-depth=16` plus `-define png:color-type=6`; explicit RGBA avoids a grayscale encoder pixel corruption observed with ImageMagick 7.1.2-26                                                                                                                                                                                                |
| `nodes/*.json`, `missing*` cases                                                     | Calling raw v1 evaluators without parameters inconsistently uses hardcoded defaults, `undefined`, or `NaN` | Validated v2 definitions resolve declared defaults before evaluation; parameters without defaults are required (polymorphic value params default to null). Keep the raw reference cases and test the explicit default policy. |
| `nodes/*.json` and `formats/*.json`, values incompatible with declared types/options | Raw builders accept invalid enum strings, fractional integer parameters, and wrong vector dimensions       | Reject invalid typed inputs with contextual errors before argument generation. Numeric min/max remain UI bounds; valid out-of-range numeric values retain the legacy calculation/clamping.                                    |

Output naming (`workflows/wf-06-setmode.json` traversal): legacy runs renamed
every Image Output with the first Rename node in topological order, connected or
not, and a set-mode output ignored Rename entirely and wrote
`setOutputPrefix + middle + setOutputSuffix`. Rust names each output with the
nearest non-bypassed Rename on its own image path. In set mode that Rename works
on the set's full name (input prefix + middle), and its Number block counts sets.
Loading a file whose Image Output still carries the set naming fields inserts a
Rename (`rename-set-N`) in front of that output that writes the legacy name, so
run output is unchanged (`packed_alpha.png` in `workflow-execution.json`). The
traversal check ignores those inserted nodes; every other node must traverse as
the golden records. A Rename that feeds no output now renames nothing, and the
load reports it.

Wired colours (`nodes/tint.json`, `nodes/outline.json`, `nodes/rotate.json`):
legacy builders stringified a colour parameter, so a Color node wired into Tint,
Outline or Rotate emitted its components as `1,0,0,1`, which ImageMagick rejects
as a colour. Rust passes these parameters through `rgba()`: text (`#ff0000`,
`red`) is emitted unchanged, so every golden case - all text - still matches as
recorded, and components become `rgba(255,0,0,1)` (channels 0-255, alpha 0-1).
`crates/bite-core/tests/semantics.rs` covers the wired case.

Flipbook Output (`workflows/wf-10-*`): the legacy batch tiled every input path
without walking the graph, so a Gate in front of the Flipbook never kept a file
out, and the Columns, Rows, Cell Width and Cell Height ports accepted wires that
only the inspector read (the batch resolved a wired background colour alone).
Rust leaves out a file whose stream a Gate shut and uses wired grid and cell
numbers. The golden workflow has neither a Gate nor a wired number, so its
montage command is unchanged.

Wired Process As Set prefix or suffix (`workflows/wf-06-setmode.json` traversal is
unaffected): legacy `getWiredPrefixValue` read the wired source node's stored
parameter. Rust groups by the value the wire carries when it can be worked out
before any file is read (a String node, or values computed from such nodes alone)
and otherwise falls back to the stored parameter as legacy did. The set's name
uses the same effective prefix.

Image dimensions in a run: when only cheap properties were needed, the legacy
batch read width and height from the file header alone, which reports nothing for
TIFF, GIF, PSD, EXR, AVIF and similar, so a Resize wired from Dimensions ran at
one pixel (the legacy preview inspected the file fully and showed the real size).
Rust runs inspect such a file fully whenever something in the output's chain reads
the width or height, so a run resolves what its preview showed.

Additional discrepancies must be documented with a specific case before changing
compatibility expectations. The old workflow README's flip and CLI sanitization
warnings are stale; current source has already fixed both.

The `rename-collisions` differential case in `test-workflows/compat-edgecases.mjs`
characterizes another nondeterminism: legacy parallel workers assign `_1`, `_2`,
etc. after async I/O, so the source image associated with each suffix varies.
Rust claims names in input order. The check requires identical filenames and an
exact pixel multiset, matching each reference image once, rather than treating
one arbitrary legacy worker ordering as a compatibility contract.
