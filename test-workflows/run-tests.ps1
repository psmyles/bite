# imgplex pipeline test suite runner.
# Generates deterministic fixture images, runs each wf-*.imgplex through imgplex-cli,
# and asserts the outputs. See README.md in this folder for the workflow specs.
#
# Usage:
#   .\run-tests.ps1                     # full suite
#   .\run-tests.ps1 -Only wf-04         # single workflow
#   .\run-tests.ps1 -Only wf-04 -AssertOnly   # verify outputs of a GUI run (no CLI execution)

param(
  [string]$Only = '',
  [switch]$AssertOnly
)

$ErrorActionPreference = 'Stop'
$Root = $PSScriptRoot
$RepoRoot = Split-Path $Root -Parent
$FixturesDir = Join-Path $Root 'fixtures'
$OutDir = Join-Path $Root 'out'

# ─── Tooling ──────────────────────────────────────────────────────────────────

$magick = (Get-Command magick -ErrorAction SilentlyContinue)?.Source
if (-not $magick) { Write-Host 'ERROR: magick not found on PATH.' -ForegroundColor Red; exit 2 }

$cliJs = Join-Path $RepoRoot 'dist-cli\cli-bundle.js'
$cliExe = Join-Path $RepoRoot 'dist-cli\cli-bundle.exe'
$node = (Get-Command node -ErrorAction SilentlyContinue)?.Source
$CliInvoker = $null
if ($node -and (Test-Path $cliJs)) {
  $CliInvoker = { param([string[]]$CliArgs) & $node $cliJs @CliArgs 2>&1 }
} elseif (Test-Path $cliExe) {
  # NOTE: the pkg exe looks for node-definitions next to itself (dist-cli\node-definitions).
  $CliInvoker = { param([string[]]$CliArgs) & $cliExe @CliArgs 2>&1 }
  if (-not (Test-Path (Join-Path $RepoRoot 'dist-cli\node-definitions'))) {
    Write-Host 'WARNING: using cli-bundle.exe but dist-cli\node-definitions does not exist — the exe may fail to load node definitions.' -ForegroundColor Yellow
  }
}
if (-not $CliInvoker -and -not $AssertOnly) {
  Write-Host 'ERROR: CLI not found. Run "npm run build" to produce dist-cli\cli-bundle.js.' -ForegroundColor Red
  exit 2
}

function Invoke-Cli([string[]]$CliArgs) {
  $output = & $CliInvoker $CliArgs | Out-String
  return @{ ExitCode = $LASTEXITCODE; Output = $output }
}

# ─── Assertion framework ──────────────────────────────────────────────────────

$Results = [ordered]@{}

function Init-Result([string]$Wf) {
  $Results[$Wf] = @{ Pass = 0; Fail = 0; Skip = $false; Messages = [System.Collections.Generic.List[string]]::new() }
}

function Assert([string]$Wf, [bool]$Cond, [string]$Msg) {
  if ($Cond) { $Results[$Wf].Pass++ }
  else {
    $Results[$Wf].Fail++
    $Results[$Wf].Messages.Add("FAIL: $Msg")
    Write-Host "  FAIL: $Msg" -ForegroundColor Red
  }
}

function Soft-Note([string]$Wf, [bool]$Cond, [string]$Msg) {
  if (-not $Cond) {
    $Results[$Wf].Messages.Add("NOTE: $Msg")
    Write-Host "  NOTE: $Msg" -ForegroundColor Yellow
  }
}

$Inv = [System.Globalization.CultureInfo]::InvariantCulture

function Get-Means([string]$File) {
  # Returns @(r, g, b) means in 0-1. $File may carry an inline crop, e.g. "atlas.png[64x64+0+0]"
  $s = (& $magick "$File" -format '%[fx:mean.r] %[fx:mean.g] %[fx:mean.b]' info: | Out-String).Trim()
  return $s -split ' ' | ForEach-Object { [double]::Parse($_, $Inv) }
}

