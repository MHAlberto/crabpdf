# Guía de trabajo de CrabPDF

Estas instrucciones se aplican a todo el proyecto. Las instrucciones explícitas del usuario tienen prioridad. Mantener esta guía actualizada cuando cambien contratos o comandos reales; no convertirla en un registro de conversaciones.

## Objetivo y alcance acordado

- CrabPDF es un lector de escritorio local, rápido y ligero, desarrollado principalmente en Rust. La versión en desarrollo es **0.3.3**, no 1.0.
- **V1:** todas las funciones y detalles visibles de la revisión de SumatraPDF fijada en `docs/parity/reference.json`, con equivalentes comprobados por plataforma. Un botón, una extensión reconocida o una compilación correcta no demuestran paridad.
- **V2:** extensiones opcionales escritas en Rust y WebAssembly. No implementar todavía un runtime, tienda o ABI pública de plugins; tampoco trasladar requisitos de V1 a V2 para cerrar artificialmente la versión.
- Sistemas objetivo: Windows 10 x64 y x86, Windows 11 x64 y ejecución de la app x86 bajo WOW64, Linux de distribuciones populares y macOS Intel/Apple Silicon. Windows 11 no tiene una edición de SO x86. Ver `docs/V1_PLAN.md` para la matriz y sus límites de evidencia.
- La solicitud original autoriza bibliotecas PDF existentes: egui/eframe, pdfium-render/PDFium, rfd, serde/TOML y Cargo. **No exige reescribir PDFium ni traducir todo el C/C++ de SumatraPDF.** Informar con honestidad de los componentes nativos y su coste.
- Conservar identidad CrabPDF, el logo `crabpdf-logo.svg` transparente, pestañas superiores, Inicio y lectura vertical continua ajustada al ancho con precarga.
- Lectura sin cuentas ni conexión obligatoria. Las integraciones externas de referencia deben ser optativas y explícitas; nunca enviar documentos o selecciones automáticamente.
- El usuario pidió mejoras incrementales y **consultar antes de cambiar el framework de UI**. Evitar reconstrucciones globales de la arquitectura.
- El usuario autorizó publicar las versiones recientes en `https://github.com/MHAlberto/crabpdf`. Publicar solo artefactos identificados con versión, arquitectura, plataforma, procedencia y validación real. La CI escrita no es evidencia de ejecución.

## Dónde encontrar la verdad del proyecto

1. `README.md`: uso y capacidades actuales.
2. `docs/IMPLEMENTATION_PROGRESS_0_3_3.md`: avance actual; `docs/IMPLEMENTATION_PROGRESS_0_3_2.md`, `docs/IMPLEMENTATION_PROGRESS_0_3_1.md` y `docs/IMPLEMENTATION_PROGRESS.md`: entregas históricas; `docs/AUDIT_V1.md`: auditoría de 0.2.
3. `docs/V1_PLAN.md` y `docs/V1_BACKLOG.csv`: rutas, dependencias, archivos propuestos y criterios de salida.
4. `docs/ROADMAP.md`: alcance V1/V2; `docs/parity/reference.json`, `commands.csv` y `cases.md`: referencia y trazabilidad funcional.
5. `docs/ARCHITECTURE.md`, `FORMATS_AND_ANNOTATIONS.md`, `VALIDATION.md`: contratos y límites actuales.

El backlog es trabajo pendiente, no código existente. No usar el número de identificadores de comandos como porcentaje de avance. No actualizar la revisión upstream silenciosamente ni convertir inspección estática en prueba funcional.

## Mapa del código actual

