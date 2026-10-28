# CrabPDF

<p><img src="assets/crabpdf-logo.svg" width="88" height="88" alt="Logo de CrabPDF"></p>

Lector local de PDF, libros electrónicos y cómics, desarrollado en Rust con egui/eframe y PDFium. Sin cuentas ni telemetría.

[Web](https://mhalberto.github.io/crabpdf/) · [Descargas](https://github.com/MHAlberto/crabpdf/releases/tag/v0.4.1) · [Documentación](docs/README.md) · [Contribuir](CONTRIBUTING.md)

**Versión pública: 0.4.1, de desarrollo.** La meta de 1.0 es la paridad de funciones con la revisión fijada de SumatraPDF. Ese objetivo todavía no se ha alcanzado.

**En preparación: [0.4.2](docs/releases/v0.4.2.md)**, con reducción de memoria al navegar y una ventana de propiedades que aprovecha el ancho disponible. Las descargas públicas de esta página siguen correspondiendo a 0.4.1.

![CrabPDF con un documento de prueba abierto](docs/screenshots/light.png)

## Descargar

| Plataforma | Paquetes |
| --- | --- |
| Windows x64 | [Instalador EXE](https://github.com/MHAlberto/crabpdf/releases/download/v0.4.1/crabpdf-0.4.1-win-x64-setup.exe) · [ZIP portable](https://github.com/MHAlberto/crabpdf/releases/download/v0.4.1/crabpdf-0.4.1-win-x64.zip) |
| Windows x86 | [Instalador EXE](https://github.com/MHAlberto/crabpdf/releases/download/v0.4.1/crabpdf-0.4.1-win-x86-setup.exe) · [ZIP portable](https://github.com/MHAlberto/crabpdf/releases/download/v0.4.1/crabpdf-0.4.1-win-x86.zip) |
| Ubuntu 24.04 x64 | [Paquete DEB](https://github.com/MHAlberto/crabpdf/releases/download/v0.4.1/crabpdf-0.4.1-ubuntu24.04-amd64.deb) |
| Linux x64 | [TAR.GZ](https://github.com/MHAlberto/crabpdf/releases/download/v0.4.1/crabpdf-0.4.1-linux-x64.tar.gz), para glibc 2.36 o posterior |
| macOS Intel / Apple Silicon | Compilación y pruebas PDF en CI; todavía sin instalador público |

[Notas de 0.4.1](docs/releases/v0.4.1.md) · [SHA-256](https://github.com/MHAlberto/crabpdf/releases/download/v0.4.1/SHA256SUMS-v0.4.1.txt) · [Procedencia de los paquetes](docs/releases/v0.4.1-publication.json)

La interfaz se ha comprobado en Windows 10 x64 y Ubuntu 24.04 con escritorio virtual; x86 se prueba bajo WOW64. Windows 11 físico, Windows de 32 bits nativo, Wayland y otras distribuciones necesitan validación propia. Los instaladores y paquetes actuales **no tienen firma digital**.

En Ubuntu:

```sh
sudo apt install ./crabpdf-0.4.1-ubuntu24.04-amd64.deb
```

En Windows puedes usar el instalador por usuario o descomprimir el ZIP. Conserva `pdfium.dll` y `format-tools/` junto al ejecutable portable. El tar de Linux incluye sus requisitos en `INSTALL-LINUX.md`.

## Qué puedes hacer

- Leer PDF, EPUB, Kindle sin DRM, FictionBook, XPS, DjVu, CHM, cómics, imágenes y texto. [Extensiones y límites](docs/FORMATS_AND_ANNOTATIONS.md).
- Navegar en lectura continua, página única, páginas enfrentadas, modo libro o manga, con zoom y precarga.
- Seleccionar texto, buscar, resaltar y subrayar con distintos colores; exportar una copia PDF con las marcas.
- Abrir varios documentos en pestañas, separar una pestaña en otra ventana y retomar el punto de lectura.
- Organizar recientes y frecuentes con portadas y guardar favoritos por libro.
- Leer en pantalla completa, alternar temas o invertir los colores del documento con **F6**.

La impresión, la edición general de formularios/anotaciones, SyncTeX y la accesibilidad completa siguen pendientes. La selección actual se limita a una página. [Alcance hacia 1.0](docs/ROADMAP.md).

## Usar y desarrollar

La [guía de uso](docs/USAGE.md) reúne los atajos, parámetros CLI, preferencias y ubicación de los datos.

Para compilar necesitas Rust **1.96.0**, fijado en `rust-toolchain.toml`, y las herramientas del sistema indicadas en la [guía de desarrollo](docs/DEVELOPMENT.md). PDFium y los conversores son componentes nativos; el proyecto aún no tiene un motor íntegramente Rust.

Las bibliotecas internas se pueden comprobar sin ventana ni PDFium:

```sh
cargo test -p crabpdf-core -p crabpdf-application --locked
```

## Organización

```text
crates/core/          Tipos, geometría y punto de lectura
crates/application/   Reglas de lectura, perfil y protocolo
crates/runtime/       PDFium, importadores, cachés y almacenamiento
src/desktop/          Servicios nativos y composición
src/ui/               Interfaz egui
assets/               Identidad visual original
tests/                Contratos y documentos de prueba
docs/                 Guías, planes, evidencia e historial
```

[Arquitectura](docs/ARCHITECTURE.md) · [Rendimiento y límites de las mediciones](docs/PERFORMANCE.md) · [Pruebas](docs/VALIDATION.md) · [Plan de trabajo](docs/V1_PLAN.md)

## Licencias

El código de CrabPDF se distribuye bajo [MIT](LICENSE). PDFium, los conversores y las demás dependencias conservan sus licencias y avisos: [componentes de terceros](docs/THIRD_PARTY_NOTICES.md). Las entregas incluyen los textos correspondientes y las fuentes de los conversores.
