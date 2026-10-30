; Frost Client Windows installer (NSIS). Build:  makensis /DVERSION=0.1.0 packaging\windows\frost.nsi
!ifndef VERSION
  !define VERSION "0.1.0"
!endif

!include "MUI2.nsh"

Name "Frost Client"
OutFile "FrostClient-Setup-${VERSION}.exe"
InstallDir "$LOCALAPPDATA\Programs\FrostClient"
InstallDirRegKey HKCU "Software\FrostClient" "InstallDir"
RequestExecutionLevel user
SetCompressor /SOLID lzma

!define MUI_ICON "frost.ico"
!define MUI_UNICON "frost.ico"
!define MUI_ABORTWARNING

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN "$INSTDIR\frost-launcher.exe"
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Section "Install"
  SetOutPath "$INSTDIR"
  File "..\..\target\release\frost-launcher.exe"
  File "frost.ico"
  WriteUninstaller "$INSTDIR\Uninstall.exe"

  CreateDirectory "$SMPROGRAMS\Frost Client"
  CreateShortcut "$SMPROGRAMS\Frost Client\Frost Client.lnk" "$INSTDIR\frost-launcher.exe" "" "$INSTDIR\frost.ico"
  CreateShortcut "$SMPROGRAMS\Frost Client\Uninstall.lnk" "$INSTDIR\Uninstall.exe"
  CreateShortcut "$DESKTOP\Frost Client.lnk" "$INSTDIR\frost-launcher.exe" "" "$INSTDIR\frost.ico"

  WriteRegStr HKCU "Software\FrostClient" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\FrostClient" "DisplayName" "Frost Client"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\FrostClient" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\FrostClient" "DisplayIcon" "$INSTDIR\frost.ico"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\FrostClient" "UninstallString" "$INSTDIR\Uninstall.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\FrostClient" "Publisher" "Frost Client"
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\frost-launcher.exe"
  Delete "$INSTDIR\frost.ico"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
  Delete "$DESKTOP\Frost Client.lnk"
  Delete "$SMPROGRAMS\Frost Client\Frost Client.lnk"
  Delete "$SMPROGRAMS\Frost Client\Uninstall.lnk"
  RMDir "$SMPROGRAMS\Frost Client"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\FrostClient"
  DeleteRegKey HKCU "Software\FrostClient"
  ; user data in %APPDATA%\FrostClient is intentionally kept
SectionEnd
