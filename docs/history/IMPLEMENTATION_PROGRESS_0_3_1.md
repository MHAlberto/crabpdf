# CrabPDF 0.3.1 — recuperación de caché importada

> Documento histórico. Describe una revisión anterior. Consulta el [índice actual](../README.md) para las instrucciones vigentes.


Fecha: 14 de octubre de 2026. Este avance continúa la [entrega 0.3.0](IMPLEMENTATION_PROGRESS.md) dentro de R05 del [plan V1](../V1_PLAN.md). **No completa R05 ni la paridad con SumatraPDF.** No se creó repositorio remoto.

En 0.3.0 un PDF interno podía tener cabecera y `%%EOF`, tamaño y CRC coincidentes con su manifiesto, y aun así ser rechazado por PDFium. La apertura terminaba en error y las siguientes aperturas reutilizaban la misma entrada, de modo que el documento quedaba bloqueado hasta limpiar manualmente la caché.

Ahora, cuando PDFium rechaza el PDF preparado de una apertura importada, el worker envía una reparación al hilo de importación. Bajo el bloqueo de esa entrada, el importador comprueba que el manifiesto aún identifica el PDF rechazado, elimina sus archivos de caché y convierte otra vez el origen. PDFium intenta abrir el resultado **una sola vez más**; si vuelve a fallar, la UI recibe el error. La apertura de PDF directo y los errores de contraseña no disparan conversiones. El documento original no se escribe.

`tests/import_recovery.rs` construye un PDF de caché inválido pero con tamaño y CRC coherentes, comprueba que PDFium lo rechaza y que `prepare_in` lo aceptaría como hit, y pide abrir el EPUB mediante el worker. La regresión pasa cuando el worker reconstruye el caché y el PDF nuevo abre, sin cambiar los bytes del EPUB. Pasó en debug x64 y en release x64/x86. Se conserva el primer informe fallido de Clippy por un préstamo redundante, corregido antes de iniciar el informe nuevo.

`scripts/Test-Local.ps1 -Architecture both -ReportRoot artifacts/validation-repair-20261014-pass` terminó en Windows 10 x64 (build 19045) con **17 comprobaciones sin fallos**: formato, Clippy, pruebas y builds release, arranque CLI, búsqueda y recorridos de UI/formatos por arquitectura. Cada target produjo 25 capturas de UI, 21 de refinamiento y 16 de formatos; se inspeccionaron EPUB y lectura vertical. La ejecución x86 fue bajo WOW64, no sobre un SO de 32 bits. Informe y logs locales (`artifacts/validation-repair-20261014-pass/results.json`, registro local no versionado).

Los paquetes portables locales pasaron `scripts/Verify-Portable.ps1` con **607 archivos verificados** cada uno: `crabpdf-0.3.1-win-x64.zip` (43 552 055 bytes; SHA-256 `510c697b78e01e245efd8dedf1eae156663a4085db585a3c5cce7c5ae1d1b4cc`) y `crabpdf-0.3.1-win-x86.zip` (43 201 049 bytes; SHA-256 `1eea3a385a78336e889efc9d18563766fddcbae5801a19e22da606055e18fcca`). Hashes locales (`artifacts/validation-repair-20261014-pass/packages.json`, registro local no versionado). Los paquetes 0.3.0 no se modificaron.

Siguen abiertos: reparación de portadas, interferencia con poda u otros procesos que mantengan abierto el PDF, esperas de bloqueo acotadas, fallos de disco entre archivos, cambios externos del origen y coste del CRC para libros grandes. Windows 11, Linux y macOS aún no tienen prueba funcional de esta revisión. Por ello R05 continúa `en_progreso` en [V1_BACKLOG.csv](../V1_BACKLOG.csv).
