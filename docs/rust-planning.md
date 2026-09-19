# Rust command planning

Run `bite plan workflow.bite --in <input-directory> --out <output-directory>`
using the workflow's actual CLI names. Like `run`, planning skips existing
outputs unless `--overwrite` is specified. Planning reads directories, file
headers and existing-output state, but creates no directories or files and
launches no ImageMagick processes.

The JSON response includes topology, resolved parameters and bindings, followed
by metadata, capture and output events. Image output events contain the exact
argument tokens targeting the final path. Execution substitutes an adjacent
temporary path for atomic replacement. Copy and text outputs are separate
operations, with source paths or report contents instead of process arguments.

`complete: false` means planning stopped at an unresolved analysis dependency.
The response includes `requires_analysis`, the unresolved event and the full
symbolic topology. Subsequent concrete events are not yet available. This is
not an execution failure; consumers must check `complete` rather than treating
a successful CLI exit as evidence that every command has been resolved.

## Supplying observations

`--facts facts.json` accepts observations obtained separately from the planner:

```json
{
  "metadata": {
    "D:\\images\\sample.png": {
      "image.name": "sample.png",
      "image.width": 1024,
      "image.height": 512,
      "image.bit_depth": 16
    }
  },
  "captures": [
    {
      "args": ["D:\\images\\sample.png", "-format", "%[fx:mean]", "info:"],
      "value": "0.5"
    }
  ]
}
```

Metadata paths must be absolute and match the input path used by the plan.
Metadata records replace the metadata reader result for that file; include all
fields the workflow references. Known field types are checked; numeric metadata
must be finite and nonnegative. Unknown metadata fields, relative paths,
duplicate capture argument arrays and oversized fact strings are rejected.
Mean results must be finite numbers between zero and one. Capture arguments
must match the recorded unresolved event exactly and are never executed by the
planner. Metadata events identify supplied records with `supplied: true`.

Facts are assertions by the caller. They are not verified against file content,
mtime or ImageMagick version, and should be refreshed whenever inputs change.
Automatic observation collection, fingerprint validation, and fully deferred
downstream plans remain pending.

## Regression coverage

`cargo test --offline -p bite-core --test planning` verifies fast-path, format
and atlas command arrays against the original TypeScript goldens, confirms no
output directories are created, and tests analysis suspension/replay and invalid
facts. The format comparison applies only the documented PNG bit-depth/color
type deviation in memory; it never changes the original golden files.

Channel/set plans fuse operations differently from the legacy temporary-file
pipeline. Exact structural tests cover channel split/negate/constant fill/merge
and multi-group set processing, including operation order and output naming.
Deferred work after unresolved analysis and fact provenance are still required
before the Phase 5 gate can close.

Legacy rename compatibility: batch execution applies the first rename in global
topological order once, even when it is disabled or disconnected from the image
output. The Rust planner and executor retain this behavior; differential tests
cover disabled, disconnected and chained rename nodes.

For image branches originating at different input nodes, legacy batch behavior
selects one traced input per output and uses its current image for other unseeded
input streams. The Rust executor preserves this behavior; it does not zip input
directories. A solid source branch also uses the selected batch image dimensions.
Solid-only graphs without a traceable input still require input-selection work.

Image execution isolates per-image failures and reports `failed` and `errors` in
its batch result. The CLI prints diagnostics and the failed count but preserves
the legacy zero exit status for completed batches containing failed images.
Setup errors and cancellation still fail the command. Planning does not recover
from these errors: missing facts suspend it, and invalid facts remain errors.