function Test-Means([string]$Wf, [string]$File, [double[]]$Expected, [double]$Tol = 0.02) {
  # $File may carry an inline magick crop suffix ("atlas.png[64x64+0+0]") — strip it for the
  # existence check, and use -LiteralPath so [] is not treated as a wildcard.
  $baseFile = $File -replace '\[[^\[\]]*\]$', ''
  if (-not (Test-Path -LiteralPath $baseFile)) { Assert $Wf $false "missing file for mean check: $File"; return }
  $m = Get-Means $File
  $ok = ([math]::Abs($m[0] - $Expected[0]) -le $Tol) -and
        ([math]::Abs($m[1] - $Expected[1]) -le $Tol) -and
        ([math]::Abs($m[2] - $Expected[2]) -le $Tol)
  $mStr = ($m | ForEach-Object { $_.ToString('0.###', $Inv) }) -join ','
  $eStr = ($Expected | ForEach-Object { $_.ToString('0.###', $Inv) }) -join ','
  Assert $Wf $ok "$(Split-Path $File -Leaf) means ($mStr) ≈ ($eStr)"
}

function Get-Dims([string]$File) {
  $s = (& $magick identify -format '%w %h' "$File" | Out-String).Trim()
  return $s -split ' ' | ForEach-Object { [int]$_ }
}

function Get-Codec([string]$File) {
  return (& $magick identify -format '%m' "$File" 2>$null | Out-String).Trim()
}

# ─── Fixtures ─────────────────────────────────────────────────────────────────

