Unicode true
!define APP "PosterCraft"
!define VER "1.0.0"
!define EXE "postercraft.exe"
!define UNREG "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP}"
!define STEPS 6

Name "${APP}"
OutFile "PosterCraft-Setup-${VER}.exe"
InstallDir "$PROGRAMFILES64\${APP}"
InstallDirRegKey HKLM "Software\${APP}" "Dir"
RequestExecutionLevel admin
SetCompressor /SOLID lzma

!include MUI2.nsh
!include nsDialogs.nsh
!include LogicLib.nsh

!define MUI_ICON "..\assets\icon.ico"
!define MUI_UNICON "..\assets\icon.ico"
!define MUI_ABORTWARNING
!define MUI_WELCOMEFINISHPAGE_BITMAP "welcome.bmp"
!define MUI_UNWELCOMEFINISHPAGE_BITMAP "welcome.bmp"
!define MUI_HEADERIMAGE
!define MUI_HEADERIMAGE_RIGHT
!define MUI_HEADERIMAGE_BITMAP "header.bmp"
!define MUI_WELCOMEPAGE_TITLE "Welcome to PosterCraft"
!define MUI_WELCOMEPAGE_TEXT "This will install PosterCraft on your computer.$\r$\n$\r$\nCrop, clean up and slice images across multiple sheets of paper, then export a PDF or print.$\r$\n$\r$\nClick Next to continue."
!define MUI_FINISHPAGE_RUN "$INSTDIR\${EXE}"
!define MUI_FINISHPAGE_RUN_TEXT "Launch PosterCraft"

Var Chk
Var Desk
Var Done

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
Page custom ShortcutPage ShortcutLeave
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Function ShortcutPage
  !insertmacro MUI_HEADER_TEXT "Desktop shortcut" "Choose how you would like to open PosterCraft."
  nsDialogs::Create 1018
  Pop $0
  ${NSD_CreateLabel} 0 8u 100% 24u "Would you like a PosterCraft shortcut on your desktop?"
  Pop $0
  ${NSD_CreateCheckbox} 0 40u 100% 12u "Yes, add a desktop shortcut"
  Pop $Chk
  ${NSD_Check} $Chk
  nsDialogs::Show
FunctionEnd

Function ShortcutLeave
  ${NSD_GetState} $Chk $Desk
FunctionEnd

; Updates the log line and the page header with a real step-based percentage.
!macro Step msg
  IntOp $Done $Done + 1
  IntOp $0 $Done * 100
  IntOp $0 $0 / ${STEPS}
  DetailPrint "${msg} - $0%"
  !insertmacro MUI_HEADER_TEXT "Installing PosterCraft" "$0% complete"
  Sleep 250
!macroend

Section "Install"
  SetShellVarContext all
  StrCpy $Done 0
  SetOutPath "$INSTDIR"
  !insertmacro Step "Preparing"
  File "..\target\release\${EXE}"
  !insertmacro Step "Copying PosterCraft"
  File "..\assets\icon.ico"
  !insertmacro Step "Copying graphics"
  CreateDirectory "$SMPROGRAMS\${APP}"
  CreateShortcut "$SMPROGRAMS\${APP}\${APP}.lnk" "$INSTDIR\${EXE}" "" "$INSTDIR\icon.ico"
  !insertmacro Step "Adding Start Menu shortcut"
  ${If} $Desk == ${BST_CHECKED}
    CreateShortcut "$DESKTOP\${APP}.lnk" "$INSTDIR\${EXE}" "" "$INSTDIR\icon.ico"
  ${EndIf}
  !insertmacro Step "Desktop shortcut"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  WriteRegStr HKLM "Software\${APP}" "Dir" "$INSTDIR"
  WriteRegStr HKLM "${UNREG}" "DisplayName" "${APP}"
  WriteRegStr HKLM "${UNREG}" "DisplayVersion" "${VER}"
  WriteRegStr HKLM "${UNREG}" "DisplayIcon" "$INSTDIR\icon.ico"
  WriteRegStr HKLM "${UNREG}" "UninstallString" "$\"$INSTDIR\Uninstall.exe$\""
  !insertmacro Step "Finishing"
SectionEnd

Section "Uninstall"
  SetShellVarContext all
  Delete "$DESKTOP\${APP}.lnk"
  Delete "$SMPROGRAMS\${APP}\${APP}.lnk"
  RMDir "$SMPROGRAMS\${APP}"
  Delete "$INSTDIR\${EXE}"
  Delete "$INSTDIR\icon.ico"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
  DeleteRegKey HKLM "${UNREG}"
  DeleteRegKey HKLM "Software\${APP}"
SectionEnd
