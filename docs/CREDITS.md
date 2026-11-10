# Credits

Bite is built on the projects listed below. For license terms and per-package copyright notices, see [THIRD_PARTY_LICENSES](THIRD_PARTY_LICENSES).

- **[ImageMagick](https://imagemagick.org)** does all of the image processing. Bite builds the
  commands; ImageMagick runs them. This product includes software developed by the ImageMagick
  Studio LLC.
- **[fire-decode](https://github.com/psmyles/fire-decode)** reads every image's size, and makes
  the thumbnails of PNG, TGA, PSD, EXR, HEIC and AVIF files without starting ImageMagick. It brings
  [psd_sdk](https://github.com/MolecularMatters/psd_sdk) by Molecular Matters,
  [libheif](https://github.com/strukturag/libheif) and
  [libde265](https://github.com/strukturag/libde265) by struktur AG,
  [dav1d](https://code.videolan.org/videolan/dav1d) by VideoLAN and
  [libwebp](https://chromium.googlesource.com/webm/libwebp) by Google.
- **[Dear ImGui](https://github.com/ocornut/imgui)** by Omar Cornut, through
  [cimgui](https://github.com/cimgui/cimgui), draws the editor.
- **[sokol](https://github.com/floooh/sokol)** by Andre Weissflog renders it on Direct3D 11 and
  Metal, through [sokol-rust](https://github.com/floooh/sokol-rust).
- **[Rust](https://www.rust-lang.org)** and the crates it's built from, among them
  [winit](https://github.com/rust-windowing/winit), [rfd](https://github.com/PolyMeilex/rfd),
  [arboard](https://github.com/1Password/arboard), [ureq](https://github.com/algesten/ureq),
  [rustls](https://github.com/rustls/rustls), [serde](https://serde.rs),
  [png](https://github.com/image-rs/image-png),
  [image-webp](https://github.com/image-rs/image-webp) and
  [schemars](https://github.com/GREsau/schemars).
- **Fonts:** [Inter](https://github.com/rsms/inter) by The Inter Project Authors and
  [JetBrains Mono](https://github.com/JetBrains/JetBrainsMono) by JetBrains.
- **[Inno Setup](https://jrsoftware.org/isinfo.php)** by Jordan Russell builds the Windows
  installer.
- On macOS, the image format libraries that ImageMagick uses (libpng, libjpeg-turbo, libtiff,
  libwebp, OpenEXR and others) ship with the app.

## Development

- **[Claude Code](https://claude.com/claude-code)** - Anthropic. Coding assistant.
