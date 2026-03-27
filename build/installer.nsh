; Custom NSIS macros for imgplex installer
; Appends / broadcasts PATH changes so the CLI is usable from any terminal.

; Make the installer UI crisp on HiDPI / scaled displays.
; PerMonitorV2 = per-monitor DPI awareness (Windows 10 1703+).
ManifestDPIAwareness "PerMonitorV2"

; Show the details pane (file log) by default during installation.
; Without this, NSIS hides the file list and only shows the progress bar.
ShowInstDetails show

!macro customInstall
  ; Show individual file names + progress bar for our custom steps below.
  SetDetailsPrint both
  ; Append install directory to the current user's PATH (HKCU\Environment)
  DetailPrint "Adding $INSTDIR to user PATH..."
  ReadRegStr $0 HKCU "Environment" "Path"
  ${If} $0 != ""
    WriteRegExpandStr HKCU "Environment" "Path" "$0;$INSTDIR"
  ${Else}
    WriteRegExpandStr HKCU "Environment" "Path" "$INSTDIR"
  ${EndIf}
  DetailPrint "Broadcasting environment change to running processes..."
  ; Notify running processes (Explorer, open terminals) without requiring a reboot
  SendMessage ${HWND_BROADCAST} ${WM_WININICHANGE} 0 "STR:Environment" /TIMEOUT=5000
  DetailPrint "PATH update complete."
!macroend

!macro customUninstall
  ; Broadcast so the system knows env has changed.
  ; Note: the PATH entry is not automatically removed — the user can clean it
  ; up via System > Advanced > Environment Variables if desired.
  SendMessage ${HWND_BROADCAST} ${WM_WININICHANGE} 0 "STR:Environment" /TIMEOUT=5000
!macroend
