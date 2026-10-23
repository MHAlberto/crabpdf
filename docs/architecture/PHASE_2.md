# Migración A02b: pestaña lógica y estado visual

Base de aplicación: 0.3.6, revisión anterior `8352541`, código del corte [`bac1777`](https://github.com/MHAlberto/crabpdf/commit/bac17778347b71a7474a9d4c6248edd7faa73da3). Continúa los [primeros cortes](PHASE_1.md) y el [plan A00–A07](../ARCHITECTURE_MIGRATION.md). Este documento describe el corte A02b; búsqueda y selección como casos de uso completos quedan para A02c.

## Propiedad del estado

| Módulo | Estado y reglas |
| --- | --- |
| `application/tabs.rs` | `ReadingTab`: identidad, ruta, estado de lectura, apertura/reintento, restauración, prioridad de opciones iniciales, navegación, snapshot y aplicación de metadatos |
| `application/search.rs` | `SearchSession`: consulta, opciones, generación, resultados y progreso; por ahora sólo datos |
| `application/profile.rs` | Definiciones únicas de `StoredDocument` y `LibraryEntry`, con el mismo esquema Serde |
| `ui/tab_view.rs` | Scroll, viewport, geometría de página, ancla de zoom, animación, espera de raster, revelado, entrada de contraseña y foco/temporización de búsqueda |
| `ui/mod.rs`, `ui/panels.rs`, `ui/viewer.rs` | Adaptación de eventos, reloj, recursos gráficos y ejecución de los efectos existentes |
| `config.rs` | Preferencias, disco y escritor concurrente; reexporta los modelos para mantener su API |
| `cli.rs` | Parseo/validación de argumentos; reexporta `InitialView` desde aplicación |

Una pestaña visual compone exactamente un `ReadingTab` y un `TabViewState`. El ID del lector continúa identificando eventos, widgets y cachés; mover la pestaña mueve ambos estados. No se mantienen mapas paralelos ni copias sincronizadas. Los accesos `tab.reader` y `tab.view` hacen explícito quién posee cada dato.

La posición guardada es relativa a la página. La vista captura sus coordenadas a partir del layout conocido y las entrega al modelo; el modelo conserva la posición pendiente de restaurar hasta que la vista la aplique. La navegación explícita libera esa posición y la vista cancela la animación/espera anterior. Zoom, rotación y modos mantienen las reglas existentes.

## Apertura y compatibilidad

`finish_open` aplica primero la sesión guardada y después las opciones iniciales explícitas. Cada opción de vista explícita invalida el ancla guardada, incluida una búsqueda explícita vacía, conforme al comportamiento anterior. Un ancla de versión futura o fuera del documento recurre a la página guardada, acotada al número de páginas actual.

Un intento de contraseña fallido conserva la restauración y las opciones de inicio. `retry_open` inicia otro intento sobre el mismo ID. El adaptador sigue detectando si el formato admite contraseña y enviándola al worker; el modelo no conoce PDFium, conversores ni contraseñas introducidas.

El resultado tipado `OpenOutcome` distingue `Ignored`, `Failed` y `Ready`. Además de la extracción, se corrige el tratamiento de una apertura repetida/ajena: no reaplica restauración ni vuelve a contar la apertura del libro en la biblioteca. La protección se basa en ID y estado de apertura pendiente; las generaciones de solicitudes dentro de un mismo intento siguen perteneciendo al trabajo A03.

Se conservan las rutas públicas `config::{StoredDocument, LibraryEntry}` y `cli::InitialView`. Los registros mantienen nombres, campos, valores y defaults Serde, incluidos perfiles sin manga/posición o con entradas antiguas de biblioteca. No cambia el perfil, la versión de dependencias, el motor, el presupuesto de cachés ni los formatos soportados.

## Verificación

En Windows 10 x64, Rust 1.96.0 y PDFium 7881:

- `cargo fmt --all -- --check` y Clippy con `--all-targets --target x86_64-pc-windows-msvc -- -D warnings`.
- 32 pruebas unitarias y 32 pruebas de integración, todas aprobadas, incluidas las nativas normalmente ignoradas. Los helpers ejecutados en procesos hijos se contabilizan dentro de su prueba de integración.
- Siete contratos nuevos en `tests/tabs_contract.rs`: restauración/snapshot, prioridad de opciones iniciales, cada opción explícita, contraseña/eventos ajenos o repetidos, anclas inválidas/futuras, pestañas duplicadas/reordenadas y perfil antiguo.
- PDFium: ocho pruebas; formatos/marcas: siete; sincronización de marcas: dos; recuperación de importación: una; procesos de perfil: dos; lectura: cuatro; contrato de favoritos: una.
- Ejecutable principal e instrumento de rendimiento construidos con el perfil release normal, ThinLTO y dependencias bloqueadas. Las pruebas usan perfiles/datos separados del usuario.

Comando principal de regresión:

```powershell
$env:RUST_READER_PDFIUM = "$PWD/native/pdfium.dll"
$env:CRABPDF_DATA_DIR = "$PWD/artifacts/architecture-phase2/checks/data"
cargo test --locked --offline --release --target x86_64-pc-windows-msvc --lib --tests -- --include-ignored --test-threads=1
```

Los cinco ejecutables gráficos también se reconstruyeron con el perfil release normal. Aprobaron:

- `document_chrome_smoke`: 25 escenas con barra propia y 25 con barra nativa, incluyendo favoritos por libro, pestañas/duplicación, controles y pantalla completa.
- `Test-WindowGeometry.ps1`: seis muestras Win32 de pantalla completa, desde ventanas normales/maximizadas y con ambas barras. El cliente ocupa el monitor completo de 1920 × 1080; también se comprueba el viewport lógico.
- `reading_session_smoke`: tres procesos independientes para guardar, restaurar a otro tamaño/escala y aplicar prioridad CLI.
- `transition_smoke`: movimiento normal y reducido, navegación interrumpida y operaciones de lectura/marcas.
- `search_smoke` y `cli_smoke`: búsqueda inicial y opciones de vista al abrir documentos.

La [evidencia de Windows](evidence/a02b-windows-validation.json) registra los resultados, hashes de ejecutables y geometría. Las capturas y logs completos permanecen localmente en `artifacts/architecture-phase2`.

La [CI del código bac1777](https://github.com/MHAlberto/crabpdf/actions/runs/40011816530) terminó correctamente en los cinco targets: Windows x64/x86, Ubuntu x64 y macOS Intel/ARM. Comprueba compilación, Clippy y pruebas sin ventana; las pruebas de formatos/marcas también se ejecutaron en ambos jobs Windows. Los recorridos gráficos descritos aquí se ejecutaron localmente en Windows x64.

El corte se conserva como desarrollo 0.3.6. No se publican instaladores ni se presenta como terminada la versión 1.0.

## Recursos

La [evidencia de mediciones](evidence/a02b-measurements-windows-x64.json) conserva tres series de tres procesos: referencia previa, extracción y control con el instrumento anterior preservado y verificado por SHA-256. El control se ejecutó después de la extracción, con el mismo corpus y perfiles nuevos, sin compilaciones simultáneas.

| Operación, mediana en ms | Referencia previa | Extracción | Anterior como control |
| --- | --- | --- | --- |
| Apertura PDF | 71.94 | 33.79 | 36.98 |
| Arranque hasta disponibilidad | 373.02 | 293.84 | 295.73 |
| Página precargada | 22.11 | 12.75 | 14.90 |
| Última página sin precargar | 32.55 | 24.54 | 26.15 |
| Scroll con estabilidad de 80 ms | 94.57 | 84.74 | 84.24 |
| Reapertura en el mismo proceso | 73.00 | 57.69 | 65.84 |
| EPUB sin caché interna | 202.94 | 139.16 | 146.43 |

El binario anterior también cambió respecto a la referencia inicial. No se atribuye una mejora de velocidad a esta separación. La extracción no muestra una ralentización superior al umbral provisional del 5 % frente al control contemporáneo en estas rutas; tres muestras no bastan para una conclusión estadística.

El pico mediano de working set fue 117.50 MiB antes, 117.04 MiB tras la extracción y 115.23 MiB en el control. La diferencia de la extracción frente al control es aproximadamente +1.6 %, dentro del umbral provisional del 5 %. El ejecutable principal pasó de 11 402 240 a 11 397 120 bytes (−0.045 %). Se conservan los intervalos individuales de CPU de cuatro segundos: la mediana cero no implica consumo nulo.

Son mediciones de la app en Windows x64, sin sumar conversores o GPU. La caché del sistema no se vacía y la disponibilidad se observa después de enviar la UI, sin medir la presentación física. Persisten los límites y pendientes de [MEASUREMENTS.md](MEASUREMENTS.md) y A00.

## Continuación

Al cerrar A02b, el siguiente corte previsto fue A02c, con estas entradas concretas. Su implementación y evidencia se registran ahora en [PHASE_3.md](PHASE_3.md):

1. Mover reinicio, aceptación de lotes y avance de coincidencias desde `ReaderApp::{start_search, poll_worker, step_search}` a aplicación. Recibir la generación como dato; devolver destino/revelado y solicitud de detener al alcanzar el límite. El foco y el debounce de 200 ms siguen en la vista.
2. Separar reglas de rango/selección pendiente de los handlers de contenido y acciones, conservando la caché de texto en su adaptador y el hit testing de pantalla en el visor.
3. Probar generaciones/IDs ajenos, consulta vacía/cancelada, límite de resultados, avance circular y contenido tardío tras navegar/cambiar de pestaña. Repetir búsqueda nativa, selección/copiar/marcar y transiciones.

`AppState` sigue en `state.rs`, sin tipos gráficos; su extracción física pertenece a los siguientes cortes.

La aplicación permanece en un único paquete Cargo. La independencia funcional de estos módulos ya puede probarse sin crear una ventana o cargar PDFium; la independencia de compilación sin dependencias gráficas/nativas llegará al extraer core/application en A04/A05.
