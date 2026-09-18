# Bite

Bite (Batch Image Transformation Editor) is a node-based batch image workflow creator and processor. All the processing is handled by ImageMagick, Bite just makes the string of commands needed to pass onto ImageMagick.

Built with [ImageMagick](https://imagemagick.org/), [Electron](https://electronjs.org/), and [Svelte](https://svelte.dev/).

A functional web version of the application is available at [psmyles.github.io/bite](https://psmyles.github.io/bite/) that can be used for creating workflows but it can't process images.

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

- [Getting Started](https://github.com/psmyles/bite/blob/main/docs/getting-started.md)
- [Node Authoring Guide](https://github.com/psmyles/bite/blob/main/docs/node-authoring-guide.md)
- [Project Spec](https://github.com/psmyles/bite/blob/main/docs/spec.md)

---

### Requirements

- [Node.js](https://nodejs.org) 20+
- [ImageMagick](https://imagemagick.org) 7+ - the Windows installer and the macOS (Apple Silicon) app bundle their own binary; everywhere else `magick` must be on your PATH

### Development

```bash
npm install
npm run dev        # Electron + Vite hot reload
npm test           # Run the unit test suite (Vitest)
```

### Build

```bash
npm run build      # Windows: production build + NSIS installer
npm run build:mac  # macOS (Apple Silicon): signed, notarized .dmg
npm run build:web  # Renderer-only build (browser testing)
```

#### macOS .dmg

`npm run build:mac` vendors ImageMagick into the app, compiles the app icon,
packages it, signs it, and notarizes it. It needs an Apple Silicon Mac with
Homebrew ImageMagick (`brew install imagemagick`), full Xcode (the icon is
compiled with `actool`, which the Command Line Tools do not ship), and a
Developer ID Application certificate in the keychain - the certificate is
detected automatically, as is the team id.

Signing and notarizing take most of the build's wall time. The ImageMagick
bundle counts its ~150 signatures as they go, and the electron-builder and
notary-service waits print the elapsed time every 30s, so a working build is
distinguishable from a hung one; add `--verbose` (e.g.
`scripts/build-mac.sh --verbose`) to also see every `codesign` call.

The app icon is built from `public/bite.icon` (Icon Composer) by
`npm run build:icon:mac`, which `build:mac` runs for you. It writes the compiled
catalogue that macOS 26+ renders as a Liquid Glass icon plus a flattened
`.icns` fallback to `build/icons/mac/` (gitignored).

Notarization credentials are set up once:

```bash
xcrun notarytool store-credentials bite --apple-id <apple-id> \
    --team-id <TEAMID> --password <app-specific-password>
echo 'APPLE_KEYCHAIN_PROFILE=bite' >> .env.mac   # gitignored, read by the build
```

(App-specific passwords come from appleid.apple.com -> Sign-In and Security.)
`APPLE_ID` + `APPLE_APP_SPECIFIC_PASSWORD`, or the `APPLE_API_KEY` /
`APPLE_API_KEY_ID` / `APPLE_API_ISSUER` trio, work instead if you prefer them in
the environment. `--skip-notarize` builds a signed but un-notarized dmg for local
testing; it still warns on anyone else's machine.

The dmg lands in `release/<version>/`, signed, notarized and stapled - both the
app and the disk image around it, so it opens without a network round trip. `scripts/bundle-magick-mac.sh` can be run
on its own (`npm run bundle:magick:mac`) to refresh `resources/mac/magick/`; pass
`--identity -` for an unsigned local bundle. Upstream publishes no relocatable
macOS ImageMagick, so that script builds one: it copies `magick` plus its dylibs
and coder modules out of Homebrew, rewrites their install names, and re-signs
them.

---
