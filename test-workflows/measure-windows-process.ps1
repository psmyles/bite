param(
  [Parameter(Mandatory = $true)][string]$Executable,
  [Parameter(Mandatory = $true)][string]$CommandArgumentsBase64
)

$json = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($CommandArgumentsBase64))
$CommandArguments = @($json | ConvertFrom-Json)
$stdout = Join-Path $env:TEMP ("bite-bench-{0}-{1}.out" -f $PID, [guid]::NewGuid())
$stderr = Join-Path $env:TEMP ("bite-bench-{0}-{1}.err" -f $PID, [guid]::NewGuid())
$quoted = $CommandArguments | ForEach-Object {
  if ($_ -match '\s') { '"' + ($_ -replace '"', '\"') + '"' } else { $_ }
}
$watch = [System.Diagnostics.Stopwatch]::StartNew()
$process = Start-Process -FilePath $Executable -ArgumentList $quoted -PassThru -NoNewWindow `
  -RedirectStandardOutput $stdout -RedirectStandardError $stderr
# Force creation of the underlying Process handle so ExitCode remains available
# after short-lived commands have exited.
$null = $process.Handle
$peak = 0L
while (-not $process.HasExited) {
  $process.Refresh()
  $total = [long]$process.WorkingSet64
  Get-Process -Name magick -ErrorAction SilentlyContinue | ForEach-Object { $total += [long]$_.WorkingSet64 }
  if ($total -gt $peak) { $peak = $total }
  Start-Sleep -Milliseconds 10
}
$process.WaitForExit()
$process.Refresh()
$exitCode = $process.ExitCode
$watch.Stop()
$result = [ordered]@{
  exitCode = $exitCode
  elapsedMs = $watch.Elapsed.TotalMilliseconds
  peakWorkingSetBytes = $peak
  stdout = [string](Get-Content -LiteralPath $stdout -Raw -ErrorAction SilentlyContinue)
  stderr = [string](Get-Content -LiteralPath $stderr -Raw -ErrorAction SilentlyContinue)
}
Remove-Item -LiteralPath $stdout, $stderr -Force -ErrorAction SilentlyContinue
$result | ConvertTo-Json -Compress
