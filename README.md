# Bite

Bite (Batch Image Transformation Editor) is a node-based batch image workflow creator and processor. All the processing is handled by ImageMagick, Bite just makes the string of commands needed to pass onto ImageMagick.

Built in [Rust](https://www.rust-lang.org/) on [ImageMagick](https://imagemagick.org/), with a
native editor drawn with [Dear ImGui](https://github.com/ocornut/imgui) over
[wgpu](https://wgpu.rs/).

The hosted workflow builder at [psmyles.github.io/bite](https://psmyles.github.io/bite/) is the
earlier Electron/Svelte renderer, kept on the `main` branch; it is not built from this tree.

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

## Documentation

- [Getting Started](docs/getting-started.md)
- [Expression Language](docs/expression-language.md) - how a v2 definition computes its arguments
- [Node Authoring Guide](docs/node-authoring-guide.md) - structure still current, the JavaScript
  sections describe the removed v1 format
- [Native Editor](docs/native-gui-prototype.md) and its
  [parity record](docs/native-editor-parity.md)
- [Project Spec](docs/spec.md) - the original TypeScript implementation, kept as the behavioural
  contract

---

### Requirements

- [Rust](https://www.rust-lang.org/tools/install) 1.87+ (stable)
- A C++ toolchain for the vendored Dear ImGui: MSVC build tools on Windows, Xcode command line
  tools on macOS
- [ImageMagick](https://imagemagick.org) 7+ - both packaged builds bundle their own binary;
  building from source needs `magick` on your PATH (or `resources/win/magick/`,
  `resources/mac/magick/`, which the binaries check first)
- The submodules: `git submodule update --init --recursive`
- The bundled ImageMagick binaries are tracked with Git LFS: `git lfs pull`

### Development

```bash
cargo run -p bite-gui     # the editor
cargo run -p bite-cli -- --help     # the CLI
cargo test                          # unit, definition-parity and planning tests
pwsh test-workflows/run-tests.ps1   # end-to-end ImageMagick execution (Windows)
```

`cargo test` covers the expression language, the node and format definitions, graph planning and
the command builder against the goldens in `tests/golden/`. `test-workflows/run-tests.ps1`
generates deterministic fixtures and runs all eleven reference workflows through the release CLI,
asserting the actual encoded output; see `test-workflows/README.md`.

### Build

```bash
cargo build --release                                # both binaries
pwsh packaging/build-windows-installer.ps1           # Windows: installer in dist/
packaging/build-mac-release.sh                       # macOS: signed, notarized .dmg in dist/
```

`packaging/build-windows-installer.ps1` is the whole distribution build: it syncs the crate
version to `product.json`, regenerates the icons, builds `bite-gui.exe` and `bite.exe`, checks
the payload (bundled ImageMagick, node and format definitions, license notices) and compiles
`packaging/bite.iss` with [Inno Setup 6](https://jrsoftware.org/isdl.php), producing
`dist/Bite-Windows-<version>-Setup.exe`.

`product.json` is the single source of the product name, version and publisher: both `build.rs`
files read it into the executables' Windows version resources, and the packaging script passes it
to the installer. Bump it there and re-run the script.

The installer lays both binaries out in one directory with `node-definitions/`,
`format-definitions/` and `magick/` beside them, which is how each finds the other's
resources. It offers the `.bite` file association, a PATH entry for the CLI and a desktop
shortcut, and installs per-user by default (`%LOCALAPPDATA%\Programs\Bite`, where the Unity
integration in `unity-tools/` looks for it) or machine-wide when elevated.

#### Icons

`build/icons/icon.png` is the one master. `scripts/generate-icons.ps1` renders it into
`build/icon.ico` (embedded in both executables by their `build.rs`, and the installer's wizard
icon) and `crates/bite-gui/assets/icon-256.png` (the window and taskbar icon). The
packaging script runs it for you.

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
