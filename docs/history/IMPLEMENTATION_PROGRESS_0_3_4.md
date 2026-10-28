# CrabPDF 0.3.4 — recuperar el punto de lectura

> Documento histórico. Describe una revisión anterior. Consulta el [índice actual](../README.md) para las instrucciones vigentes.


**Fecha:** 15 de octubre de 2026. Esta entrega avanza parcialmente R10 del [plan V1](../V1_PLAN.md), sobre la base de [0.3.3](IMPLEMENTATION_PROGRESS_0_3_3.md) y su [paquete Linux](IMPLEMENTATION_PROGRESS_0_3_3_LINUX.md). No declara paridad 1.0.

## Cambios

- Al cerrar y reabrir un documento se conserva el punto relativo a la página, además de modo, zoom, rotación y colores. El modo manga también se guarda. La restauración opcional de sesión utiliza los mismos datos.
- El ancla conserva un punto de página y su posición relativa en el área visible. Así puede recuperar la región con otra ventana o escala lógica. Si el centro queda entre páginas, se usa el borde más cercano y su posición visible para evitar un salto.
- La restauración espera las dimensiones reales de la página solicitada; no usa su altura estimada. La navegación, búsqueda, zoom o interacción explícita con el documento cancelan el ancla pendiente. Los parámetros de vista explícitos de la CLI prevalecen sobre ella.
- El desplazamiento programa el guardado diferido de preferencias con 500 ms de espera; el hilo de disco existente realiza la escritura. El cierre espera ese hilo.
- Las preferencias anteriores siguen cargando. Las anclas tienen esquema versionado y coordenadas enteras normalizadas; un esquema desconocido, coordenadas fuera de rango o página inexistente utilizan la página guardada como alternativa.

`src/reading_position.rs` mantiene este contrato independiente de egui/PDFium. No cambia el framework, los motores, las dependencias ni los presupuestos de caché. `Pack-Linux.sh` obtiene también la versión de las instrucciones incluidas desde Cargo, evitando conservar un número antiguo en el paquete.

## Preparación reproducible de CI

