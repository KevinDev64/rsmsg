!define APP_NAME "rsmsg"
!ifndef VERSION
!define VERSION "0.1.0"
!endif
!ifndef DIST_DIR
!define DIST_DIR "dist\release\windows-x86_64-pc-windows-gnu"
!endif

Name "${APP_NAME} ${VERSION}"
OutFile "${DIST_DIR}\rsmsg-setup-${VERSION}-x86_64.exe"
InstallDir "$LOCALAPPDATA\rsmsg"
RequestExecutionLevel user

Page directory
Page instfiles
UninstPage uninstConfirm
UninstPage instfiles

Section "Install"
  SetOutPath "$INSTDIR"
  File "${DIST_DIR}\rsmsg.exe"
  File /nonfatal "${DIST_DIR}\*.dll"
  SetOutPath "$INSTDIR\locales"
  File /r "${DIST_DIR}\locales\*"
  SetOutPath "$INSTDIR"
  CreateShortcut "$DESKTOP\rsmsg.lnk" "$INSTDIR\rsmsg.exe"
  CreateDirectory "$SMPROGRAMS\rsmsg"
  CreateShortcut "$SMPROGRAMS\rsmsg\rsmsg.lnk" "$INSTDIR\rsmsg.exe"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
SectionEnd

Section "Uninstall"
  Delete "$DESKTOP\rsmsg.lnk"
  Delete "$SMPROGRAMS\rsmsg\rsmsg.lnk"
  RMDir "$SMPROGRAMS\rsmsg"
  RMDir /r "$INSTDIR"
SectionEnd
