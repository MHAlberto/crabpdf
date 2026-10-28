# CrabPDF 0.3.3: paquete Linux x64 y validación de Ubuntu

> Documento histórico. Describe una revisión anterior. Consulta el [índice actual](../README.md) para las instrucciones vigentes.


**Fecha:** 2026-10-15. Se mantuvo la versión Rust 0.3.3 y se añadieron paquetes Linux a la entrega Windows de la misma versión. No se declara paridad 1.0.

## Construcción

El ejecutable se compiló con Rust 1.96.0 en `rust:1.96.0-bookworm` (Linux x86_64, glibc 2.36), con `cargo build --locked --release`. PDFium build 7881 para Linux x64 se descargó con `scripts/Get-Pdfium.ps1` y su SHA-256 se cotejó con el asset publicado. MuPDF 1.28.5 y libmobi 0.12 se compilaron desde las fuentes fijadas en `scripts/Build-LinuxFormatTools.sh`; el empaquetador comprueba sus hashes y ELF x86_64. `python3 scripts/Export-RustLicenses.py` exportó los textos de Cargo para Linux y `bash scripts/Pack-Linux.sh` produjo los dos paquetes.

| Archivo | Bytes | SHA-256 |
| --- | ---: | --- |
| `crabpdf-0.3.3-linux-x64.tar.gz` | 40 574 019 | `752722b08118b7fed69a4010fecb6e9803d5633d2273d60656a3073bad5a6a2d` |
| `crabpdf-0.3.3-ubuntu24.04-amd64.deb` | 30 598 048 | `8c7e5f1c2f60185f4f76678c9e63caa776e3f866856f4132adbcbc582b338584` |

El tar y el `.deb` incluyen CrabPDF, PDFium, MuPDF, libmobi, logo, licencias y `SHA256SUMS`. `scripts/Verify-Linux.sh` comprobó **408 archivos** y la identidad del ejecutable en cada paquete. El `.deb` instala DjVuLibre y 7-Zip desde APT junto con las bibliotecas gráficas y OpenSSL 3; el tar requiere esos programas del sistema. Las fuentes de MuPDF/libmobi están en el archivo separado `crabpdf-0.2.0-format-tools-sources.zip`, sin cambios desde la entrega 0.2.0.

## Pruebas ejecutadas

- En Debian Bookworm x86_64 pasaron `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked --release` y las siete pruebas ignoradas de `native_pdf` con PDFium real. La suite de formatos con **conversores Debian 12 del sistema** falló en dos de siete casos: MuPDF 1.21 no conservó el índice EPUB ni convirtió texto/Markdown y libmobi 0.11 aceptó un Kindle cifrado. Estos resultados motivaron el empaquetado de MuPDF 1.28.5 y libmobi 0.12; no se trasladaron como pruebas aprobadas.
- En un contenedor **Ubuntu 24.04 limpio**, `apt` instaló el `.deb`, se comprobó `crabpdf --version` y se verificó el `SHA256SUMS` interno. Con las herramientas empaquetadas, `formats_and_marks --ignored --test-threads=1` aprobó **7/7** pruebas: EPUB, Kindle y DRM, DjVu, cómics, texto/Markdown, persistencia y exportación de marcas. Log (`artifacts/validation-linux-20261015/final-ubuntu-formats.log`, registro local no versionado).
- Con Xvfb y render OpenGL por software, `visual_smoke` recorrió la ventana real y guardó **25 capturas**, incluidas lectura, modo continuo, búsqueda, paleta, selección y pestañas. La prueba terminó con código 0 y se inspeccionaron las capturas de lectura normal y continua. Log (`artifacts/validation-linux-20261015/final-ubuntu-visual.log`, registro local no versionado) y capturas (`artifacts/validation-linux-20261015/final-ui`, registro local no versionado).
- Tras actualizar la documentación dentro de los paquetes, se añadió Mesa a las dependencias declaradas del `.deb` para proporcionar el render por software usado en la prueba. Los binarios permanecen idénticos. En otro Ubuntu 24.04 limpio se instaló el `.deb` definitivo con APT y solo Xvfb como entorno de prueba adicional: el ejecutable instalado abrió su ventana durante ocho segundos sin errores de bibliotecas. `scripts/Verify-Linux.sh` aprobó los 408 hashes internos de cada archivo final. El comando y perfil aislado se conservaron en `artifacts/validation-linux-20261015/`.

## Límites

La ventana se ejecutó en X11 virtual con Xvfb, sin prueba en Wayland, GPU física, impresora, selector nativo, accesibilidad ni otras distribuciones. El tar usa glibc 2.36 como base de compilación, pero no se certifica en cualquier distro con esa versión: requiere bibliotecas gráficas, OpenSSL 3, DjVuLibre y 7-Zip compatibles. La extracción nativa de PDFium, MuPDF y libmobi mantiene componentes C/C++; no se reescribieron en Rust. El inventario de licencias Linux tiene 213 dependencias resueltas, de las cuales 12 no traen archivo de licencia en la copia Cargo; se incluyen los textos upstream disponibles y queda revisión final antes de una versión estable. Windows 10 x64/x86 conserva su [validación de 0.3.3](IMPLEMENTATION_PROGRESS_0_3_3.md).
