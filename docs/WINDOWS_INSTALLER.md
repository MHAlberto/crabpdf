# Instalador de Windows

La entrega pública actual es [0.4.1](https://github.com/MHAlberto/crabpdf/releases/tag/v0.4.1), de desarrollo. [Índice](README.md) · [Desarrollo](DEVELOPMENT.md)

## Comportamiento

El instalador EXE usa NSIS Modern UI 2, el logo original y los iconos generados por build.rs. Está disponible en x64 y x86. Instala por usuario en `%LOCALAPPDATA%/Programs/CrabPDF-x64` o `CrabPDF-x86`, sin pedir privilegios de administrador.

Ofrece acceso en Inicio, acceso de escritorio opcional y una entrada Abrir con para PDF. No cambia el lector predeterminado. El lanzamiento al finalizar es opcional y empieza desmarcado.

El payload contiene el mismo lector, PDFium, conversores y licencias que el ZIP portable, más los avisos de NSIS. No descarga componentes durante la instalación. El desinstalador conserva preferencias, marcas y archivos que no pertenecen a la distribución.

Una actualización exige identificar una instalación de la misma arquitectura. Se rechaza una carpeta ajena o el ejecutable bloqueado, sin cerrar la aplicación por la fuerza. Los directorios sólo se eliminan si están vacíos; no se utiliza borrado recursivo.

## Construir y comprobar

Prepara el entorno descrito en Desarrollo:

```powershell
./scripts/Build-Installer.ps1 -Architecture x64
./scripts/Build-Installer.ps1 -Architecture x86
./scripts/Test-Installer.ps1 -Architecture x64 -Installer artifacts/packages/crabpdf-0.4.1-win-x64-setup.exe
./scripts/Verify-Portable.ps1
```

La última ruta es un ejemplo para 0.4.1; los constructores obtienen su versión de Cargo. `-SkipBuild` reutiliza una compilación existente y `-PortableDirectory` un paquete previamente generado.

Build-Installer verifica versión, arquitectura, ejecutable y hashes antes de crear un payload separado. Get-NSIS prepara NSIS 3.12 con tamaño y SHA-256 fijados, sin instalar herramientas globales. La CI conserva EXE, ZIP, hashes e informes de instalación.

Test-Installer usa una carpeta aislada y asociaciones desactivadas. Si detecta una instalación registrada, se detiene para no modificarla. Comprueba instalación, actualización, archivos bloqueados, desinstalación y conservación de archivos ajenos.

## Firma y evidencia

Los instaladores 0.4.1 tienen estado **NotSigned**. No hay un certificado de firma disponible ni se declara una certificación independiente de seguridad.

Para firmar se necesita una identidad válida de firma de código; `scripts/Sign-Windows.ps1` documenta los parámetros. El publicador admite exigir firma con `-RequireSignedInstaller`. No se sustituyen certificados reales por certificados que simulen confianza.

El [registro de publicación](releases/v0.4.1-publication.json) conserva tamaños, hashes, commit de origen y validación CI de ambas arquitecturas. El [informe 0.4.0](history/RELEASE_0_4_0.md) conserva el ensayo anterior de actualización entre versiones.

La revisión manual completa del asistente, idiomas, DPI, accesos opcionales, Windows 11 y Windows x86 nativo sigue pendiente. El ensayo silencioso no equivale a revisar la interfaz del instalador. La migración de archivos retirados entre inventarios de versiones también necesita trabajo explícito.
