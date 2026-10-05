# Intentional migration deviations

Goldens capture the legacy behavior. Do not regenerate them to hide a Rust change.

| Reference golden                                                                     | Legacy behavior                                                                                            | Required Rust behavior                                                                                                                                                                                                        |
| ------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `nodes/solid_image.json`                                                             | Unregistered executor; no image operation (pass-through)                                                   | Generate the requested solid image                                                                                                                                                                                            |
| `formats/png.json`                                                                   | `-depth 16` may produce lower stored PNG depth for flat content                                            | Use `-define png:bit-depth=16` plus `-define png:color-type=6`; explicit RGBA avoids a grayscale encoder pixel corruption observed with ImageMagick 7.1.2-26                                                                                                                                                                                                |
| `nodes/*.json`, `missing*` cases                                                     | Calling raw v1 evaluators without parameters inconsistently uses hardcoded defaults, `undefined`, or `NaN` | Validated v2 definitions resolve declared defaults before evaluation; parameters without defaults are required (polymorphic value params default to null). Keep the raw reference cases and test the explicit default policy. |
| `nodes/*.json` and `formats/*.json`, values incompatible with declared types/options | Raw builders accept invalid enum strings, fractional integer parameters, and wrong vector dimensions       | Reject invalid typed inputs with contextual errors before argument generation. Numeric min/max remain UI bounds; valid out-of-range numeric values retain the legacy calculation/clamping.                                    |

Additional discrepancies must be documented with a specific case before changing
compatibility expectations. The old workflow README's flip and CLI sanitization
warnings are stale; current source has already fixed both.

The `rename-collisions` differential case in `test-workflows/compat-edgecases.mjs`
characterizes another nondeterminism: legacy parallel workers assign `_1`, `_2`,
etc. after async I/O, so the source image associated with each suffix varies.
Rust claims names in input order. The check requires identical filenames and an
exact pixel multiset, matching each reference image once, rather than treating
one arbitrary legacy worker ordering as a compatibility contract.