| Ruta | Responsabilidad |
| --- | --- |
| `src/main.rs`, `src/cli.rs` | Arranque, opciones y errores de CLI. |
| `src/document.rs`, `src/state.rs`, `src/layout.rs` | Datos independientes del SDK, navegación y geometría. |
| `src/engine.rs` | Adaptador PDFium; documento, texto, raster, marcas y exportación. |
| `src/worker.rs`, `src/cache.rs` | Dueño de PDFium, colas, búsqueda y cachés con presupuesto. |
| `src/formats.rs`, `src/djvu_text.rs`, `src/import_info.js` | Preparación local de formatos y recuperación de texto/metadatos. |
| `src/annotations.rs` | Almacén local de marcas, bloqueo y persistencia. |
| `src/app_paths.rs`, `src/config.rs`, `src/fileops.rs`, `src/platform.rs` | Perfil, preferencias, escritura atómica y servicios del sistema. |
| `src/ui/` | Interfaz; `mod.rs` coordina, `viewer.rs` dibuja/interactúa, `panels.rs` presenta controles. |
| `src/commands.rs`, `docs/actions.csv` | Acciones de aplicación y documentación de atajos. |
| `src/library.rs`, `src/ui/home.rs` | Portadas e Inicio. |
| `build.rs`, `src/branding.rs`, `crabpdf-logo.svg` | Iconos y recursos nativos. |
| `tests/`, `examples/*smoke.rs`, `scripts/` | Regresiones, recorridos gráficos y distribución local. |

## Reglas de implementación

- Mantener `document`, `state` y `layout` libres de egui/PDFium. La UI recibe datos, identificadores y resultados; no objetos nativos.
- Respetar el único dueño de PDFium. No paralelizar llamadas al SDK sin comprobar el contrato real de la versión fijada. Sí separar importación, disco y servicios nativos de la UI y del camino de render visible.
- Colas acotadas, cancelación por documento/generación y prioridad para páginas visibles. Considerar también una tarea de baja prioridad que ya empezó: la prioridad de cola no la interrumpe.
- Conservar tiles y presupuestos por bytes, incluidos x86 y GPU. No rasterizar todo un libro para abrirlo. Los presupuestos de caché no limitan el RSS total.
- Las modificaciones de documento deben invalidar todos los datos afectados y descartar resultados anteriores. La revisión del contenido debe acompañar las claves cuando se implemente R01/R02.
- No usar textos traducidos de la interfaz como protocolo entre motor y UI. Preferir estructuras tipadas para capacidades, errores, progreso y metadatos.
- No sobrescribir el original como efecto de abrir, convertir o marcar. El futuro guardado explícito de V1 deberá proteger cambios externos, permisos y recuperación ante fallo.
- Persistir con reemplazo atómico; para varias ventanas hacen falta además bloqueo/versión y reconciliación. Atomicidad no significa ausencia de actualizaciones perdidas.
- Versionar esquemas y anclajes de anotación antes de cambiar paginación/importadores. Conservar y diagnosticar datos antiguos que no se puedan migrar; no descartarlos silenciosamente.
- Respetar permisos PDF y DRM. No ejecutar scripts, adjuntos, comandos ni enlaces externos por el simple hecho de abrir un documento.
- Acotar lecturas **antes** de reservar/cargar contenido; validar rutas y tamaños al extraer. Los procesos auxiliares necesitan cancelación, espera de terminación y limpieza incluso en errores intermedios.
- Mantener compatibilidad de `RUST_READER_*` y rutas antiguas hasta disponer de migración probada. Resolver `--config` y datos derivados de forma coherente; ver F05 en la auditoría.
- Reutilizar acciones entre menú, paleta, contexto y teclado; comprobar foco de campos de texto y disponibilidad por documento. Conservar entradas Unicode y atajos específicos de macOS.
- Extraer módulos al modificar una responsabilidad concreta. No introducir frameworks, bases de datos, capas genéricas ni dependencias grandes sin un caso real y medición del coste.
- `unsafe_code = "forbid"` se mantiene en el código propio. No atribuir esa garantía a bibliotecas nativas.

## Validación proporcionada al cambio

Para código Rust, desde la raíz:

```powershell
cargo fmt --all -- --check
cargo clippy --locked --offline --all-targets -- -D warnings
cargo test --locked --offline
```

`--offline` presupone dependencias ya descargadas. No cambiar versiones para resolver una falta de caché. El toolchain está fijado en `rust-toolchain.toml`; `Cargo.lock` forma parte de la entrega. La discrepancia entre MSRV 1.95 y toolchain 1.96 está pendiente de resolver, no de asumir compatible.

