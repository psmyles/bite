# Bite

Bite (Batch Image Transformation Editor) is a node-based batch image workflow creator and processor. All the processing is handled by ImageMagick, Bite just makes the string of commands needed to pass onto ImageMagick.

Built in [Rust](https://www.rust-lang.org/) on [ImageMagick](https://imagemagick.org/), with a
native editor drawn with [Dear ImGui](https://github.com/ocornut/imgui) through
[sokol_gfx](https://github.com/floooh/sokol), on Direct3D 11 on Windows and Metal on macOS.

---

## Features

- **Visual node graph** - drag nodes from the library, connect typed wires, see results live
- **Multiple inputs and outputs** - any number of Input nodes, each with its own image list; multiple typed output nodes (Image Output, Text Output, Flipbook Output) in a single workflow
- **Batch processing** - run a pipeline against an entire folder; gate node conditionally skips images
- **Set processing** - group images by naming convention (stereo pairs, bracketed exposures, etc.) and process them together as a unit using the `Process as Set` node
- **Live preview** - real-time pipeline output on the selected image, node-level cached
- **CLI export** - export any workflow as a standalone script (PowerShell, Bash, or Windows Command Prompt)
- **Workflow files** - save and load pipelines as `.bite` JSON; double-clicking a `.bite` file opens it directly in the app; version compatibility is checked on open
- **Per-format export settings** - format-specific encoding controls (JPEG quality/chroma/progressive, WebP lossless, PNG compression, AVIF effort, ...) driven by data files in `format-definitions/`
- **Extensible** - add new nodes by dropping a JSON file into `node-definitions/`, no recompile needed
- **Pure-value graph** - math, logic, and value constant nodes with typed wires route parameters without touching the image pipeline
- **Node groups & comments** - visually organise your graph with resizable containers and sticky notes
- **Undo / redo** - full history for all graph edits

---

## Command Line

The `bite` CLI runs a saved workflow without the editor. Every Input and output node has a CLI
name, set in the Inspector, and each one becomes a flag: an Input's names the folder to read and
is required, an output's overrides where that output writes.

```bash
bite run workflow.bite --in <folder> --out <folder>    # run every output node
bite run workflow.bite --in <folder> --overwrite       # replace existing files, whatever each output says
bite run workflow.bite --in <folder> --out <folder> --run-report run.json   # and what it did, as JSON
bite plan workflow.bite --in <folder> --out <folder>   # the commands a run would issue, as JSON
bite describe workflow.bite                            # the workflow's CLI names, as JSON
bite validate workflow.bite                            # load and check a workflow without running it
bite --help                                            # everything else
```

A run exits 0 only when every file worked, 1 when one failed or the run stopped, and 2 when the
command line can't be followed: a name the workflow doesn't have, a flag without its path, or an
input left out. `--run-report` writes the counts, the files written, each file that failed and
why, and the `problem` that stopped the run, however it ends. It isn't `--report` because the
workflow's CLI names share the command line, and `report` is a likely name for a Text Output.

These assume the CLI names `in` and `out`. **CLI export** writes the same call into a script with
every flag the workflow needs, and the Windows installer can put `bite` on your PATH.

---

## Documentation

- [Getting Started](docs/getting-started.md)
- [Expression Language](docs/expression-language.md) - how a v2 definition computes its arguments
- [Node Authoring Guide](docs/node-authoring-guide.md) - structure still current, the JavaScript
  sections describe the removed v1 format
- [Command Planning](docs/rust-planning.md) - what `bite plan` and `bite observe` report
- [Native Editor](docs/native-gui-prototype.md) and its
  [parity record](docs/native-editor-parity.md)
- [Project Spec](docs/spec.md) - the original TypeScript implementation, kept as the behavioural
  contract

---

### Requirements

- Windows, or macOS on Apple Silicon. The CLI and the core crates also build and test on Linux;
  the editor does not, as it has no Linux renderer
- [Rust](https://www.rust-lang.org/tools/install) 1.88+ (stable)
- A C/C++ toolchain for the vendored Dear ImGui and sokol: MSVC build tools on Windows, Xcode
  command line tools on macOS. Both are checked in, so there are no submodules to fetch
- [ImageMagick](https://imagemagick.org) 7+ - both packaged builds bundle their own binary;
  building from source needs `magick` on your PATH (or `resources/win/magick/`,
  `resources/mac/magick/` under the working directory, which the binaries check first). Set
  `BITE_MAGICK` to a binary's path to override all of these
- The Windows ImageMagick binary is tracked with Git LFS: `git lfs pull`. The macOS one is not in
  git; `scripts/bundle-magick-mac.sh` builds it (see [macOS](#macos))

### Development

```bash
cargo run -p bite-gui               # the editor
cargo run -p bite-cli -- --help     # the CLI
cargo test                          # unit, definition-parity and planning tests
pwsh test-workflows/run-tests.ps1   # end-to-end ImageMagick execution (Windows)
```

`cargo test` covers the expression language, the node and format definitions, graph planning and
the command builder against the goldens in `tests/golden/`. On Linux, run
`cargo test --workspace --exclude bite-gui`.

`test-workflows/run-tests.ps1` generates deterministic fixtures and runs all eleven reference
workflows through the CLI, asserting the actual encoded output. It needs PowerShell 7.1+, `magick`
on your PATH (it does not use the bundled one) and a CLI built first with
`cargo build --release -p bite-cli`, falling back to a debug build; see `test-workflows/README.md`.

### Build

```bash
cargo build --release                                # both binaries
cargo xtask package-windows                          # Windows: installer in dist/
packaging/build-mac-release.sh                       # macOS: signed, notarized .dmg in dist/
```

`cargo xtask package-windows` is the whole Windows distribution build: it syncs the crate
version to `product.json`, redraws the icons if the artwork changed, builds `bite-gui.exe` and
`bite.exe`, checks the payload (bundled ImageMagick, node and format definitions, license
notices), draws the wizard images and compiles `packaging/bite.iss` with
[Inno Setup](https://jrsoftware.org/isdl.php) 6.5.2 or later, producing
`dist/Bite-Windows-<version>-Setup.exe`. Pass `--no-build` to reuse `target/release` as it
stands.

`product.json` is the single source of the product name, version and publisher: both `build.rs`
files read it into the executables' Windows version resources, and the packaging passes it to
the installer. Bump it there and re-run `cargo xtask package-windows`.

The installer lays both binaries out in one directory with `node-definitions/`,
`format-definitions/` and `magick/` beside them, which is how each finds the other's
resources. It offers the `.bite` file association, a PATH entry for the CLI and a desktop
shortcut, and installs per-user by default (`%LOCALAPPDATA%\Programs\Bite`, where the Unity
integration in `unity-tools/` looks for it) or machine-wide when elevated.

#### Icons

`build/icons/icon.png` is the one master. `cargo xtask icons` draws it into `build/icon.ico`
(embedded in both executables by their `build.rs`, and the installer's icon) and
`crates/bite-gui/assets/icon-256.png` (the window and taskbar icon). Both are committed, and a
test fails until they match the artwork, so after changing it run `cargo xtask icons` and
commit the result; `package-windows` redraws them for you too. The installer's wizard images
are drawn from the same artwork at each display scale, at packaging time
(`cargo xtask icons --preview DIR` writes them out to look at).

#### macOS

Apple Silicon only - the bundled ImageMagick is arm64. Two scripts, differing only in whether
Apple's notary service is involved:

```bash
packaging/build-mac-dmg.sh        # signed .dmg, for testing the real packaged app
packaging/build-mac-release.sh    # signed, notarized and stapled .dmg - the one to ship
```

Both produce `dist/Bite-Mac-<version>-arm64.dmg`. They sync the crate version to `product.json`,
build `bite-gui` and `bite`, assemble `Bite.app`, check that nothing in it links against the
build machine's Homebrew, sign it with your Developer ID and smoke-test the bundled ImageMagick
from inside the signed bundle. `packaging/mac-common.sh` holds the steps both share. Pass
`--skip-magick --skip-icon --skip-build` to reuse what is already built; `--help` lists the rest.

`build-mac-dmg.sh` stops there, so its output is signed but **not notarized**: it opens on this
Mac and on another one only through System Settings -> Privacy & Security -> Open Anyway. Use
`build-mac-release.sh` for anything you send to someone. That one notarizes and staples the
`.app` before wrapping it, then notarizes and staples the `.dmg` too, and ends by asking
Gatekeeper the same questions a downloader's Mac will. It needs credentials once:

```bash
xcrun notarytool store-credentials bite --apple-id <your-apple-id> \
    --team-id <your-team-id> --password <app-specific-password>
echo 'APPLE_KEYCHAIN_PROFILE=bite' >> .env.mac    # gitignored
```

`APPLE_ID` + `APPLE_APP_SPECIFIC_PASSWORD`, or the `APPLE_API_KEY` trio, work instead.

Two inputs are built by the scripts but can be refreshed on their own, and neither is in git:
`scripts/bundle-magick-mac.sh` vendors a relocatable, signed ImageMagick into
`resources/mac/magick/` (upstream publishes none, so it copies `magick` plus its dylibs and coder
modules out of Homebrew, rewrites their install names and re-signs them), and
`scripts/build-icon-mac.sh` compiles `build/icons/bite.icon` into the layered icon catalogue plus
an `.icns` fallback. The icon script needs full Xcode, not just the command line tools.

The bundle carries the CLI at `Bite.app/Contents/MacOS/bite`; there is no PATH entry, so either
call it by that path or symlink it somewhere yourself.

---
