# Instalador de Windows

26 de octubre de 2026. Compilación de desarrollo 0.3.6; todavía no es 1.0.

## Experiencia e instalación

El nuevo `crabpdf-<versión>-win-x64-setup.exe` usa un asistente nativo NSIS Modern UI 2, con el logo original, icono multirresolución, panel azul oscuro, páginas de bienvenida/finalización y tipografía Segoe UI. También se genera x86. El arte se produce desde el SVG mediante el resvg que ya usa la compilación; no añade un navegador, servicio o dependencia a la aplicación.

Instala para el usuario actual en `%LOCALAPPDATA%/Programs/CrabPDF-x64` o `CrabPDF-x86`, con entrada en Aplicaciones instaladas. No solicita privilegios de administrador. Ofrece acceso en Inicio, escritorio opcional y una opción «Abrir con» opcional para PDF; no modifica el lector predeterminado. El lanzamiento al finalizar es opcional y empieza desmarcado.

Incluye el mismo ejecutable, PDFium, conversores, manifiesto y licencias del portátil. No descarga componentes durante la instalación. Las preferencias y marcas se conservan en sus ubicaciones de datos; el desinstalador no las borra.

Una carpeta existente debe tener la identificación de una instalación CrabPDF de la misma arquitectura. Las actualizaciones reemplazan los archivos de esa distribución; se rechaza el ejecutable bloqueado sin cerrar la aplicación por la fuerza. La desinstalación usa una lista explícita de archivos y elimina directorios sólo si quedan vacíos. No utiliza borrado recursivo. Los nombres retirados en futuras versiones necesitarán su migración explícita: no se declara resuelta todavía una actualización entre versiones con inventarios diferentes.

## Compilar y comprobar

Preparar Rust/MSVC, PDFium y herramientas de formatos conforme al README. Después:

```powershell
./scripts/Build-Installer.ps1 -Architecture x64
./scripts/Build-Installer.ps1 -Architecture x86
./scripts/Test-Installer.ps1 -Architecture x64 -Installer artifacts/packages/crabpdf-0.3.6-win-x64-setup.exe
./scripts/Verify-Portable.ps1
```

`Build-Installer.ps1` comprueba arquitectura/versión, ejecutable actual y hashes de todos los archivos del portátil antes de construir un payload independiente. `-SkipBuild` reutiliza una compilación ya hecha; `-PortableDirectory` permite reutilizar un paquete verificado. Los destinos quedan dentro del proyecto. La selección del paquete Cargo se hace por nombre, preparándolo para A04.

`Get-NSIS.ps1` descarga NSIS 3.12 dentro de `native/installer-tools`, verifica SHA-256 y tamaño del ZIP y restaura el compilador desde ese archivo antes de ejecutarlo. El checksum está fijado y documentado en el script a partir del [Portfile de MacPorts](https://github.com/macports/macports-ports/blob/master/devel/nsis/Portfile). No instala herramientas globalmente.

La CI Windows construye ambas arquitecturas, ejecuta la comprobación de instalación y sube EXE, ZIP, hashes e informes como artefactos del workflow. Linux conserva su flujo existente. El publicador incorpora los instaladores para una futura etiqueta nueva y sigue rechazando fuentes distintas del tag y reemplazos de releases públicos existentes.

## Firma e identidad del editor

**Estos instaladores no tienen firma digital: el usuario todavía no dispone de certificado. No se afirma que estén certificados como seguros.** El asistente y el informe JSON identifican ese estado. El SHA-256 permite comprobar integridad respecto de un valor obtenido de una fuente confiable; no sustituye una identidad Authenticode.

Se dejó preparada la firma del ejecutable propio, el desinstalador y el instalador, con certificado del almacén CurrentUser/My y sello RFC 3161 SHA-256:

```powershell
./scripts/Build-Installer.ps1 -Architecture x64 -CertificateThumbprint '<huella-real-de-40-hex>' -RequireSignature
./scripts/Publish-Release.ps1 -Version '<version-nueva>' -RequireSignedInstaller
```

`Sign-Windows.ps1` exige clave privada, vigencia y uso de firma de código, utiliza SignTool y verifica la cadena, la identidad elegida y el sello de tiempo. No genera certificados autofirmados ni modifica almacenes de confianza o protecciones de Windows. La ruta firmada no se ha ejecutado: requiere el certificado real y su proveedor de clave.

Según [Microsoft](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options), una firma acredita editor e integridad, pero archivos nuevos pueden conservar advertencias SmartScreen mientras acumulan reputación. Un certificado no es un análisis independiente de ausencia de malware. No se ha contratado un certificado ni publicado en Microsoft Store en esta entrega.

## Evidencia local

Los instaladores de esta revisión están en `artifacts/installer-a03b/packages/`:

| Arquitectura | Bytes del EXE instalador | Estado |
| --- | ---: | --- |
| x64 | 31 914 307 | NotSigned |
| x86 | 31 703 865 | NotSigned |

Ambos aprobaron instalación silenciosa en una ruta con espacios, 607 archivos verificados por manifiesto, actualización en sitio, rechazo de carpeta ajena y ejecutable bloqueado, desinstalación, limpieza del registro y conservación de un archivo ajeno. Los 608 archivos del payload incluyen el propio manifiesto; el desinstalador y el identificador de instalación se generan al instalar. Los ZIP se comprobaron separadamente.

Se revisó el bitmap con el logo. La revisión interactiva de las páginas, idiomas, DPI y accesos opcionales sigue pendiente: la sesión no expone `node_repl` para la habilidad de control de ventanas. La prueba silenciosa no se presenta como inspección visual del asistente. Windows 11, Windows de 32 bits nativo y antivirus independientes también requieren su propia evidencia.

El [informe versionado](architecture/evidence/a03b-windows-validation.json) contiene hashes y límites. No se sustituyó la publicación pública 0.3.6.
