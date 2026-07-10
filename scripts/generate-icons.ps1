<#
  Regenerates all packaged/runtime icon assets from a single source PNG.

  Source of truth: build/icons/icon.png (should be square, >= 1024x1024).
  Edit that file first, then run this script.

  Outputs:
    build/icon.ico          - multi-res Windows ICO (NSIS installer/app icon)
    build/icons/icon.ico    - same multi-res ICO, kept alongside the icns/png master set
    build/icons/icon.icns   - macOS icon bundle (full retina size ladder)
    build/icons/icon.png    - unchanged; source for Linux AppImage packaging
    public/icon.png         - 512x512, runtime BrowserWindow icon (electron/main.ts)
    public/favicon.png      - 64x64, app title bar icon (index.html)

  Requires: ImageMagick `magick` on PATH, and node_modules/app-builder-bin
  (installed as a transitive dep of electron-builder).
#>

$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
$source = Join-Path $root 'build\icons\icon.png'

if (-not (Test-Path $source)) {
    throw "Source icon not found: $source"
}

if (-not (Get-Command magick -ErrorAction SilentlyContinue)) {
    throw "ImageMagick 'magick' not found on PATH"
}

$appBuilder = Join-Path $root 'node_modules\app-builder-bin\win\x64\app-builder.exe'
if (-not (Test-Path $appBuilder)) {
    throw "app-builder.exe not found at $appBuilder (run npm install)"
}

$work = Join-Path $env:TEMP "imgplex-icon-gen-$([guid]::NewGuid())"
New-Item -ItemType Directory -Path $work | Out-Null

try {
    # --- Windows ICO: multi-resolution, built from independently-resized frames ---
    Write-Host "Generating multi-resolution ICO..."
    $sizes = 16, 32, 48, 64, 128, 256
    $sizedPngs = @()
    foreach ($size in $sizes) {
        $out = Join-Path $work "$size.png"
        & magick $source -resize "${size}x${size}" $out
        $sizedPngs += $out
    }
    $icoOut = Join-Path $work 'icon.ico'
    & magick @sizedPngs $icoOut

    Copy-Item $icoOut (Join-Path $root 'build\icon.ico') -Force
    Copy-Item $icoOut (Join-Path $root 'build\icons\icon.ico') -Force

    # --- macOS ICNS: generated via app-builder (same tool electron-builder uses) ---
    Write-Host "Generating ICNS..."
    $icnsWork = Join-Path $work 'icns'
    New-Item -ItemType Directory -Path $icnsWork | Out-Null
    & $appBuilder icon --format icns --input $source --out $icnsWork --root $root
    Copy-Item (Join-Path $icnsWork 'icon.icns') (Join-Path $root 'build\icons\icon.icns') -Force

    # --- Runtime icons used directly by the app (not by electron-builder) ---
    Write-Host "Generating runtime icons..."
    & magick $source -resize 512x512 (Join-Path $root 'public\icon.png')
    & magick $source -resize 64x64 (Join-Path $root 'public\favicon.png')

    Write-Host "Done. Regenerated:"
    Write-Host "  build/icon.ico"
    Write-Host "  build/icons/icon.ico"
    Write-Host "  build/icons/icon.icns"
    Write-Host "  public/icon.png"
    Write-Host "  public/favicon.png"
}
finally {
    Remove-Item $work -Recurse -Force -ErrorAction SilentlyContinue
}
