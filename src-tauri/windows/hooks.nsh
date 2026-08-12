!macro NSIS_HOOK_POSTINSTALL
  ; The native SDK is linked by the launcher and must sit beside the executable.
  CopyFiles /SILENT "$INSTDIR\resources\discord\discord_partner_sdk.dll" "$INSTDIR"
  CopyFiles /SILENT "$INSTDIR\resources\discord\discord_krisp.dll" "$INSTDIR"
!macroend
