Unicode true
!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "FileFunc.nsh"
!include "WinVer.nsh"
!include "x64.nsh"
!include "Sections.nsh"
!define PRODUCT_KEY "Software\CrabPDF\Installations\${ARCH}"
!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\CrabPDF-${ARCH}"
!define OPENWITH_KEY "Software\Classes\CrabPDF.${ARCH}"
Name "CrabPDF ${VERSION} (${ARCH})"
OutFile "${OUTPUT}"
InstallDir "$LOCALAPPDATA\Programs\CrabPDF-${ARCH}"
InstallDirRegKey HKCU "${PRODUCT_KEY}" "InstallLocation"
RequestExecutionLevel user
ManifestSupportedOS Win10
ManifestDPIAware true
SetCompressor /SOLID lzma
SetCompressorDictSize 16
SetDatablockOptimize on
SetFont "Segoe UI" 9
BrandingText "CrabPDF · ${VERSION} · ${ARCH}"
ShowInstDetails show
ShowUninstDetails show
VIProductVersion "${VERSION}.0"
VIAddVersionKey /LANG=1033 "ProductName" "CrabPDF"
VIAddVersionKey /LANG=1033 "FileDescription" "CrabPDF installer (${ARCH})"
VIAddVersionKey /LANG=1033 "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=1033 "LegalCopyright" "CrabPDF contributors · MIT"
!define MUI_ICON "${BRANDING}\crabpdf.ico"
!define MUI_UNICON "${BRANDING}\crabpdf.ico"
!define MUI_HEADERIMAGE
!define MUI_HEADERIMAGE_RIGHT
!define MUI_HEADERIMAGE_BITMAP "${BRANDING}\installer-header.bmp"
!define MUI_WELCOMEFINISHPAGE_BITMAP "${BRANDING}\installer-sidebar.bmp"
!define MUI_UNWELCOMEFINISHPAGE_BITMAP "${BRANDING}\installer-sidebar.bmp"
!define MUI_TEXTCOLOR "19212D"
!define MUI_ABORTWARNING
!define MUI_WELCOMEPAGE_TITLE "$(WelcomeTitle)"
!define MUI_WELCOMEPAGE_TEXT "$(WelcomeText)"
!define MUI_FINISHPAGE_TITLE "$(FinishTitle)"
!define MUI_FINISHPAGE_TEXT "$(FinishText)"
!define MUI_FINISHPAGE_RUN "$INSTDIR\crabpdf.exe"
!define MUI_FINISHPAGE_RUN_NOTCHECKED
!define MUI_FINISHPAGE_RUN_TEXT "$(LaunchText)"
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "${LICENSE}"
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_WELCOME
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_UNPAGE_FINISH
!insertmacro MUI_LANGUAGE "Spanish"
!insertmacro MUI_LANGUAGE "English"
LangString WelcomeTitle ${LANG_SPANISH} "Tu próxima lectura empieza aquí"
LangString WelcomeTitle ${LANG_ENGLISH} "Your next read starts here"
LangString WelcomeText ${LANG_SPANISH} "Bienvenido a CrabPDF.$\r$\n$\r$\nUn lector de documentos ligero, local y sin cuentas.$\r$\n$\r$\nSe instalará solo para tu usuario. Puedes elegir los accesos directos y la carpeta.$\r$\n$\r$\n${VERSION} · En desarrollo · Windows ${ARCH}$\r$\n${SIGNATURE_LABEL}"
LangString WelcomeText ${LANG_ENGLISH} "Welcome to CrabPDF.$\r$\n$\r$\nA lightweight document reader. Local, with no accounts.$\r$\n$\r$\nInstalls for your user only. Choose your shortcuts and destination.$\r$\n$\r$\n${VERSION} · Development · Windows ${ARCH}$\r$\n${SIGNATURE_LABEL}"
LangString FinishTitle ${LANG_SPANISH} "Todo listo para leer"
LangString FinishTitle ${LANG_ENGLISH} "Ready to read"
LangString FinishText ${LANG_SPANISH} "CrabPDF ya está instalado.$\r$\n$\r$\nTus preferencias y marcas se guardan por separado y se conservarán al desinstalar.$\r$\n$\r$\nF11 activa la pantalla completa.$\r$\nCtrl+O abre un documento."
LangString FinishText ${LANG_ENGLISH} "CrabPDF is installed.$\r$\n$\r$\nPreferences and annotations are stored separately and preserved when uninstalling.$\r$\n$\r$\nF11 enters fullscreen.$\r$\nCtrl+O opens a document."
LangString LaunchText ${LANG_SPANISH} "Abrir CrabPDF"
LangString LaunchText ${LANG_ENGLISH} "Open CrabPDF"
LangString ReaderName ${LANG_SPANISH} "Lector y formatos compatibles (obligatorio)"
LangString ReaderName ${LANG_ENGLISH} "Reader and supported formats (required)"
LangString MenuName ${LANG_SPANISH} "Acceso en el menú Inicio"
LangString MenuName ${LANG_ENGLISH} "Start menu shortcut"
LangString DesktopName ${LANG_SPANISH} "Acceso en el escritorio"
LangString DesktopName ${LANG_ENGLISH} "Desktop shortcut"
LangString OpenWithName ${LANG_SPANISH} "Mostrar en Abrir con para archivos PDF"
LangString OpenWithName ${LANG_ENGLISH} "Show in Open with for PDF files"
LangString UnsafePath ${LANG_SPANISH} "Elige una carpeta nueva o una instalación CrabPDF de la misma arquitectura."
LangString UnsafePath ${LANG_ENGLISH} "Choose a new folder or a CrabPDF installation of the same architecture."
LangString CloseReader ${LANG_SPANISH} "Cierra CrabPDF antes de actualizar o desinstalar y vuelve a intentarlo."
LangString CloseReader ${LANG_ENGLISH} "Close CrabPDF before updating or uninstalling, then try again."
!ifdef SIGN_THUMBPRINT
!uninstfinalize '"${POWERSHELL}" -NoProfile -ExecutionPolicy Bypass -File "${SIGN_SCRIPT}" -CertificateThumbprint "${SIGN_THUMBPRINT}" -Path "%1"' = 0
!endif

