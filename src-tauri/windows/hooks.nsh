!macro NSIS_HOOK_PREINSTALL
  DetailPrint "Preparing Coldem and the Runner..."
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; The native SDK is linked by the launcher and must sit beside the executable.
  ; Tauri installs the mapped resources under $INSTDIR\discord, not resources\discord.
  ; Use explicit destination filenames so the Windows loader finds the SDK before
  ; the Rust application code starts.
  CopyFiles /SILENT "$INSTDIR\discord\discord_partner_sdk.dll" "$INSTDIR\discord_partner_sdk.dll"
  CopyFiles /SILENT "$INSTDIR\discord\discord_krisp.dll" "$INSTDIR\discord_krisp.dll"
  DetailPrint "Runner ready. Coldem is installed."
!macroend
