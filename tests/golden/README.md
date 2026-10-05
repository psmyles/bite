# Behavioral reference

These goldens were captured from the original TypeScript/Electron implementation and are the
contract the Rust crates are held to. That implementation is gone from this branch (it remains on
`main`), so nothing here can be regenerated any more: the files are now a frozen reference, and a
disagreement means the Rust code changed, not that the expectation is stale.

`cargo test` reads them:

- `crates/bite-expr/tests/golden_parity.rs` - every node and format definition's argument tokens
  and computed parameters, 591 cases. Node and format files include the original definition, its
  cases, and the exact returned tokens. Nonfinite numbers use `{"$number":"NaN"}` / `Infinity`
  markers.
- `crates/bite-core/tests/planning.rs` - the planned ImageMagick command lines per workflow
  (`workflows/<id>-magick.json`), plus naming and set grouping.
- `crates/bite-core/tests/workflows.rs` - migration of each `test-workflows/*.bite` fixture and
  its traversal order (`workflows/<id>.json`).

Actual execution is checked by `test-workflows/run-tests.ps1`, which runs every reference
workflow through the built CLI and asserts filenames, dimensions, text output, channel means,
codecs, an exact lossless WebP round-trip, CLI diagnostics, exit codes and overwrite semantics.
Encoded byte sizes and EXIF write support are version-dependent, so they are normalized rather
than asserted; each run records the codec version and platform in its `results.json`.

`KNOWN_DEVIATIONS.md` records where the Rust implementation deliberately differs, including
collision-suffix assignment. macOS verification is pending: passing Windows runs do not establish
macOS parity.
