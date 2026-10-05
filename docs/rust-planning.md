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

`complete: false` means one or more output instances depend on unresolved image
analysis. The response includes stable dependency IDs in `dependencies` and a
`deferred_outputs` entry for every currently blocked input/output pair. Each
entry names its dependencies and the remaining topology operations. Concrete
commands appear only after their facts are supplied; the planner never invents
branch values. This is not an execution failure, so consumers must check
`complete`.

## Supplying observations

Generate observations separately from the read-only planner:

```text
bite observe workflow.bite --in images --out output --facts-out facts.json
bite plan workflow.bite --in images --out output --facts facts.json
```

`observe` runs only the metadata and capture operations requested by planning.
It iterates until every dependency is resolved, writes the facts document, and
does not emit workflow outputs. `--facts facts.json` then supplies those
observations to `plan`:

```json
{
  "schema_version": 1,
  "provenance": {
    "plan_digest": "...",
    "analysis_identity": "image-magick:...",
    "files": {}
  },
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

Facts are bound to the serialized workflow, the loaded node and format
definition versions, the ImageMagick binary contents, and every analyzed input
file. File fingerprints contain size, nanosecond modification time and a
deterministic content digest. Planning rejects missing provenance, changed
inputs, changed workflow/definitions and a different ImageMagick binary with a
specific stale-facts diagnostic.

## Regression coverage

`cargo test --offline -p bite-core --test planning` verifies fast-path, format
and atlas command arrays against the original TypeScript goldens, confirms no
output directories are created, and tests analysis suspension/replay and invalid
facts. The format comparison applies only the documented PNG bit-depth/color
type deviation in memory; it never changes the original golden files.

Channel/set plans fuse operations differently from the legacy temporary-file
pipeline. Exact structural tests cover channel split/negate/constant fill/merge
and multi-group set processing, including operation order and output naming.
Unresolved-analysis coverage verifies that independent image and report outputs
remain represented together and that iterative observation replay resolves to
the same concrete plan.

Legacy rename compatibility: batch execution applies the first rename in global
topological order once, even when it is disabled or disconnected from the image
output. The Rust planner and executor retain this behavior; differential tests
cover disabled, disconnected and chained rename nodes.

For image branches originating at different input nodes, legacy batch behavior
selects one traced input per output and uses its current image for other unseeded
input streams. The Rust executor preserves this behavior; it does not zip input
directories. A solid source branch also uses the selected batch image dimensions.
When a Solid Image reaches an output without an image connection, the planner
uses the sole workflow Input as its batch and dimension source. It rejects the
case explicitly when there is no Input or more than one possible Input.

Image execution isolates per-image failures and reports `failed` and `errors` in
its batch result. The CLI prints diagnostics and the failed count but preserves
the legacy zero exit status for completed batches containing failed images.
Setup errors and cancellation still fail the command. Planning does not recover
from these errors: missing facts suspend it, and invalid facts remain errors.
