# Desarrollo y compilación

[Índice](README.md) · [Contribuir](../CONTRIBUTING.md) · [Arquitectura](ARCHITECTURE.md)

El workspace tiene cuatro paquetes. `core` y `application` se prueban sin PDFium ni interfaz; `runtime` adapta motores y almacenamiento, y `crabpdf` compone el escritorio. La biblioteca nativa se carga al ejecutar: no es necesario descargarla para probar las bibliotecas internas.

## Requisitos comunes

- Rust **1.96.0**, fijado por [rust-toolchain.toml](../rust-toolchain.toml). Usa `--locked` para conservar Cargo.lock.
- Herramientas de compilación del sistema y un controlador OpenGL para ejecutar la interfaz.
- PDFium **build 7881** de la misma arquitectura del ejecutable para lectura y pruebas nativas.
- Conversores adicionales para formatos distintos de PDF.

Node.js 24 sólo se usa para la web y las comprobaciones documentales; no forma parte de la aplicación de escritorio.

## Comprobación rápida

Desde la raíz:

```sh
cargo test -p crabpdf-core -p crabpdf-application --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --lib --tests --locked
```

El último comando omite las pruebas marcadas `#[ignore]`. Para ejecutarlas, prepara sus bibliotecas y herramientas. No interpretes un resultado general verde como validación de los casos omitidos.

## Windows

Instala Visual Studio 2022 Build Tools con C++, MSVC, Windows SDK y CMake. Después:

```powershell
./scripts/Get-Pdfium.ps1 -Destination native
./scripts/Get-FormatTools.ps1
cargo run --locked -- tests/fixtures/reading-sample.pdf
```

Si la política local requiere otro lanzamiento de scripts, usa el procedimiento permitido por tu entorno. No se cambia la política del sistema desde el repositorio.

Para x86:

```powershell
rustup target add i686-pc-windows-msvc
./scripts/Get-Pdfium.ps1 -Architecture x86 -Destination native/win-x86
$env:RUST_READER_PDFIUM = (Resolve-Path native/win-x86/pdfium.dll).Path
cargo run --locked --target i686-pc-windows-msvc -- tests/fixtures/reading-sample.pdf
```

PDFium y el ejecutable deben coincidir en arquitectura. Los conversores Windows preparados por Get-FormatTools son x86 y se comparten entre ambas builds. El script verifica sus fuentes y genera un manifiesto; no modifica instalaciones globales.

`scripts/Test-Local.ps1 -Architecture both` ejecuta clippy, pruebas nativas, compilaciones release y recorridos visuales con datos aislados. `-DebugTests` ejecuta las mismas pruebas de contratos y motor sin la optimización LTO de release; las compilaciones de la aplicación y los recorridos visuales siguen usando release. `-SkipVisual` omite los recorridos y la compilación de sus ejemplos auxiliares. Para un cambio de interfaz, ejecuta además el recorrido visual pertinente o comprueba directamente la aplicación release.

## Ubuntu / Debian

Instala las bibliotecas de compilación:

```sh
sudo apt-get install build-essential pkg-config libx11-dev libxi-dev libxrandr-dev libxcursor-dev libxinerama-dev libgl1-mesa-dev libegl1-mesa-dev libwayland-dev libxkbcommon-dev libdbus-1-dev
cargo build --locked --release
```

Prepara PDFium para ejecutar. Si tienes PowerShell 7, el mismo script de CI sirve en Linux:

```sh
pwsh -File scripts/Get-Pdfium.ps1 -Platform linux -Architecture x64 -Destination native
cargo run --locked -- tests/fixtures/reading-sample.pdf
```

