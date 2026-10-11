!macro NSIS_HOOK_POSTINSTALL
  ; Use a shipped standalone icon from the same real installation directory.
  IfFileExists "$DESKTOP\WinGlow.lnk" 0 +2
    CreateShortCut "$DESKTOP\WinGlow.lnk" "$INSTDIR\WinGlow.exe" "" "$INSTDIR\branding\WinGlow-desktop.ico" 0
!macroend
