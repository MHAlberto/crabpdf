# Migración A03a: protocolo y fallos de búsqueda

Fecha: 26 de octubre de 2026. Base de desarrollo: 0.3.6, revisión anterior `48b5903`. Código: [`88fbc7d`](https://github.com/MHAlberto/crabpdf/commit/88fbc7d8ea099112620d3c6b660c4e2f656926b3). Continúa [A02c](PHASE_3.md) y el [plan de migración](../ARCHITECTURE_MIGRATION.md).

## Límites del protocolo

`application/protocol.rs` define ahora `WorkerEvent`. Sus payloads dependen sólo de datos de `document` y de `std`; no importan el worker, la caché de portadas, PDFium, egui ni acceso a disco. El productor de eventos sigue en el adaptador existente y la UI consume explícitamente el protocolo de aplicación.

El registro RGBA `Cover` se define en `document.rs`. `library.rs` conserva la carga, conversión y caché, y reexporta el mismo tipo. `worker.rs` reexporta `WorkerEvent`. Se mantienen las rutas `worker::WorkerEvent` y `library::Cover`: no hay tipos duplicados, conversiones o copias adicionales del buffer. Una prueba comprueba también la identidad de la asignación compartida por `Arc`.

Se agrega una variante pública `SearchFailed { id, generation, error }`. Los consumidores que hagan un match exhaustivo deben contemplarla. `WorkerEvent::search_result` conserva la misma identidad tanto en lotes correctos como en errores; el worker utiliza ese constructor para el resultado real de cada página buscada.

Esto separa el protocolo de salida. Los comandos privados, las colas, el propietario de PDFium, la importación y las escrituras todavía pertenecen a runtime dentro del paquete actual. No se han creado paquetes Cargo o puertos vacíos.

## Corrección de errores tardíos

Antes, un fallo de búsqueda se emitía como `Details(id, Err(error))`: perdía la generación y podía mostrar un aviso aunque el usuario ya hubiera cambiado o borrado la consulta. Ahora `SearchSession::fail` sólo acepta el error del documento/generación actuales cuando la búsqueda sigue pendiente.

Un fallo aceptado:

- Detiene el progreso de esa búsqueda y deja de producir solicitudes.
- Conserva consulta, coincidencias parciales y navegación entre ellas.
- Guarda el error exclusivamente en la sesión de búsqueda.
- No modifica los metadatos, el documento, los errores de renderizado o el estado de contraseña.

Una nueva consulta/reintento limpia el fallo anterior. Descartar el aviso sólo lo oculta; no reactiva la búsqueda. Errores antiguos, ajenos, duplicados, posteriores a la finalización o al límite de resultados se descartan igual que sus lotes tardíos.

La UI consulta `is_scanning` para el indicador de progreso y presenta el aviso incluso si la barra de búsqueda está oculta. Un error de documento tiene prioridad sobre el de búsqueda. Para el segundo, el aviso ofrece «Reintentar búsqueda» y «Cerrar aviso»; sus controles no activan el reintento de render ni el diálogo de contraseña. El adaptador público `ReaderApp::search_error` permite observar ese estado separado.

## Verificación

Aprobados localmente en Windows x64, Rust 1.96.0 y PDFium 7881:

- Formato y Clippy con `--all-targets -D warnings`.
- 33 pruebas unitarias y 51 pruebas de integración: 84 pruebas únicas, incluidas las nativas normalmente ignoradas. Los helpers de procesos hijos pertenecen a su prueba de integración.
- Seis contratos nuevos en `tests/protocol_contract.rs`: fachadas/Arc, identidad de éxito/error, generaciones ajenas/antiguas, conservación de coincidencias/metadatos/errores de documento, finalización/límite y descarte/reintento.
- Se mantienen las pruebas reales de PDFium, caché de portadas, formatos, marcas, recuperación, perfiles, lectura, favoritos, pestañas y selección.

```powershell
$env:RUST_READER_PDFIUM = "$PWD/native/pdfium.dll"
$env:CRABPDF_DATA_DIR = "$PWD/artifacts/architecture-phase4/checks/data"
cargo test --locked --offline --target x86_64-pc-windows-msvc --lib --tests -- --include-ignored --test-threads=1
```

También aprobados:

- Ejecutable principal Windows x64 en release con ThinLTO.
- `search_smoke`: coincidencia visible al final de una página alta, consulta vacía y sustitución rápida mediante teclado.
- `document_chrome_smoke` con barra propia/nativa: 50 capturas y seis muestras Win32 del cliente ocupando el monitor completo.
- `formats_smoke`: 16 capturas, selección de palabra, resaltados de colores, subrayado, persistencia, EPUB/CBR/XPS/DjVu/CHM, biblioteca con portadas y aviso de DRM. Se revisaron las capturas de biblioteca y error de documento.

Los ejecutables de pruebas gráficas usan el perfil dev y datos/preferencias aislados. Los fallos de búsqueda se comprueban mediante los contratos de aplicación/protocolo; no se ha añadido un método de inyección de errores a la aplicación de producción.

La [evidencia local](evidence/a03a-windows-validation.json) registra fuentes, ejecutables, conteos y geometría. Los logs y las 66 capturas quedan en `artifacts/architecture-phase4`.

La [CI del código 88fbc7d](https://github.com/MHAlberto/crabpdf/actions/runs/40336360506) terminó correctamente en sus cinco plataformas: Windows x86/x64, Ubuntu x64 y macOS Intel/ARM. La evidencia enlazada registra el resultado y cada job. Las comprobaciones gráficas descritas arriba se ejecutaron localmente en Windows.

## Recursos y alcance

No cambian las dependencias, las políticas de caché, la concurrencia nativa, las herramientas de formatos ni el esquema de perfiles. `Cover` conserva campos y asignación; el error nuevo se almacena en un único lugar. Este corte no realiza una nueva campaña de latencia: la evaluación completa de A00 y la variabilidad de [A02c](PHASE_3.md) siguen abiertas. No se atribuye una mejora de velocidad a mover estos tipos.

La aplicación conserva la versión de desarrollo 0.3.6. Este corte no publica instaladores ni completa la versión 1.0.

## Continuación de A03

A03a cierra la extracción del protocolo de resultados y el tratamiento de errores de búsqueda con generación. El hito A03 completo permanece abierto.

La extracción del mailbox/planificador se implementó en [A03b](PHASE_5.md), con prioridades, deduplicación, cancelación, precarga y contratos de cierre. Después se extraerán almacenamiento, importación y adaptación de PDFium por responsabilidad, conforme al plan A03–A05.
