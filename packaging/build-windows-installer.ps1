<#
.SYNOPSIS
    Build the Bite release binaries and package them into a Windows installer with Inno Setup.

.DESCRIPTION
    The one-command distribution build, and the Windows twin of packaging/build-mac-dmg.sh:

      1. Reads product.json - the canonical source of product name, version and publisher.
      2. Syncs the [workspace.package] version in Cargo.toml to product.json and refreshes
         Cargo.lock, so neither the crate version nor the exe's version resource can drift
         from the installer's.
      3. Regenerates the icon artifacts from build/icons/icon.png (scripts/generate-icons.ps1).
         Both build.rs files read build/icon.ico from disk, so it must be current before the
         build; skipped by -SkipIcon.
      4. Builds bite-gui.exe and bite.exe in release.
      5. Verifies everything the installer ships is present: the bundled ImageMagick, the node
         and format definitions, and the license notices.
      6. Locates ISCC.exe (Inno Setup 6) and compiles packaging\bite.iss, passing the product
         metadata as /D defines.

    Output: dist\Bite-Windows-<version>-Setup.exe

    Bump the version in product.json, re-run this, and the new value flows into the manifests,
    both executables and the installer. The only tracked files it edits are Cargo.toml and
    Cargo.lock (step 2) plus the regenerated icons (step 3).

.PARAMETER SkipBuild
    Skip the cargo release build and use what is already in target\release.

.PARAMETER SkipIcon
    Skip regenerating the icons from build/icons/icon.png.

.EXAMPLE
    pwsh packaging\build-windows-installer.ps1

.EXAMPLE
    pwsh packaging\build-windows-installer.ps1 -SkipBuild
