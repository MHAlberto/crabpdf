# Dependencias y distribución

El código propio de CrabPDF y sus PDFs de ejemplo se distribuyen bajo la licencia MIT de `LICENSE`. Se desarrolló independientemente. No se copiaron código, iconos, fuentes ni recursos de SumatraPDF. La referencia de funcionalidades es [SumatraPDF](https://github.com/sumatrapdfreader/sumatrapdf), cuyo [COPYING](https://github.com/sumatrapdfreader/sumatrapdf/blob/master/COPYING) contiene GPL v3.

## Dependencias directas

Las versiones exactas resueltas están en `Cargo.lock`.

| Dependencia | Uso | Licencia declarada |
| --- | --- | --- |
| eframe 0.36.2 / egui 0.36.2 | Ventana, interfaz y render gráfico | MIT OR Apache-2.0 |
| pdfium-render 0.9.4 | Bindings y API de PDFium | MIT OR Apache-2.0 |
| rfd 0.17.2 | Selector de archivos nativo | MIT |
| serde 1.0.229 | Serialización de preferencias | MIT OR Apache-2.0 |
| toml 1.1.8 | Preferencias TOML | MIT OR Apache-2.0 |
| image 0.25.10 | Iconos, portadas PNG y capturas de pruebas; se reutiliza la dependencia de eframe | MIT OR Apache-2.0 |
| resvg 0.48.1 (compilación) | Convertir el SVG aportado por el usuario en iconos nativos | MIT OR Apache-2.0 |
| embed-resource 3.0.11 (compilación) | Incrustar iconos y propiedades del ejecutable Windows | MIT |
| roxmltree 0.21.1 | Leer la capa de texto XML de DjVu | MIT OR Apache-2.0 |
| serde_json 1.0.154 | Metadatos e índice de documentos preparados | MIT OR Apache-2.0 |

`docs/dependencies.csv` registra nombres, versiones, expresiones de licencia y repositorios de todas las dependencias devueltas por `cargo metadata`, incluidas herramientas de compilación, desarrollo y targets distintos. No todas se enlazan con el ejecutable. Regenerar desde la raíz con:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/Export-Licenses.ps1
```

El inventario es metadata declarada, no sustituye los textos de licencia ni los avisos de los archivos incluidos. `epaint_default_fonts` declara `(MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0`: se conservan también OFL, UFL, Hack y el aviso MIT de emoji-icon-font del paquete original.

## PDFium nativo

Se utiliza [PDFium build 7881 de bblanchon/pdfium-binaries](https://github.com/bblanchon/pdfium-binaries/releases/tag/chromium%2F7881), variante sin V8/XFA. `scripts/Get-Pdfium.ps1` verifica el SHA-256 publicado en la API de GitHub y conserva el paquete completo, su `LICENSE` y la carpeta `licenses/`. La licencia del empaquetador es MIT. PDFium tiene su propio aviso BSD, disponible en `licenses/pdfium.txt`; los componentes nativos tienen licencias separadas.

El paquete Windows consultado incluye avisos de PDFium, Abseil, AGG, fast_float, FreeType, ICU, LCMS, libjpeg-turbo, OpenJPEG, libpng, libtiff, LLVM libc, simdutf y zlib. Los paquetes de otros sistemas pueden contener un conjunto diferente: conservar siempre todos los avisos del paquete concreto utilizado. El registro `native/pdfium-provenance.json` conserva origen, build y hash.

## Preparación de un paquete

Para una futura distribución, incluir `LICENSE` de CrabPDF, los textos y avisos de las dependencias Rust realmente incluidas y todos los avisos del PDFium correspondiente. Para dependencias bajo `MIT OR Apache-2.0`, documentar la opción elegida y conservar los avisos asociados; respetar también licencias de fuentes y otras expresiones presentes en el inventario. Herramientas como [cargo-about](https://github.com/EmbarkStudios/cargo-about) pueden generar el documento de atribución desde el lockfile, pero requieren revisar la selección y los recursos no descritos por Cargo.

`scripts/Pack-Portable.ps1` genera paquetes locales de desarrollo x64/x86 con los avisos originales de PDFium, textos encontrados en los paquetes de Cargo, fuentes y seis suplementos obtenidos de upstream en la revisión de `.cargo_vcs_info.json`. La procedencia de esos suplementos está en `licenses-upstream/sources.json`. Para las dependencias duales MIT/Apache de esos suplementos se conserva la opción MIT; gl_generator/khronos_api conservan Apache-2.0 y clipboard-win BSL-1.0. Las herramientas de conversión e incrustación del logo son dependencias de compilación y no se enlazan al lector.

El inventario de Cargo es un superconjunto de cada ejecutable. Su índice registra paquetes cuyos archivos publicados omiten textos de licencia; muchos son específicos de plataformas no compiladas aquí. La presencia de archivos no sustituye la revisión de recursos/avisos para una distribución pública. Se debe cerrar esa revisión al preparar macOS/Linux y la versión estable. El instalador Windows y sus avisos adicionales se describen en [WINDOWS_INSTALLER.md](WINDOWS_INSTALLER.md); las compilaciones actuales no llevan firma digital.

Las compilaciones Windows usan CRT estático para evitar depender de un redistribuible de VC++ instalado. Los imports de PDFium también se verificaron y sólo requieren DLL del sistema. `native/` está excluido de Git; quien obtenga el código debe descargar PDFium con el script o aportar una biblioteca compatible. No se aplica la licencia MIT del proyecto a las bibliotecas ajenas.

## Herramientas de formatos de 0.2

La aplicación Rust sigue usando PDFium para visualizar páginas. Los siguientes programas separados se distribuyen en `format-tools/` para preparar documentos no PDF; sus licencias no se reemplazan por la MIT de CrabPDF.

| Componente | Procedencia y licencia conservada |
| --- | --- |
| MuPDF / mutool 1.28.5 | [Fuentes oficiales de Artifex](https://github.com/ArtifexSoftware/mupdf-downloads/releases/tag/1.28.5), GNU AGPLv3 o posterior; contiene sus dependencias y fuentes tipográficas con avisos propios. Compilación Win32 Release con CRT estático, usando `FormatToolsStaticCRT.props`. |
| libmobi / mobitool 0.12 | [bfabiszewski/libmobi v0.12](https://github.com/bfabiszewski/libmobi/tree/v0.12), GNU LGPLv3 o posterior; se conservan LGPLv3 y GPLv3. Herramienta estática con miniz/xmlwriter internos y sin soporte de DRM. Se añade `win32/getopt.c` al target CMake de herramientas; el script reproduce ese cambio. |
| DjVuLibre 3.5.29 | [Distribución oficial Windows](https://sourceforge.net/projects/djvu/files/DjVuLibre_Windows/3.5.29%2B4.12/), GNU GPLv2 o posterior; se extraen sólo los programas de decodificación y DLL necesarias, sin instalar DjView. Fuente correspondiente en el archivo separado. |
| 7-Zip 26.04 | [ip7z/7zip 26.04](https://github.com/ip7z/7zip/releases/tag/26.04), LGPL y restricciones unRAR indicadas en `License.txt`; se incluye el texto original y la fuente. No se instala el programa en el sistema. |
| JPEG 6b, TIFF 4.0.10 y zlib 1.2.11 | DLL de la distribución DjVuLibre anterior; se conservan sus README/COPYRIGHT y archivos de fuente exactos. |
| Visual C++ CRT x86 14.44.35112 | `vcruntime140.dll` y `msvcp140.dll` de la carpeta redistribuible del SDK instalado; se usan sólo junto a las herramientas DjVu. [Lista de distribución de Visual Studio 2022](https://learn.microsoft.com/en-us/visualstudio/releases/2022/redistribution) y [documentación de distribución](https://learn.microsoft.com/en-us/cpp/windows/redistributing-visual-cpp-files). |

`native/format-tools/provenance.json` registra versiones, arquitectura y SHA-256 de cada archivo. `scripts/Get-FormatTools.ps1` fija hashes de descargas y opciones de compilación. Los paquetes de esta entrega se acompañan de `crabpdf-0.2.0-format-tools-sources.zip`, con las fuentes completas de las herramientas, bibliotecas JPEG/TIFF/zlib, textos y scripts/propiedades para reproducir las compilaciones. Los ayudantes pueden sustituirse por otra compilación compatible; la aplicación no obliga a usar sus hashes en ejecución.

El núcleo Rust, PDFium, mutool y mobitool no requieren un redistribuible instalado. DjVuLibre usa el CRT local incluido en su propia carpeta. En Windows 10/11, el UCRT es una biblioteca del sistema. Los términos de Microsoft se mantienen separados de las licencias de las herramientas abiertas.

## Instalador Windows

El asistente usa NSIS 3.12 sin modificaciones del motor, Modern UI 2 y plugins estándar. Su [licencia oficial](https://nsis.sourceforge.io/License) distingue zlib/libpng para el motor/recursos y CPL 1.0 con excepción de enlace para el módulo LZMA. El instalador conserva el texto original verificado del compilador en `installer-licenses/NSIS-COPYING.txt` y una referencia a la fuente exacta en `NSIS-NOTICE.txt`. Estos componentes pertenecen al instalador/desinstalador; no se enlazan con la aplicación Rust ni cambian su runtime.