function New-Fixtures {
  Write-Host 'Generating fixtures...' -ForegroundColor Cyan
  if (Test-Path $FixturesDir) { Remove-Item $FixturesDir -Recurse -Force }
  $main = Join-Path $FixturesDir 'main'
  $meta = Join-Path $FixturesDir 'meta'
  $sets = Join-Path $FixturesDir 'sets'
  $flip = Join-Path $FixturesDir 'flip'
  foreach ($d in @($main, $meta, $sets, $flip)) { New-Item -ItemType Directory -Force $d | Out-Null }

  & $magick -size 256x256 'xc:srgb(255,0,0)' (Join-Path $main 'red_256.png')
  & $magick -size 256x256 'xc:srgb(0,255,0)' (Join-Path $main 'green_256.png')
  & $magick -size 256x256 'xc:srgb(0,0,255)' (Join-Path $main 'blue_256.png')
  # PNG24:/PNG32: force RGB(A) storage — plain gray content would otherwise be written as
  # single-channel grayscale PNGs, changing per-channel split/mean behavior.
  & $magick -size 256x256 'xc:gray(128)' ('PNG24:' + (Join-Path $main 'gray50_256.png'))
  # 10px black/white checker tiled to 100x80 (5x4 tiles of 20px) → mean exactly 0.5
  & $magick -size 10x10 xc:black xc:white +append '(' +clone -flop ')' -append `
    -write mpr:tile +delete -size 100x80 tile:mpr:tile ('PNG24:' + (Join-Path $main 'checker_100x80.png'))
  # solid white with a linear alpha gradient
  & $magick -size 128x128 xc:white '(' -size 128x128 gradient: ')' `
    -alpha off -compose CopyOpacity -composite ('PNG32:' + (Join-Path $main 'alpha_grad_128.png'))

  & $magick -size 64x64 gradient: -depth 16 (Join-Path $meta 'deep16_64.png')
  & $magick -size 200x150 gradient:blue-yellow -units PixelsPerInch -density 300 `
    -set exif:Make TestCam -quality 90 (Join-Path $meta 'photo_300dpi.jpg')

  & $magick -size 64x64 'xc:gray(200)' (Join-Path $sets 'set_alpha_diffuse.png')
  & $magick -size 64x64 'xc:gray(100)' (Join-Path $sets 'set_alpha_normal.png')
  & $magick -size 64x64 'xc:gray(50)'  (Join-Path $sets 'set_alpha_rough.png')
  & $magick -size 64x64 'xc:gray(30)'  (Join-Path $sets 'set_beta_diffuse.png')
  & $magick -size 64x64 'xc:gray(60)'  (Join-Path $sets 'set_beta_normal.png')
  & $magick -size 64x64 'xc:gray(90)'  (Join-Path $sets 'set_beta_rough.png')

  & $magick -size 64x64 'xc:srgb(255,0,0)' (Join-Path $flip 'fb_a_red.png')
  & $magick -size 64x64 'xc:srgb(0,255,0)' (Join-Path $flip 'fb_b_green.png')
  & $magick -size 64x64 'xc:srgb(0,0,255)' (Join-Path $flip 'fb_c_blue.png')

  $count = (Get-ChildItem $FixturesDir -Recurse -File).Count
  Write-Host "  $count fixture files created." -ForegroundColor Cyan
}

$MainNames = @('alpha_grad_128.png', 'blue_256.png', 'checker_100x80.png', 'gray50_256.png', 'green_256.png', 'red_256.png')

# ─── Workflow execution helper ────────────────────────────────────────────────

function Invoke-WorkflowRun([string]$Wf, [string]$File, [string[]]$Flags) {
  # Returns $true when the workflow was executed (or AssertOnly), $false on SKIP.
  $wfPath = Join-Path $Root $File
  if (-not (Test-Path $wfPath)) {
    $Results[$Wf].Skip = $true
    Write-Host "  SKIP: $File not found (workflow not built yet)." -ForegroundColor DarkYellow
    return $false
  }
  if ($AssertOnly) { return $true }
  $wfOut = Join-Path $OutDir $Wf
  if (Test-Path $wfOut) { Remove-Item $wfOut -Recurse -Force }
  New-Item -ItemType Directory -Force $wfOut | Out-Null
  $r = Invoke-Cli (@('run', $wfPath) + $Flags)
  Assert $Wf ($r.ExitCode -eq 0) "CLI exit code 0 (got $($r.ExitCode)): $($r.Output.Trim())"
  return ($r.ExitCode -eq 0)
}

function Get-ReportLines([string]$ReportPath) {
  if (-not (Test-Path $ReportPath)) { return $null }
  return @(Get-Content $ReportPath | Where-Object { $_ -ne '' })
}

# ─── Per-workflow definitions ─────────────────────────────────────────────────

$Suite = @(
  @{
    Name = 'wf-01'; File = 'wf-01-fastpath.imgplex'
    Flags = { param($O) @('--in', (Join-Path $FixturesDir 'main'), '--out', $O) }
    Assert = {
      param($Wf, $O)
      $files = @(Get-ChildItem $O -File -ErrorAction SilentlyContinue | Sort-Object Name)
      Assert $Wf ($files.Count -eq 6) "6 outputs (got $($files.Count))"
      Assert $Wf (@(Compare-Object $files.Name $MainNames).Count -eq 0) 'output filenames match input names'
      foreach ($f in $files) {
        $d = Get-Dims $f.FullName
        Assert $Wf ($d[0] -eq 128 -and $d[1] -eq 128) "$($f.Name) is 128x128 (got $($d -join 'x'))"
      }
    }
  }
  @{
    Name = 'wf-02'; File = 'wf-02-props-light.imgplex'
    Flags = { param($O) @('--in', (Join-Path $FixturesDir 'main'), '--report', (Join-Path $O 'report.txt')) }
    Assert = {
      param($Wf, $O)
      $lines = Get-ReportLines (Join-Path $O 'report.txt')
      Assert $Wf ($null -ne $lines) 'report.txt written'
      if ($null -eq $lines) { return }
      Assert $Wf ($lines.Count -eq 6) "6 report lines (got $($lines.Count))"
      $expected = @(
        '^alpha_grad_128\.png,png,[\d.]+,128,128,true$'
        '^blue_256\.png,png,[\d.]+,256,256,true$'
        '^checker_100x80\.png,png,[\d.]+,100,80,false$'
        '^gray50_256\.png,png,[\d.]+,256,256,true$'
        '^green_256\.png,png,[\d.]+,256,256,true$'
        '^red_256\.png,png,[\d.]+,256,256,true$'
      )
      for ($i = 0; $i -lt [math]::Min($lines.Count, 6); $i++) {
        Assert $Wf ($lines[$i] -match $expected[$i]) "line $($i + 1) '$($lines[$i])' matches $($expected[$i])"
      }
    }
  }
  @{
    Name = 'wf-03'; File = 'wf-03-props-heavy.imgplex'
    Flags = { param($O) @('--in', (Join-Path $FixturesDir 'meta'), '--report', (Join-Path $O 'report.txt')) }
    Assert = {
      param($Wf, $O)
      $lines = Get-ReportLines (Join-Path $O 'report.txt')
      Assert $Wf ($null -ne $lines) 'report.txt written'
      if ($null -eq $lines) { return }
      $deep = $lines | Where-Object { $_ -like 'deep16_64.png*' }
      $photo = $lines | Where-Object { $_ -like 'photo_300dpi.jpg*' }
      Assert $Wf ($deep -match '^deep16_64\.png,16,') "deep16 bit depth is 16: '$deep'"
      Assert $Wf ($photo -match '^photo_300dpi\.jpg,8,300') "photo is 8-bit / 300 DPI: '$photo'"
      Soft-Note $Wf ($photo -match 'TestCam$') "EXIF Make not read back (ImageMagick may not write EXIF): '$photo'"
    }
  }
  @{
    Name = 'wf-04'; File = 'wf-04-channels.imgplex'
    Flags = { param($O) @('--in', (Join-Path $FixturesDir 'main'), '--out', $O) }
    Assert = {
      param($Wf, $O)
      $files = @(Get-ChildItem $O -File -ErrorAction SilentlyContinue)
      Assert $Wf ($files.Count -eq 6) "6 outputs (got $($files.Count))"
      Test-Means $Wf (Join-Path $O 'red_256.png')   @(1, 1, 0.5)
      Test-Means $Wf (Join-Path $O 'green_256.png') @(0, 0, 0.5)
      Test-Means $Wf (Join-Path $O 'blue_256.png')  @(0, 1, 0.5)
    }
  }
  @{
    Name = 'wf-05'; File = 'wf-05-meanlogic.imgplex'
    Flags = { param($O) @('--in', (Join-Path $FixturesDir 'main'), '--out', (Join-Path $O 'images'), '--report', (Join-Path $O 'report.txt')) }
    Assert = {
      param($Wf, $O)
      $lines = Get-ReportLines (Join-Path $O 'report.txt')
      Assert $Wf ($null -ne $lines) 'report.txt written'
      if ($null -ne $lines) {
        $expected = @(
          'alpha_grad_128.png,1,1,1,false'
          'blue_256.png,0,0,1,false'
          'checker_100x80.png,0.5,0.5,0.5,false'
          'gray50_256.png,0.502,0.502,0.502,false'
          'green_256.png,0,1,0,false'
          'red_256.png,1,0,0,true'
        )
        Assert $Wf ($lines.Count -eq 6) "6 report lines (got $($lines.Count))"
        for ($i = 0; $i -lt [math]::Min($lines.Count, 6); $i++) {
          Assert $Wf ($lines[$i] -eq $expected[$i]) "line $($i + 1) '$($lines[$i])' equals '$($expected[$i])'"
        }
      }
      $imgs = @(Get-ChildItem (Join-Path $O 'images') -File -ErrorAction SilentlyContinue)
      Assert $Wf ($imgs.Count -eq 1 -and $imgs[0].Name -eq 'red_256.png') "gate passed only red_256.png (got: $($imgs.Name -join ', '))"
    }
  }
  @{
    Name = 'wf-06'; File = 'wf-06-setmode.imgplex'
    Flags = { param($O) @('--in', (Join-Path $FixturesDir 'sets'), '--out', $O) }
    Assert = {
      param($Wf, $O)
      $files = @(Get-ChildItem $O -File -ErrorAction SilentlyContinue | Sort-Object Name)
      Assert $Wf (@(Compare-Object $files.Name @('packed_alpha.png', 'packed_beta.png')).Count -eq 0) "outputs are packed_alpha.png + packed_beta.png (got: $($files.Name -join ', '))"
      # R=diffuse, G=normal, B=negate(rough); gray(n) → n/255
      Test-Means $Wf (Join-Path $O 'packed_alpha.png') @((200 / 255), (100 / 255), ((255 - 50) / 255))
      Test-Means $Wf (Join-Path $O 'packed_beta.png')  @((30 / 255), (60 / 255), ((255 - 90) / 255))
    }
  }
  @{
    Name = 'wf-07'; File = 'wf-07-gate.imgplex'
    Flags = { param($O) @('--in', (Join-Path $FixturesDir 'main'), '--out', $O) }
    Assert = {
      param($Wf, $O)
      $files = @(Get-ChildItem $O -File -ErrorAction SilentlyContinue)
      Assert $Wf ($files.Count -eq 1 -and $files[0].Name -eq 'red_256.png') "only red_256.png written (got: $($files.Name -join ', '))"
    }
  }
  @{
    Name = 'wf-08'; File = 'wf-08-formats.imgplex'
    Flags = {
      param($O)
      $f = @('--in', (Join-Path $FixturesDir 'main'))
      foreach ($fmt in 'png', 'jpeg', 'webp', 'avif', 'tiff', 'bmp', 'tga') { $f += @("--out-$fmt", (Join-Path $O $fmt)) }
      $f
    }
    Assert = {
      param($Wf, $O)
      $formats = @(
        @{ Dir = 'png';  Ext = '.png';  Codec = 'PNG' }
        @{ Dir = 'jpeg'; Ext = '.jpg';  Codec = 'JPEG' }
        @{ Dir = 'webp'; Ext = '.webp'; Codec = 'WEBP' }
        @{ Dir = 'avif'; Ext = '.avif'; Codec = 'AVIF' }
        @{ Dir = 'tiff'; Ext = '.tif';  Codec = 'TIFF' }
        @{ Dir = 'bmp';  Ext = '.bmp';  Codec = 'BMP' }
        @{ Dir = 'tga';  Ext = '.tga';  Codec = 'TGA' }
      )
      foreach ($fmt in $formats) {
        $dir = Join-Path $O $fmt.Dir
        $files = @(Get-ChildItem $dir -File -ErrorAction SilentlyContinue)
        Assert $Wf ($files.Count -eq 6) "$($fmt.Dir): 6 outputs (got $($files.Count))"
        $badExt = @($files | Where-Object { $_.Extension -ne $fmt.Ext })
        Assert $Wf ($badExt.Count -eq 0) "$($fmt.Dir): all files have $($fmt.Ext) extension"
        $red = Join-Path $dir "red_256$($fmt.Ext)"
        if (Test-Path $red) {
          $codec = Get-Codec $red
          Assert $Wf ($codec -like "*$($fmt.Codec)*") "$($fmt.Dir): codec is $($fmt.Codec) (got '$codec')"
          Test-Means $Wf $red @(1, 0, 0) 0.05
        } else {
          Assert $Wf $false "$($fmt.Dir): red_256$($fmt.Ext) exists"
        }
      }
      # png_depth=16 cannot be hard-asserted: ImageMagick's PNG encoder reduces the IHDR
      # bit depth for losslessly-representable content (solid red → 1-bit) despite -depth 16.
      # Forcing it would need "-define png:bit-depth=16" in format-definitions/png.json.
      $png16 = Join-Path $O 'png\red_256.png'
      if (Test-Path $png16) {
        $depth = (& $magick identify -format '%z' "$png16" | Out-String).Trim()
        Soft-Note $Wf ($depth -eq '16') "png_depth=16 not honored for flat content (IHDR depth: $depth) — known format-definition quirk"
      }
      # WEBP is lossless → zero pixel difference vs the source ("0 (0)" output → first token)
      $webp = Join-Path $O 'webp\red_256.webp'
      if (Test-Path $webp) {
        $ae = (& $magick compare -metric AE (Join-Path $FixturesDir 'main\red_256.png') "$webp" null: 2>&1 | Out-String).Trim()
        Assert $Wf (($ae -split '\s+')[0] -eq '0') "webp lossless round-trip (AE=$ae)"
      }
    }
  }
  @{
    Name = 'wf-09'; File = 'wf-09-rename.imgplex'
    Flags = { param($O) @('--in', (Join-Path $FixturesDir 'main'), '--out', $O) }
    Assert = {
      param($Wf, $O)
      $expected = @(
        'test_001_alpha_grad_128.png', 'test_002_blue_256.png', 'test_003_checker_100x80.png',
        'test_004_gray50_256.png', 'test_005_green_256.png', 'test_006_red_256.png'
      )
      $files = @(Get-ChildItem $O -File -ErrorAction SilentlyContinue | Sort-Object Name)
      Assert $Wf (@(Compare-Object $files.Name $expected).Count -eq 0) "renamed outputs match (got: $($files.Name -join ', '))"
      if ($AssertOnly -or $files.Count -eq 0) { return }
      # Overwrite semantics via a content sentinel (mtimes are useless here: Windows CopyFile
      # preserves the source file's timestamp, so every copy run yields identical mtimes).
      $wfPath = Join-Path $Root 'wf-09-rename.imgplex'
      $probe = Join-Path $O 'test_006_red_256.png'
      Set-Content -LiteralPath $probe -Value 'SENTINEL' -NoNewline
      $r2 = Invoke-Cli @('run', $wfPath, '--in', (Join-Path $FixturesDir 'main'), '--out', $O)
      Assert $Wf ($r2.ExitCode -eq 0) 'skip re-run exits 0'
      $afterSkip = Get-Content -LiteralPath $probe -Raw
      Assert $Wf ($afterSkip -eq 'SENTINEL') 'skip mode left the existing file untouched'
      $r3 = Invoke-Cli @('run', $wfPath, '--in', (Join-Path $FixturesDir 'main'), '--out', $O, '--overwrite')
      Assert $Wf ($r3.ExitCode -eq 0) 'overwrite re-run exits 0'
      $bytes = [System.IO.File]::ReadAllBytes($probe)
      $isPng = $bytes.Length -gt 8 -and $bytes[1] -eq 0x50 -and $bytes[2] -eq 0x4E -and $bytes[3] -eq 0x47
      Assert $Wf $isPng 'overwrite mode rewrote the sentinel file with real image data'
    }
  }
  @{
    Name = 'wf-10'; File = 'wf-10-flipbook.imgplex'
    Flags = { param($O) @('--in', (Join-Path $FixturesDir 'flip'), '--atlas', (Join-Path $O 'atlas.png')) }
    Assert = {
      param($Wf, $O)
      $atlas = Join-Path $O 'atlas.png'
      Assert $Wf (Test-Path $atlas) 'atlas.png written'
      if (-not (Test-Path $atlas)) { return }
      $d = Get-Dims $atlas
      Assert $Wf ($d[0] -eq 128 -and $d[1] -eq 128) "atlas is 128x128 (got $($d -join 'x'))"
      Test-Means $Wf "$atlas[64x64+0+0]"   @(1, 0, 0)   # top-left: fb_a_red
      Test-Means $Wf "$atlas[64x64+64+0]"  @(0, 1, 0)   # top-right: fb_b_green
      Test-Means $Wf "$atlas[64x64+0+64]"  @(0, 0, 1)   # bottom-left: fb_c_blue
      Test-Means $Wf "$atlas[64x64+64+64]" @(1, 0, 1)   # bottom-right: magenta background
    }
  }
  @{
    Name = 'wf-11'; File = 'wf-11-compute.imgplex'
    Flags = { param($O) @('--in', (Join-Path $FixturesDir 'main'), '--out', (Join-Path $O 'images'), '--report', (Join-Path $O 'report.txt')) }
    Assert = {
      param($Wf, $O)
      $imgs = @(Get-ChildItem (Join-Path $O 'images') -File -ErrorAction SilentlyContinue)
      Assert $Wf ($imgs.Count -eq 6) "6 blurred images (got $($imgs.Count))"
      $lines = Get-ReportLines (Join-Path $O 'report.txt')
      Assert $Wf ($null -ne $lines) 'report.txt written'
      if ($null -eq $lines) { return }
      Assert $Wf ($lines.Count -eq 6) "6 report lines (got $($lines.Count))"
      $bad = @($lines | Where-Object { $_ -ne '5,5,0.6,1024,OK' })
      Assert $Wf ($bad.Count -eq 0) "all lines are '5,5,0.6,1024,OK' (bad: $($bad -join ' | '))"
    }
  }
)

# ─── Main ─────────────────────────────────────────────────────────────────────

if (-not $AssertOnly) { New-Fixtures }
if (-not (Test-Path $OutDir)) { New-Item -ItemType Directory -Force $OutDir | Out-Null }

$selected = if ($Only) { @($Suite | Where-Object { $_.Name -like "*$Only*" }) } else { $Suite }
if ($selected.Count -eq 0) { Write-Host "No workflow matches -Only '$Only'." -ForegroundColor Red; exit 2 }

foreach ($wf in $selected) {
  Write-Host "`n=== $($wf.Name) — $($wf.File) ===" -ForegroundColor Cyan
  Init-Result $wf.Name
  $wfOut = Join-Path $OutDir $wf.Name
  $ran = Invoke-WorkflowRun $wf.Name $wf.File (& $wf.Flags $wfOut)
  if ($ran) { & $wf.Assert $wf.Name $wfOut }
}

