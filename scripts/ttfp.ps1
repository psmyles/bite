<#
.SYNOPSIS
  Time-to-first-frame and footprint harness for the editor.

.DESCRIPTION
  Launches bite-gui.exe N times and reports the median, mean and spread of the milliseconds from
  kernel process creation to the first presented frame, together with the working set and commit
  charge once the process has settled.

  The exe measures itself. With BITE_TIMING pointing at a file it appends one stamped line per
  startup phase; with BITE_EXIT_AFTER_FIRST_FRAME set the first presented frame stamps itself,
  waits 1.5 s for the allocator and the driver to settle, stamps again and exits. This script
  launches, waits, and reads the "FIRST FRAME" and "settled" lines back out of that file. See
  crates/bite-gui/src/timing.rs.

  The first launch of each run is a warm-up that is measured and discarded, so the numbers are
  warm-start numbers: the OS file cache holds the exe, and the shader and driver caches are hot.
  That is the case the targets in docs/sokol-migration-plan.md are written against.

  -Breakdown prints the full phase list from the last launch, which is where a regression is
  read: one line per phase with the elapsed time and the footprint at that moment.

.EXAMPLE
  .\scripts\ttfp.ps1
  .\scripts\ttfp.ps1 -N 9 -Breakdown
  .\scripts\ttfp.ps1 -Exe .\target\release\bite-gui.exe -Csv ttfp.csv
#>
param(
    [string] $Exe = '',
    [int] $N = 5,
    [string] $Label = '',
    [switch] $Breakdown,
    [string] $Csv = ''
)

$ErrorActionPreference = 'Stop'

if (-not $Exe) {
    $Exe = Join-Path (Split-Path $PSScriptRoot -Parent) 'target\release\bite-gui.exe'
}
if (-not (Test-Path $Exe)) {
    throw "no editor at $Exe - build it first: cargo build --release -p bite-gui"
}
if (-not $Label) { $Label = Split-Path $Exe -Leaf }

$sink = Join-Path ([IO.Path]::GetTempPath()) ("bite-ttfp-{0}.txt" -f [Guid]::NewGuid())

# One stamped line: "  563.21 ms   ws   172.4 MB   commit   325.6 MB   FIRST FRAME".
$line = '^\s*([0-9.]+) ms\s+ws\s+([0-9.]+) MB\s+commit\s+([0-9.]+) MB\s+(.*)$'

function Invoke-Launch {
    if (Test-Path $sink) { Remove-Item $sink }
    $env:BITE_TIMING = $sink
    $env:BITE_EXIT_AFTER_FIRST_FRAME = '1'
    try {
        $process = Start-Process -FilePath $Exe -PassThru
        if (-not $process.WaitForExit(30000)) {
            $process.Kill()
            throw 'the editor did not present a frame within 30 s'
        }
    } finally {
        Remove-Item Env:\BITE_TIMING -ErrorAction SilentlyContinue
        Remove-Item Env:\BITE_EXIT_AFTER_FIRST_FRAME -ErrorAction SilentlyContinue
    }
    if (-not (Test-Path $sink)) { throw 'the editor exited without writing the timing sink' }

    $phases = @()
    foreach ($text in Get-Content $sink) {
        if ($text -match $line) {
            $phases += [pscustomobject]@{
                ms      = [double] $Matches[1]
                ws      = [double] $Matches[2]
                commit  = [double] $Matches[3]
                step    = $Matches[4].Trim()
            }
        }
    }
    Remove-Item $sink
    $first = $phases | Where-Object { $_.step -eq 'FIRST FRAME' } | Select-Object -First 1
    $settled = $phases | Where-Object { $_.step -eq 'settled' } | Select-Object -First 1
    if (-not $first) { throw 'the timing sink holds no FIRST FRAME line' }
    if (-not $settled) { throw 'the timing sink holds no settled line' }
    Start-Sleep -Milliseconds 250   # let the previous window's teardown settle
    return [pscustomobject]@{ first = $first; settled = $settled; phases = $phases }
}

function Get-Median([double[]] $values) {
    $sorted = @($values | Sort-Object)
    if ($sorted.Count % 2) { return $sorted[[int][Math]::Floor($sorted.Count / 2)] }
    return ($sorted[$sorted.Count / 2 - 1] + $sorted[$sorted.Count / 2]) / 2
}

function Write-Stat([string] $name, [double[]] $values, [string] $unit) {
    $median = Get-Median $values
    $mean = ($values | Measure-Object -Average).Average
    $sd = [Math]::Sqrt((($values | ForEach-Object { ($_ - $mean) * ($_ - $mean) }) | Measure-Object -Sum).Sum / [Math]::Max(1, $values.Count - 1))
    $sorted = @($values | Sort-Object)
    Write-Host ("  {0,-14} median {1,8:N1} {2,-3}  mean {3,8:N1}  sd {4,6:N1}  min {5,8:N1}  max {6,8:N1}" -f $name, $median, $unit, $mean, $sd, $sorted[0], $sorted[-1])
}

Write-Host ("== {0} ==  {1} launches, warm" -f $Label, $N)
[void] (Invoke-Launch)   # warm-up, measured and discarded

$rows = @()
$ms = @(); $ws = @(); $commit = @()
$last = $null
for ($i = 0; $i -lt $N; $i++) {
    $run = Invoke-Launch
    $last = $run
    $ms += $run.first.ms
    $ws += $run.settled.ws
    $commit += $run.settled.commit
    Write-Host ("  {0,2}: first frame {1,8:N1} ms   settled ws {2,7:N1} MB   commit {3,7:N1} MB" -f ($i + 1), $run.first.ms, $run.settled.ws, $run.settled.commit)
    $rows += [pscustomobject]@{ build = $Label; iter = $i + 1; first_frame_ms = $run.first.ms; working_set_mb = $run.settled.ws; commit_mb = $run.settled.commit }
}

Write-Host ''
Write-Stat 'first frame' $ms 'ms'
Write-Stat 'working set' $ws 'MB'
Write-Stat 'commit' $commit 'MB'

if ($Breakdown) {
    Write-Host "`n  phase breakdown of the last launch:"
    foreach ($phase in $last.phases) {
        Write-Host ("  {0,9:N2} ms   ws {1,7:N1} MB   commit {2,7:N1} MB   {3}" -f $phase.ms, $phase.ws, $phase.commit, $phase.step)
    }
}

if ($Csv) { $rows | Export-Csv -NoTypeInformation -Path $Csv; Write-Host "`nraw results: $Csv" }
