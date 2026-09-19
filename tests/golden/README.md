# Legacy behavioral reference

Run `npm run test:golden` for definition, compute, graph, and naming goldens.
Run `npm run test:workflows` for actual ImageMagick workflow execution and the CLI
contract. The runner redirects every output into a fresh `test-workflows/out/`
directory and leaves source fixtures untouched. `--only wf-04` can be passed
directly to `node test-workflows/run-tests.mjs` for diagnosis.

To intentionally regenerate, set `BITE_UPDATE_GOLDENS=1` in the environment and
run those commands against the **legacy** backend. Review the diff. Normal test
runs never rewrite expected results. Node/format files include the original
definition, cases, and exact returned argument tokens or computed parameters.
Nonfinite numbers use `{"$number":"NaN"}` / `Infinity` markers.

The execution reference compares filenames, dimensions, text, CLI diagnostics,
exit codes, and overwrite semantics. The runner additionally checks channel
means, codecs, and an exact lossless WebP round-trip. Encoded byte sizes and EXIF
write support are version-dependent and explicitly normalized. Actual codec
version and platform are recorded in each run's `results.json`.

`BITE_TEST_CLI` can point to a Rust executable for the same integration checks.
Use `--compare-with <legacy-run-directory>` to additionally compare every output
image: AE=0 for lossless outputs, normalized RMSE <=0.005 for JPEG and AVIF.
Rust help permits additive commands; stderr and exit codes remain exact, and
the named flag/overwrite/default-skip help contract is checked explicitly.
The runner refuses to update legacy goldens when `BITE_TEST_CLI` is set.
macOS verification is pending; passing Windows runs do not establish macOS parity.

`npm run test:compat` builds both CLIs and runs additional live differential
characterization cases: incomplete sets, natural ascending/descending atlas
ordering, collision-safe rename, and separate input branches. Each backend gets
its own workflow copy and output directory. These checks preserve the original
goldens and compare output pixels directly. Collision suffix assignment is
normalized as documented in `KNOWN_DEVIATIONS.md`.