La [CI previa](https://github.com/MHAlberto/crabpdf/actions/runs/38779819304), correspondiente a 0.3.3 (`be09b74`), aprobó Linux y macOS Intel/ARM, incluidas siete pruebas nativas PDF por target. Windows falló al generar muestras porque esperaba un CHM extraído manualmente. `New-FormatSamples.ps1` ahora extrae ese archivo del instalador de 7-Zip 26.04 tras verificar su SHA-256. La generación pasó en un directorio nuevo. Estos resultados remotos de 0.3.3 no se atribuyen a 0.3.4 ni certifican UI o formatos de macOS.

El primer empaquetado Windows dejó de avanzar durante la creación del ZIP; no se determinó su causa exacta. Se detuvo únicamente ese proceso y se repitió con `ProgressPreference=SilentlyContinue`; el ZIP se generó correctamente. El script portable conserva ese ajuste para ejecuciones por lotes. No se modificaron los binarios por este cambio.

## Regresiones

Las cuatro pruebas de `tests/reading_session.rs` cubren una página de 400×3000 puntos con otro viewport/zoom, el centro entre páginas, enfrentadas/libro con cuatro giros y manga, y compatibilidad de preferencias/esquemas inválidos. Pasaron durante el desarrollo.

`examples/reading_session_smoke.rs` ejecuta **tres procesos nativos independientes** con PDFium real y perfil aislado: guarda una posición junto al final de la página larga, restaura sesión a 820×620 con escala lógica 1.25 y verifica la prioridad de `--page 1 --zoom fit-page`. Compara la región visible, comprueba el TOML tras el cierre y guarda tres capturas. La ejecución de desarrollo pasó; se inspeccionaron las imágenes de guardado y restauración.

## Validación y paquetes

Windows 10 x64 build 19045 aprobó `cargo fmt --all -- --check`, `cargo clippy --locked --offline --all-targets -- -D warnings` y `cargo test --locked --offline`. Comandos y resultados de desarrollo (`artifacts/validation-reading-20261015/development-checks.json`, registro local no versionado).

La matriz `scripts/Test-Local.ps1 -Architecture both -ReportRoot artifacts/validation-reading-20261015/windows` aprobó **19 comprobaciones**: formato, Clippy, suite release con pruebas ignoradas, builds, CLI, búsqueda y recorridos gráficos en cada arquitectura. Produjo **25 + 3 + 46 + 16 = 90 capturas por arquitectura**. X86 corrió bajo WOW64. Informe y logs (`artifacts/validation-reading-20261015/windows/results.json`, registro local no versionado). Se inspeccionaron guardado/restauración x64, restauración x86 y pantalla completa limpia x64.

La [CI del código 0.3.4](https://github.com/MHAlberto/crabpdf/actions/runs/38785344688), commit `a6cdd22`, aprobó los cinco targets: Windows x64/x86, Ubuntu x64 y macOS Intel/ARM. Incluye formato, Clippy, pruebas ordinarias, siete pruebas nativas PDF por target y build release; en Windows también preparación de muestras y siete pruebas de formatos/marcas. No ejecuta la interfaz ni valida otros formatos de macOS. Estado consultado (`artifacts/validation-reading-20261015/ci.json`, registro local no versionado).

Los ZIP de Windows contienen **607 archivos verificados** cada uno y el ejecutable idéntico al compilado. Verificación final (`artifacts/validation-reading-20261015/verify-windows-packages-final.log`, registro local no versionado).

Linux x64 se compiló con Rust 1.96.0 sobre Debian Bookworm/glibc 2.36, reutilizando PDFium 7881, MuPDF 1.28.5 y libmobi 0.12 de procedencia verificada. Se ejecutaron `cargo build --locked --offline --release --bins --examples`, `cargo test --locked --offline --release --no-run --message-format=json`, la exportación de licencias y `Pack-Linux.sh`. Comandos de construcción (`artifacts/validation-reading-20261015/build-linux.sh`, registro local no versionado) y artefactos de Cargo (`artifacts/validation-reading-20261015/linux-test-build.jsonl`, registro local no versionado).

En Ubuntu 24.04 limpio, APT instaló el DEB con sus dependencias y Xvfb. Pasaron las pruebas compiladas de lógica, posición, perfiles, sincronización de marcas, recuperación de importación, las siete de PDF y las siete de formatos/marcas, incluidas las ignoradas. `Verify-Linux.sh` aprobó **408 archivos por paquete** y la identidad del ejecutable; también se verificó el manifiesto instalado. `visual_smoke` produjo 25 capturas y `reading_session_smoke` tres, con PDFium y herramientas instaladas y render Mesa por software. Se inspeccionaron las capturas Linux de guardado/restauración. El ejecutable instalado abrió su ventana con el PDF durante ocho segundos sin errores de arranque; terminó mediante el timeout esperado, código 124. Informe Ubuntu (`artifacts/validation-reading-20261015/ubuntu/results.json`, registro local no versionado), comandos (`artifacts/validation-reading-20261015/test-ubuntu.sh`, registro local no versionado) y arranque instalado (`artifacts/validation-reading-20261015/test-installed-window.sh`, registro local no versionado).

| Paquete | Bytes | SHA-256 |
| --- | ---: | --- |
| `crabpdf-0.3.4-win-x64.zip` | 43 566 963 | `7a89992125f356c6adc064f90e5bd4c839a13f3663d42f4ae4927a388e3f4b07` |
| `crabpdf-0.3.4-win-x86.zip` | 43 202 740 | `ad1c80ce932dfc899d8615dd8f03790f5705f52d9f75de37ad299cc668b725a8` |
| `crabpdf-0.3.4-linux-x64.tar.gz` | 40 583 757 | `15f5ae82d38b7923c92d7cf41055adcda33180b8b7391e6887b53dbcb1a51623` |
| `crabpdf-0.3.4-ubuntu24.04-amd64.deb` | 30 599 980 | `06741b95139732d18b71f2b2598f72c5ff41585dcf088a09dbbce5af49fecb1d` |

Registro de paquetes (`artifacts/validation-reading-20261015/packages.json`, registro local no versionado) y hashes de fuentes/scripts (`artifacts/validation-reading-20261015/sources.json`, registro local no versionado). Los binarios corresponden al código de `a6cdd22`; el commit de entrega posterior solo documenta resultados y silencia el progreso del empaquetador ZIP. Las fuentes de conversores siguen en `crabpdf-0.2.0-format-tools-sources.zip`, sin cambios. Los paquetes incluyen sus manifiestos y avisos; el inventario mantiene 47 dependencias sin texto propio en Cargo para todos los targets Windows y 12 para Linux, con los textos upstream disponibles incluidos. La revisión final de distribución continúa pendiente de la versión estable.

## Límites y siguiente trabajo

El punto es geométrico y relativo a la página: todavía no tiene una firma del contenido ni un ancla por cita/texto. Un archivo modificado o un importador con otra paginación puede llevar a otro contenido. El scroll se limita a los bordes cuando el nuevo viewport no permite mantener exactamente la misma posición visual. La escala lógica de la prueba no certifica todas las transiciones DPI de monitores reales.

Ubuntu se ejecutó en X11 virtual con Xvfb; faltan escritorio físico, Wayland, otras distribuciones y servicios nativos. Windows x86 se probó bajo WOW64, no en un SO x86; Windows 11 sigue pendiente. La CI de macOS verifica compilación y PDF, sin interfaz, otros formatos, `.app` o firma. Los componentes nativos C/C++ permanecen; estos resultados no acreditan un motor íntegramente Rust ni rendimiento equivalente a SumatraPDF.

El historial atrás/adelante completo sigue en memoria y no recupera todas las regiones al reiniciar. También quedan pendientes la reconciliación de sesiones de varias ventanas sobre el mismo archivo (R03), archivos movidos, recuperación tras cierre abrupto y equivalentes de todas las plataformas. **R10 continúa en progreso.** Las prioridades siguientes son cerrar esos contratos y ampliar selección, búsqueda y paneles de H2; impresión, edición general, accesibilidad y el resto de la matriz siguen siendo requisitos de V1.
