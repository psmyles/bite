<#
  Regenerates every icon artifact from the one source PNG.

  Source of truth: build/icons/icon.png (square, >= 1024x1024). Replace the
  artwork there, run this script, and every build input follows.

  Outputs:
    build/icon.ico                              - multi-resolution Windows ICO;
                                                  embedded in both exes by their
                                                  build.rs and used as the Inno
                                                  Setup wizard icon
    build/icons/icon.ico                        - the same ICO, kept beside the
                                                  master set
    crates/bite-gui/assets/icon-256.png - the window / taskbar icon the
                                                  editor decodes at startup

  The macOS icon (build/icons/mac/) is compiled separately from
  build/icons/bite.icon by scripts/build-icon-mac.sh, which needs Xcode.

  Requires: ImageMagick `magick` on PATH.
#>

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$root = Split-Path -Parent $PSScriptRoot
$source = Join-Path $root 'build\icons\icon.png'

if (-not (Test-Path $source)) {
    throw "Source icon not found: $source"
}

$magickCmd = Get-Command magick -ErrorAction SilentlyContinue
if (-not $magickCmd) { throw "ImageMagick 'magick' not found on PATH" }
$magick = $magickCmd.Source

# --- Windows ICO: one multi-resolution file, frames resized independently -----
# icon:auto-resize writes each listed size as its own frame, so Explorer picks a
# purpose-built 16 or 32 px rendition instead of downscaling the 1024 at draw time.
Write-Host '==> Generating build\icon.ico' -ForegroundColor Cyan
$icoOut = Join-Path $root 'build\icon.ico'
& $magick $source -background none -define icon:auto-resize=256,128,64,48,32,16 $icoOut
if ($LASTEXITCODE -ne 0) { throw "magick failed to build icon.ico (exit $LASTEXITCODE)" }
Copy-Item $icoOut (Join-Path $root 'build\icons\icon.ico') -Force

# --- Runtime window icon -----------------------------------------------------
# include_bytes!-embedded by the editor and decoded at startup (see icon.rs), so
# it lives inside the crate rather than in the packaging assets.
Write-Host '==> Generating crates\bite-gui\assets\icon-256.png' -ForegroundColor Cyan
$assets = Join-Path $root 'crates\bite-gui\assets'
New-Item -ItemType Directory -Force -Path $assets | Out-Null
& $magick $source -background none -resize 256x256 -strip (Join-Path $assets 'icon-256.png')
if ($LASTEXITCODE -ne 0) { throw "magick failed to build icon-256.png (exit $LASTEXITCODE)" }

Write-Host 'Done. Regenerated:' -ForegroundColor Green
Write-Host '  build/icon.ico'
Write-Host '  build/icons/icon.ico'
Write-Host '  crates/bite-gui/assets/icon-256.png'