; Sections precede callbacks so their numeric identifiers are defined.
Section "$(ReaderName)" READER
  SectionIn RO
  Call ValidateDestination
  IfFileExists "$INSTDIR\crabpdf.exe" 0 files
  ClearErrors
  FileOpen $0 "$INSTDIR\crabpdf.exe" a
  ${If} ${Errors}
    MessageBox MB_OK|MB_ICONSTOP "$(CloseReader)" /SD IDOK
    SetErrorLevel 4
    Abort
  ${EndIf}
  FileClose $0
  files:
  SetOverwrite on
  !include "${INCLUDE_DIR}\install-files.nsh"
  SetOutPath "$INSTDIR"
  WriteINIStr "$INSTDIR\crabpdf-install.ini" "CrabPDF" "Product" "CrabPDF"
  WriteINIStr "$INSTDIR\crabpdf-install.ini" "CrabPDF" "Architecture" "${ARCH}"
  WriteINIStr "$INSTDIR\crabpdf-install.ini" "CrabPDF" "Version" "${VERSION}"
  WriteUninstaller "$INSTDIR\uninstall.exe"
  WriteRegStr HKCU "${PRODUCT_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayName" "CrabPDF (${ARCH})"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "Publisher" "CrabPDF contributors"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayIcon" "$INSTDIR\crabpdf.exe"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKCU "${UNINSTALL_KEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegStr HKCU "${UNINSTALL_KEY}" "URLInfoAbout" "https://github.com/MHAlberto/crabpdf"
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoRepair" 1
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "EstimatedSize" ${INSTALLED_KIB}
SectionEnd
Section "$(MenuName)" MENU
  CreateDirectory "$SMPROGRAMS\CrabPDF (${ARCH})"
  CreateShortcut "$SMPROGRAMS\CrabPDF (${ARCH})\CrabPDF.lnk" "$INSTDIR\crabpdf.exe"
  WriteINIStr "$INSTDIR\crabpdf-install.ini" "CrabPDF" "Menu" "1"
SectionEnd
Section /o "$(DesktopName)" DESKTOP
  CreateShortcut "$DESKTOP\CrabPDF (${ARCH}).lnk" "$INSTDIR\crabpdf.exe"
  WriteINIStr "$INSTDIR\crabpdf-install.ini" "CrabPDF" "Desktop" "1"
SectionEnd
Section /o "$(OpenWithName)" OPENWITH
  WriteRegStr HKCU "${OPENWITH_KEY}" "" "CrabPDF PDF"
  WriteRegStr HKCU "${OPENWITH_KEY}\DefaultIcon" "" "$INSTDIR\crabpdf.exe,0"
  WriteRegStr HKCU "${OPENWITH_KEY}\shell\open\command" "" '"$INSTDIR\crabpdf.exe" "%1"'
  WriteRegStr HKCU "Software\Classes\.pdf\OpenWithProgids" "CrabPDF.${ARCH}" ""
  WriteINIStr "$INSTDIR\crabpdf-install.ini" "CrabPDF" "OpenWith" "1"
SectionEnd
!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
!insertmacro MUI_DESCRIPTION_TEXT ${READER} "PDF, EPUB, cómics y otros formatos. Incluye motor, herramientas y licencias."
!insertmacro MUI_DESCRIPTION_TEXT ${MENU} "Abre CrabPDF desde Inicio."
!insertmacro MUI_DESCRIPTION_TEXT ${DESKTOP} "Crea un acceso directo para tu usuario."
!insertmacro MUI_DESCRIPTION_TEXT ${OPENWITH} "Añade una opción sin cambiar tu lector predeterminado."
!insertmacro MUI_FUNCTION_DESCRIPTION_END

Function .onInit
  SetShellVarContext current
  SetRegView 32
  ${IfNot} ${AtLeastWin10}
    MessageBox MB_OK|MB_ICONSTOP "CrabPDF necesita Windows 10 o posterior." /SD IDOK
    SetErrorLevel 2
    Abort
  ${EndIf}
!if "${ARCH}" == "x64"
  ${IfNot} ${RunningX64}
    MessageBox MB_OK|MB_ICONSTOP "Usa el instalador x86 en Windows de 32 bits." /SD IDOK
    SetErrorLevel 2
    Abort
  ${EndIf}
!endif
  ; Automated checks avoid user shortcuts. No default-file association is changed.
  ${GetParameters} $0
  ClearErrors
  ${GetOptions} $0 "/NOINTEGRATION" $1
  ${IfNot} ${Errors}
    SectionSetFlags ${MENU} 0
    SectionSetFlags ${DESKTOP} 0
    SectionSetFlags ${OPENWITH} 0
  ${EndIf}
FunctionEnd

Function .onVerifyInstDir
  System::Call 'kernel32::GetFullPathNameW(w "$INSTDIR", i ${NSIS_MAX_STRLEN}, w .r2, p 0) i .r3'
  ${If} $3 == 0
  ${OrIf} $3 >= ${NSIS_MAX_STRLEN}
    SetErrorLevel 3
    Abort
  ${EndIf}
  StrCpy $INSTDIR $2
  IfFileExists "$INSTDIR\." 0 valid
  ReadINIStr $0 "$INSTDIR\crabpdf-install.ini" "CrabPDF" "Architecture"
  ReadINIStr $1 "$INSTDIR\crabpdf-install.ini" "CrabPDF" "Product"
  ${If} $0 != "${ARCH}"
  ${OrIf} $1 != "CrabPDF"
    Abort
  ${EndIf}
  valid:
FunctionEnd

Function ValidateDestination
  System::Call 'kernel32::GetFullPathNameW(w "$INSTDIR", i ${NSIS_MAX_STRLEN}, w .r2, p 0) i .r3'
  ${If} $3 == 0
  ${OrIf} $3 >= ${NSIS_MAX_STRLEN}
    SetErrorLevel 3
    Abort
  ${EndIf}
  StrCpy $INSTDIR $2
  ${GetRoot} "$INSTDIR" $0
  ${If} $INSTDIR == ""
  ${OrIf} $INSTDIR == "$0\"
    SetErrorLevel 3
    Abort
  ${EndIf}
  IfFileExists "$INSTDIR\." 0 valid
  ReadINIStr $0 "$INSTDIR\crabpdf-install.ini" "CrabPDF" "Architecture"
  ReadINIStr $1 "$INSTDIR\crabpdf-install.ini" "CrabPDF" "Product"
  ${If} $0 != "${ARCH}"
  ${OrIf} $1 != "CrabPDF"
    MessageBox MB_OK|MB_ICONSTOP "$(UnsafePath)" /SD IDOK
    SetErrorLevel 3
    Abort
  ${EndIf}
  valid:
FunctionEnd

Function un.onInit
  SetShellVarContext current
  SetRegView 32
  ReadINIStr $0 "$INSTDIR\crabpdf-install.ini" "CrabPDF" "Product"
  ReadINIStr $1 "$INSTDIR\crabpdf-install.ini" "CrabPDF" "Architecture"
  ${If} $0 != "CrabPDF"
  ${OrIf} $1 != "${ARCH}"
    SetErrorLevel 3
    Abort
  ${EndIf}
  IfFileExists "$INSTDIR\crabpdf.exe" 0 unlocked
  ClearErrors
  FileOpen $0 "$INSTDIR\crabpdf.exe" a
  ${If} ${Errors}
    MessageBox MB_OK|MB_ICONSTOP "$(CloseReader)" /SD IDOK
    SetErrorLevel 4
    Abort
  ${EndIf}
  FileClose $0
  unlocked:
FunctionEnd

Section "Uninstall"
  ; No recursive deletion: only this build's manifest and owned integration.
  !include "${INCLUDE_DIR}\uninstall-files.nsh"
  ReadINIStr $0 "$INSTDIR\crabpdf-install.ini" "CrabPDF" "Menu"
  ${If} $0 == "1"
    Delete "$SMPROGRAMS\CrabPDF (${ARCH})\CrabPDF.lnk"
    RMDir "$SMPROGRAMS\CrabPDF (${ARCH})"
  ${EndIf}
  ReadINIStr $0 "$INSTDIR\crabpdf-install.ini" "CrabPDF" "Desktop"
  ${If} $0 == "1"
    Delete "$DESKTOP\CrabPDF (${ARCH}).lnk"
  ${EndIf}
  ReadRegStr $0 HKCU "${OPENWITH_KEY}\shell\open\command" ""
  ${If} $0 == '"$INSTDIR\crabpdf.exe" "%1"'
    DeleteRegValue HKCU "Software\Classes\.pdf\OpenWithProgids" "CrabPDF.${ARCH}"
    DeleteRegKey HKCU "${OPENWITH_KEY}"
  ${EndIf}
  Delete "$INSTDIR\crabpdf-install.ini"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
  ReadRegStr $0 HKCU "${PRODUCT_KEY}" "InstallLocation"
  ${If} $0 == "$INSTDIR"
    DeleteRegKey HKCU "${PRODUCT_KEY}"
    DeleteRegKey HKCU "${UNINSTALL_KEY}"
  ${EndIf}
SectionEnd