También puedes extraer manualmente `lib/libpdfium.so` del [paquete Linux x64 de build 7881](https://github.com/bblanchon/pdfium-binaries/releases/tag/chromium%2F7881), conservar sus licencias y establecer `RUST_READER_PDFIUM` a la biblioteca.

Para los importadores, [Build-LinuxFormatTools.sh](../scripts/Build-LinuxFormatTools.sh) compila MuPDF/libmobi desde los archivos fuente verificados de `native/format-downloads/`. `python3 scripts/Get-UnixFormatSources.py` los prepara desde el archivo de fuentes publicado y comprueba sus hashes. DjVuLibre y 7-Zip se instalan desde la distribución. El lector PDF no necesita iniciar estos conversores.

## macOS

Instala Rust y las Command Line Tools de Xcode. Prepara PDFium de tu arquitectura:

```sh
xcode-select --install
pwsh -File scripts/Get-Pdfium.ps1 -Platform mac -Architecture arm64 -Destination native
cargo run --locked -- tests/fixtures/reading-sample.pdf
```

El ejemplo de descarga usa PowerShell 7. En Intel usa `-Architecture x64`; también puedes extraer `lib/libpdfium.dylib` manualmente del paquete build 7881 y señalarlo con `RUST_READER_PDFIUM`. No se proporciona todavía un .app firmado o notarizado.

La compilación y las pruebas PDF de Intel/ARM están aprobadas en la [CI de 0.4.2](https://github.com/MHAlberto/crabpdf/actions/runs/40737537305). Eso no valida su interfaz ni sus importadores en un Mac físico. macOS queda fuera de los paquetes publicados en esta entrega.

## Pruebas nativas e interacción

Con PDFium preparado:

```sh
cargo test --locked --test native_pdf -- --ignored --test-threads=1
```

Con los conversores y muestras preparados:

```powershell
./scripts/New-FormatSamples.ps1
cargo test --locked --test formats_and_marks -- --ignored --test-threads=1
```

Elige el recorrido gráfico que cubra el cambio:

| Caso | Ejemplo |
| --- | --- |
| Selección, marcas y flechas mantenidas | `interaction_smoke` |
| Favoritos, pestañas y ventana | `document_chrome_smoke` |
| Transiciones y movimiento reducido | `transition_smoke` |
| Búsqueda y página larga | `search_smoke` |
| Posición entre procesos y escalas | `reading_session_smoke` |
| Importación y biblioteca | `formats_smoke` |

```sh
cargo run --locked --example interaction_smoke -- artifacts/interaction
cargo run --locked --example document_chrome_smoke -- artifacts/document-chrome
```

Los ejemplos necesitan un escritorio y usan perfiles aislados. Sus capturas no sustituyen una matriz física de monitores, GPU y compositores. En Windows, `scripts/Test-WindowGeometry.ps1` compara también cliente y monitor; una captura del framebuffer no detecta todos los recortes externos.

La batería Windows completa está en `scripts/Test-Local.ps1`. Ejecuta ambas arquitecturas, motores y recorridos; es más costosa que la comprobación rápida. Sus resultados se guardan en `artifacts/validation/`.

## Empaquetado y publicación

| Tarea | Herramienta |
| --- | --- |
| ZIP Windows | `scripts/Pack-Portable.ps1 -Architecture x64` o `x86` |
| Instalador Windows | `scripts/Build-Installer.ps1 -Architecture x64` o `x86` |
| Instalación / actualización / desinstalación | `scripts/Test-Installer.ps1` |
| Integridad de ZIP | `scripts/Verify-Portable.ps1` |
| TAR y DEB Linux | `scripts/Pack-Linux.sh` y `scripts/Verify-Linux.sh` |
| Instalación y capturas Ubuntu limpio | `scripts/Test-LinuxPackage.sh` dentro de Ubuntu 24.04 |
| Fuentes de conversores | `scripts/Pack-FormatSources.ps1` |
| Publicación GitHub | `scripts/Publish-Release.ps1` |

Las salidas quedan en `artifacts/packages/`. El empaquetador Linux toma el ELF de `artifacts/crabpdf-linux-x64`, PDFium/conversores de `native/linux-x64/` y licencias generadas con `scripts/Export-RustLicenses.py`. Consulta los requisitos de cada script antes de ejecutar.

El workflow manual [release-packages.yml](../.github/workflows/release-packages.yml), con `platforms: linux`, reproduce el empaquetado Linux. Compila en Debian 12 con glibc 2.36 e instala el DEB en un contenedor limpio Ubuntu 24.04. Antes de construir comprueba que las fuentes de la aplicación coinciden con la etiqueta solicitada. Los informes conservan también el commit de los scripts de empaquetado y el job que realmente pasó. La ruta experimental macOS no forma parte de los paquetes publicados en 0.4.2.

El publicador usa la versión del Cargo.toml si no se especifica `-Version`. Requiere fuentes compatibles con la etiqueta, paquetes verificados, notas y CI aprobada para ese commit. Crea un borrador, verifica tamaños y digests de los uploads y después lo publica. No modifica una entrega pública ya existente. Los instaladores actuales están sin firma; [Windows](WINDOWS_INSTALLER.md) documenta la firma real y sus límites.

## Recursos y documentación

El logo original vive en `assets/crabpdf-logo.svg`; build.rs genera los iconos nativos. Las capturas canónicas están en `docs/screenshots/`. La web copia esos recursos durante su build, sin duplicarlos en Git. [Web](WEBSITE.md).

Las dependencias y licencias no se retiran por parecer redundantes: forman parte de los paquetes. El pequeño puente JavaScript de MuPDF sigue siendo necesario; [su alcance](../crates/runtime/src/formats/mupdf/README.md) está documentado.

`artifacts/`, `native/`, `target/`, `dist/` y node_modules no se versionan. No borres resultados locales o preferencias de usuario como parte de una limpieza del código.
