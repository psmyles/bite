<#
.SYNOPSIS
  Drives a running editor window through the states a capture cannot reach.

.DESCRIPTION
  `--capture` renders every panel and dialog offscreen, so it checks what the interface looks
  like. What it cannot check is what only a real window has: a swapchain that is resized, a
  minimise that takes the client area to zero, a maximise, and a close that has to tear the
  graphics stack down in the right order. Those are exactly the paths the sokol shell rewrote
  (`crates/bite-gui/src/render/d3d11.rs`, `platform.rs`), and a mistake in any of them is a crash
  or a hang rather than a wrong pixel.

  So this launches the editor, walks its window through those states from the outside with the
  Win32 calls a user's window manager would make, closes it, and reports whether it exited
  cleanly and whether it logged anything at WARN or ERROR along the way.

.EXAMPLE
  .\scripts\window-exercise.ps1
  .\scripts\window-exercise.ps1 -Exe .\target\release\bite-gui.exe
#>
param(
    [string] $Exe = '',
    [int] $SettleMs = 500
)

$ErrorActionPreference = 'Stop'

if (-not $Exe) {
    $Exe = Join-Path (Split-Path $PSScriptRoot -Parent) 'target\release\bite-gui.exe'
}
if (-not (Test-Path $Exe)) {
    throw "no editor at $Exe - build it first: cargo build --release -p bite-gui"
}

Add-Type -Namespace Win -Name Api -MemberDefinition @'
[DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int w, int t, uint flags);
[DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
[DllImport("user32.dll")] public static extern bool IsWindow(IntPtr h);
[DllImport("user32.dll")] public static extern IntPtr SendMessageTimeout(IntPtr h, uint msg, IntPtr w, IntPtr l, uint flags, uint ms, out IntPtr result);
'@

# The log is per user, not per run, so the run's own lines are the ones written after this mark.
$log = Join-Path ([IO.Path]::GetTempPath()) ("bite-window-{0}.log" -f [Guid]::NewGuid())
$env:BITE_LOG_PATH = $log
try {
    $process = Start-Process -FilePath $Exe -PassThru
    # The window is created a few frames in; wait for it rather than guessing.
    $deadline = (Get-Date).AddSeconds(20)
    while (-not $process.MainWindowHandle -or $process.MainWindowHandle -eq [IntPtr]::Zero) {
        if ((Get-Date) -gt $deadline) { $process.Kill(); throw 'the editor never opened a window' }
        Start-Sleep -Milliseconds 100
        $process.Refresh()
    }
    $window = $process.MainWindowHandle
    Write-Host ("window 0x{0:X}" -f [int64] $window)

    $SWP_NOMOVE = 0x0002; $SWP_NOZORDER = 0x0004
    $steps = @(
        @{ what = 'resize small';      act = { [Win.Api]::SetWindowPos($window, [IntPtr]::Zero, 0, 0, 640, 480, $SWP_NOMOVE -bor $SWP_NOZORDER) } },
        @{ what = 'resize wide';       act = { [Win.Api]::SetWindowPos($window, [IntPtr]::Zero, 0, 0, 1600, 400, $SWP_NOMOVE -bor $SWP_NOZORDER) } },
        @{ what = 'resize tall';       act = { [Win.Api]::SetWindowPos($window, [IntPtr]::Zero, 0, 0, 400, 1000, $SWP_NOMOVE -bor $SWP_NOZORDER) } },
        @{ what = 'minimise';          act = { [Win.Api]::ShowWindow($window, 6) } },   # SW_MINIMIZE - a zero-size client
        @{ what = 'restore';           act = { [Win.Api]::ShowWindow($window, 9) } },   # SW_RESTORE
        @{ what = 'maximise';          act = { [Win.Api]::ShowWindow($window, 3) } },   # SW_MAXIMIZE
        @{ what = 'restore again';     act = { [Win.Api]::ShowWindow($window, 9) } },
        @{ what = 'resize back';       act = { [Win.Api]::SetWindowPos($window, [IntPtr]::Zero, 0, 0, 1280, 800, $SWP_NOMOVE -bor $SWP_NOZORDER) } }
    )
    foreach ($step in $steps) {
        [void] (& $step.act)
        Start-Sleep -Milliseconds $SettleMs
        if ($process.HasExited) { throw ("the editor died during: {0}" -f $step.what) }
        if (-not [Win.Api]::IsWindow($window)) { throw ("the window vanished during: {0}" -f $step.what) }
        Write-Host ("  {0,-14} ok" -f $step.what)
    }

    # WM_CLOSE, the way the title bar's X asks: the editor saves its session and leaves.
    $out = [IntPtr]::Zero
    [void] [Win.Api]::SendMessageTimeout($window, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero, 0, 5000, [ref] $out)
    if (-not $process.WaitForExit(15000)) {
        $process.Kill()
        throw 'the editor did not exit within 15 s of being asked to close'
    }
    Write-Host ("  {0,-14} ok (exit code {1})" -f 'close', $process.ExitCode)
    if ($process.ExitCode -ne 0) { throw ("the editor exited with code {0}" -f $process.ExitCode) }
} finally {
    Remove-Item Env:\BITE_LOG_PATH -ErrorAction SilentlyContinue
}

if (Test-Path $log) {
    $bad = Select-String -Path $log -Pattern '\[(WARN|ERROR)\]' -ErrorAction SilentlyContinue
    if ($bad) {
        Write-Host "`nlogged while running:"
        $bad | ForEach-Object { Write-Host ("  " + $_.Line) }
        Remove-Item $log
        throw 'the editor logged warnings or errors'
    }
    Remove-Item $log
}
Write-Host "`nall window states survived, clean exit, nothing logged above INFO"