Las 14 pruebas de integración nativas de `native_pdf` y `formats_and_marks`, `annotation_sync` (prueba real entre procesos y su ayudante) e `import_recovery` están ignoradas por defecto; `cargo test` solo no las ejecuta. Con PDFium de la arquitectura correcta y decodificadores preparados:

```powershell
$env:CRABPDF_DATA_DIR = Join-Path (Get-Location) 'artifacts/test-reader-data'
$env:RUST_READER_PDFIUM = (Resolve-Path 'native/pdfium.dll').Path
cargo test --locked --offline --test native_pdf -- --ignored --test-threads=1
cargo test --locked --offline --test formats_and_marks -- --ignored --test-threads=1
cargo test --locked --offline --test annotation_sync -- --ignored --test-threads=1
cargo test --locked --offline --test import_recovery -- --ignored --test-threads=1
```

- Para cambios de UI/render/entrada, ejecutar el recorrido pertinente (`visual_smoke`, `search_smoke`, `refinement_smoke` o `formats_smoke`) e inspeccionar las capturas. Una captura no prueba todos los gestos.
- Para cambios en persistencia/concurrencia, agregar una regresión que produzca el fallo real: dos instancias, cancelación, disco lleno simulado o caché dañada, según el cambio. Evitar pruebas que repitan la implementación.
- Para cambios que se van a distribuir en Windows, usar `scripts/Test-Local.ps1` (x64 y x86); requiere escritorio interactivo y bibliotecas nativas. Después `scripts/Pack-Portable.ps1 -Architecture x64`/`x86` y `scripts/Verify-Portable.ps1` según sus parámetros documentados.
- Para Linux x64, compilar con Rust 1.96.0 y bibliotecas X11/Wayland en un entorno con glibc 2.36, compilar MuPDF/libmobi con `scripts/Build-LinuxFormatTools.sh`, exportar los textos de Cargo con `python3 scripts/Export-RustLicenses.py` y ejecutar `bash scripts/Pack-Linux.sh`. Comprobar la instalación del `.deb` y su `SHA256SUMS` en Ubuntu 24.04 limpio; ejecutar `formats_and_marks` y el recorrido `visual_smoke` con Xvfb. El tar requiere DjVuLibre y 7-Zip del sistema. Un CLI que imprime `--help` no acredita la interfaz.
- Aislar preferencias, datos y fixtures de prueba dentro de `artifacts/`. No probar sobre documentos personales ni borrar preferencias reales. Restaurar variables de entorno al terminar.
- Para cambios solo documentales, verificar rutas, CSV, referencias y coherencia; no recompilar todos los motores sin necesidad.
- Conservar informes históricos de validación. Guardar resultados nuevos con fecha, plataforma, hashes y comandos; no sustituir pruebas faltantes por resultados de otra versión o sistema.
- X86 bajo WOW64 no verifica un SO x86; compilar para macOS/Linux no demuestra que abra formatos, imprima o funcione con lectores de pantalla.

## Entregas y documentación

- Actualizar README, documentación de la función, acciones e inventario de paridad cuando cambie el comportamiento. Indicar límites y qué falta por validar.
- No declarar 1.0 hasta cumplir los criterios de `docs/V1_PLAN.md`. No cambiar versión ni generar paquetes por una auditoría documental.
- Paquetes autocontenidos: biblioteca nativa de ABI correcta, decodificadores, recursos, licencias, fuentes exigibles y manifiesto. Medir el paquete completo, no solo el `.exe`.
- Conservar procedencia de todo código, fixture y herramienta externa. Estudiar o traducir código upstream no elimina sus condiciones de licencia.
- Continuar las acciones locales reversibles autorizadas sin pedir confirmación rutinaria. Para cambios fuera del alcance, respetar las instrucciones del usuario y las restricciones del entorno.
- Comunicar en español lo cambiado, su motivo, cómo se verificó y los límites pendientes. No afirmar pruebas o correcciones que no se realizaron.
