; Tauri NSIS installer hooks.

; Uninstalling removes the "Start with Windows" entry the app wrote, so no
; dead entry points at the deleted exe. The HKLM line clears entries from
; builds before 0.1.1 (it only succeeds when the uninstaller runs as admin).
!macro NSIS_HOOK_POSTUNINSTALL
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Nudgy"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run" "Nudgy"
  DeleteRegValue HKLM "Software\Microsoft\Windows\CurrentVersion\Run" "Nudgy"
  DeleteRegValue HKLM "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run" "Nudgy"
!macroend