#>
[CmdletBinding()]
param(
    [switch] $SkipBuild,
    [switch] $SkipIcon
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$RepoRoot = Split-Path -Parent $PSScriptRoot
$ProductJson = Join-Path $RepoRoot 'product.json'
$CargoToml = Join-Path $RepoRoot 'Cargo.toml'
$IssScript = Join-Path $PSScriptRoot 'bite.iss'
$DistDir = Join-Path $RepoRoot 'dist'

function Write-Step($message) { Write-Host "==> $message" -ForegroundColor Cyan }

# --- 1. Read product metadata -------------------------------------------------
# Read before anything is built: the version below is what the manifests, both version
# resources and the installer are all synced to.
Write-Step 'Reading product.json'
if (-not (Test-Path $ProductJson)) { throw "product.json not found at $ProductJson." }
$product = Get-Content -Raw -LiteralPath $ProductJson | ConvertFrom-Json

# Check for presence before access: Set-StrictMode throws on a missing property.
$names = $product.PSObject.Properties.Name
$missing = foreach ($field in 'productName', 'version', 'publisher', 'copyright', 'homepage', 'exeName', 'cliName') {
    if (($names -notcontains $field) -or [string]::IsNullOrWhiteSpace([string] $product.$field)) { $field }
}
if ($missing) { throw "product.json is missing required field(s): $($missing -join ', ')" }
if ($names -notcontains 'fileAssociation') { throw 'product.json is missing fileAssociation.' }

$appName = $product.productName
$appVersion = $product.version
$appExe = "$($product.exeName).exe"
$appCliExe = "$($product.cliName).exe"
Write-Host "    $appName $appVersion - $($product.publisher)"

# --- 2. Sync the workspace version to product.json ----------------------------
# Nothing in a `cargo build` reads product.json into a manifest, so the release syncs it. The
# only line-anchored `version = "..."` in Cargo.toml belongs to [workspace.package]: dependency
# versions are written inside inline tables, which this regex cannot reach.
Write-Step "Syncing Cargo.toml [workspace.package] version to $appVersion"
$cargoText = Get-Content -Raw -LiteralPath $CargoToml
$versionRx = [regex] '(?m)^version\s*=\s*"([^"]*)"'
$versionMatch = $versionRx.Match($cargoText)
if (-not $versionMatch.Success) { throw "no [workspace.package] version found in $CargoToml." }
$currentVersion = $versionMatch.Groups[1].Value
if ($currentVersion -eq $appVersion) {
    Write-Host '    already in sync.'
}
else {
    # WriteAllText with an explicit no-BOM encoding: Set-Content -Encoding utf8 adds a BOM under
    # Windows PowerShell 5.1, and the manifest is committed.
    [System.IO.File]::WriteAllText(
        $CargoToml,
        $versionRx.Replace($cargoText, "version = `"$appVersion`"", 1),
        (New-Object System.Text.UTF8Encoding $false))
    Write-Host "    updated: $currentVersion -> $appVersion."
    if ($SkipBuild) { Write-Warning "-SkipBuild, so target\release still holds a $currentVersion build." }
}

# Run unconditionally: the manifest can be in sync while the committed lock is not. `cargo
# update --workspace` re-resolves the workspace members only, so every registry dependency
# stays pinned exactly as it was.
& cargo update --manifest-path $CargoToml --workspace --quiet
if ($LASTEXITCODE -ne 0) { throw "cargo update failed (exit $LASTEXITCODE) - Cargo.lock is not synced to $appVersion." }

# --- 3. Regenerate the icons --------------------------------------------------
# build/icon.ico is read from disk by both build.rs files (exe icon + version resource) and by
# bite.iss (wizard icon), so it has to be current before either runs.
if (-not $SkipIcon) {
    Write-Step 'Regenerating icons from build\icons\icon.png'
    & (Join-Path $RepoRoot 'scripts\generate-icons.ps1')
    if ($LASTEXITCODE -ne 0) { throw "icon generation failed (exit $LASTEXITCODE)." }
}
else {
    Write-Step 'Skipping icon regeneration (-SkipIcon)'
}

# --- 4. Build the release binaries --------------------------------------------
if (-not $SkipBuild) {
    Write-Step 'Building release binaries (cargo build --release -p bite-gui -p bite-cli)'
    Push-Location $RepoRoot
    try {
        & cargo build --release -p bite-gui -p bite-cli
        if ($LASTEXITCODE -ne 0) { throw "cargo build failed (exit $LASTEXITCODE)." }
    }
    finally {
        Pop-Location
    }
}
else {
    Write-Step 'Skipping cargo build (-SkipBuild)'
}

# --- 5. Verify the payload ----------------------------------------------------
Write-Step 'Verifying the installer payload'
$exePath = Join-Path $RepoRoot "target\release\$appExe"
$cliPath = Join-Path $RepoRoot "target\release\$appCliExe"
foreach ($binary in $exePath, $cliPath) {
    if (-not (Test-Path $binary)) { throw "Release binary not found at $binary. Run without -SkipBuild." }
}

# The editor and the CLI both resolve <exe dir>\magick\magick.exe first, so the bundle is what
# makes an install work without ImageMagick on PATH.
$magickExe = Join-Path $RepoRoot 'resources\win\magick\magick.exe'
if (-not (Test-Path $magickExe)) {
    throw "Bundled ImageMagick not found at $magickExe. It is tracked with Git LFS - run ``git lfs pull``."
}
# A Git LFS pointer file is a few hundred bytes; the real binary is tens of megabytes. Shipping
# the pointer produces an installer whose every run fails, so check the size, not just presence.
$magickSize = (Get-Item $magickExe).Length
if ($magickSize -lt 1MB) {
    throw "$magickExe is only $magickSize bytes - that is a Git LFS pointer, not the binary. Run ``git lfs pull``."
}

# Upstream's portable distribution ships the legacy utilities beside magick.exe, and each is a
# full 30 MB static copy of the same library that nothing here ever runs (every call site uses
# `magick <tool>` instead). Re-vendoring a new upstream drop is the one way they come back, and
# they are invisible in a diff - they would just quietly add 210 MB and quintuple the installer.
$strayTools = @(Get-ChildItem -Path (Split-Path $magickExe) -Filter '*.exe' |
    Where-Object { $_.Name -ne 'magick.exe' })
if ($strayTools) {
    throw ("resources\win\magick holds legacy utilities that must not ship: $($strayTools.Name -join ', '). " +
        "magick.exe runs all of them as subcommands - delete them.")
}

foreach ($definitions in 'node-definitions', 'format-definitions') {
    $dir = Join-Path $RepoRoot $definitions
    $count = @(Get-ChildItem -Path $dir -Filter '*.json' -ErrorAction SilentlyContinue).Count
    if ($count -eq 0) { throw "No definitions found in $dir - the install would have no nodes." }
    Write-Host "    $definitions`: $count definitions."
}

# The ImageMagick license requires its notice to travel with the binary, so an installer built
# without it is a compliance bug rather than a cosmetic omission.
foreach ($notice in 'LICENSE', 'THIRD_PARTY_LICENSES') {
    if (-not (Test-Path (Join-Path $RepoRoot $notice))) { throw "Missing license file the installer must ship: $notice" }
}
Write-Host "    ImageMagick bundle: $([math]::Round($magickSize / 1MB, 1)) MB binary."

# --- 6. Compile the installer -------------------------------------------------
Write-Step 'Locating ISCC (Inno Setup 6)'
$iscc = (Get-Command 'ISCC.exe' -ErrorAction SilentlyContinue)?.Source
if (-not $iscc) {
    $candidates = @(
        "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
        "$env:ProgramFiles\Inno Setup 6\ISCC.exe"
    )
    $iscc = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
}
if (-not $iscc) {
    throw "ISCC.exe (Inno Setup 6) not found. Install it from https://jrsoftware.org/isdl.php (or ``winget install JRSoftware.InnoSetup``) or add it to PATH."
}
Write-Host "    $iscc"

New-Item -ItemType Directory -Force -Path $DistDir | Out-Null

Write-Step "Packaging the installer for $appName $appVersion"
& $iscc `
    "/DMyAppName=$appName" `
    "/DMyAppVersion=$appVersion" `
    "/DMyAppPublisher=$($product.publisher)" `
    "/DMyAppCopyright=$($product.copyright)" `
    "/DMyAppExe=$appExe" `
    "/DMyAppCliExe=$appCliExe" `
    "/DMyAppProgId=$($product.fileAssociation.progId)" `
    "/DMyAppTypeName=$($product.fileAssociation.typeName)" `
    "/DMyAppExtension=$($product.fileAssociation.extension)" `
    "/DMyAppUrl=$($product.homepage)" `
    $IssScript
if ($LASTEXITCODE -ne 0) { throw "ISCC failed (exit $LASTEXITCODE)." }

$installer = Join-Path $DistDir "$appName-Windows-$appVersion-Setup.exe"
Write-Host ''
if (Test-Path $installer) {
    Write-Step "Done: $installer"
    Write-Host "    $([math]::Round((Get-Item $installer).Length / 1MB, 1)) MB" -ForegroundColor Green
}
else {
    Write-Step "Done: installer written to $DistDir"
}