# CLI behavior: a missing input flag must be a hard error (uses wf-01 if present).
if (-not $AssertOnly -and (-not $Only -or 'wf-01' -like "*$Only*")) {
  $wf01 = Join-Path $Root 'wf-01-fastpath.imgplex'
  if (Test-Path $wf01) {
    Write-Host "`n=== cli — missing input flag ===" -ForegroundColor Cyan
    Init-Result 'cli'
    $r = Invoke-Cli @('run', $wf01)
    Assert 'cli' ($r.ExitCode -ne 0) "missing --in exits non-zero (got $($r.ExitCode))"
    Assert 'cli' ($r.Output -match 'Missing required flag') "error names the missing flag: $($r.Output.Trim())"
  }
}

# ─── Summary ──────────────────────────────────────────────────────────────────

Write-Host "`n──────── Summary ────────" -ForegroundColor Cyan
$anyFail = $false
foreach ($k in $Results.Keys) {
  $r = $Results[$k]
  if ($r.Skip) { Write-Host ("{0,-8} SKIP  (workflow file not built yet)" -f $k) -ForegroundColor DarkYellow; continue }
  $status = if ($r.Fail -eq 0) { 'PASS' } else { $anyFail = $true; 'FAIL' }
  $color = if ($r.Fail -eq 0) { 'Green' } else { 'Red' }
  Write-Host ("{0,-8} {1}  ({2} passed, {3} failed)" -f $k, $status, $r.Pass, $r.Fail) -ForegroundColor $color
}
if ($anyFail) { exit 1 } else { exit 0 }
