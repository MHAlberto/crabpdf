# Plan de migración de arquitectura de CrabPDF

**Fecha:** 23 de octubre de 2026. **Base:** 0.3.6, commit `3516ac91fc39c278a2c0ce19a3d85e01de761541`. **Estado:** ejecución iniciada. [Primeros cortes y evidencia](architecture/PHASE_1.md): base local A00, metadatos A01 y favoritos A02a. [A02b](architecture/PHASE_2.md) separa la pestaña lógica, los modelos de perfil y el estado visual. [A02c](architecture/PHASE_3.md) extrae búsqueda/selección y cierra A02 dentro del paquete actual. A03 y la división de paquetes siguen pendientes. La aplicación conserva la versión de desarrollo 0.3.6.

La dirección seleccionada es un **monolito modular con puertos y adaptadores**, dividido por funciones de lectura. La [decisión ADR-0001](adr/0001-modular-architecture.md) compara opciones y explica sus costes. [ARCHITECTURE.md](ARCHITECTURE.md) sigue describiendo el código que existe; este documento describe el destino y cómo llegar.

## 1. Diagnóstico del código actual

Se inspeccionaron manifiestos, módulos, consumidores en tests/ejemplos, CI y scripts de distribución. Las cifras reproducibles están en [architecture-baseline-0.3.6.json](architecture-baseline-0.3.6.json). Cuentan líneas físicas, incluidos comentarios y blancos; no son complejidad ciclomática ni tiempos de ejecución.

| Punto observado | Evidencia en la base | Consecuencia y prioridad |
| --- | --- | --- |
| Coordinación mezclada con presentación | `src/ui/mod.rs`: 2181 líneas; `ReaderApp::with_config`, `poll_worker`, `execute`, `shortcuts`, `save_preferences` | Cambios de navegación o sesión requieren tocar el anfitrión gráfico. Alta |
| Una pestaña mezcla datos lógicos y visuales | `Tab` contiene `AppState`, búsqueda, contraseña, restauración, `egui::Rect`, scroll y animaciones | Separar propietario de lectura de propietario de vista. Alta |
| Contrato tipado incompletamente utilizado | `DocumentDetails` ya tiene `title`, `annotation_count`, `annotation_warning`; `poll_worker` vuelve a buscarlos en etiquetas españolas | Traducción y presentación influyen en decisiones. Resolver antes de extraer controladores. Alta |
| El trait del motor no es todavía el límite real | `PdfEngine` cubre abrir/render; el worker construye `PdfiumEngine` y llama a métodos concretos de búsqueda, marcas y exportación | No presentar la abstracción actual como un backend intercambiable completo. Alta |
| Colas con garantías diferentes | `worker.rs::Pending` limita renders/prefetch y usa canales acotados, pero `control` es un `VecDeque` sin un techo global | Definir admisión y equidad por clase; no descartar silenciosamente guardar o marcar para liberar espacio. Alta |
| Contrato de worker contiene tipos de implementación | `WorkerEvent::Cover` usa `library::Cover`; eventos y canales viven junto al ejecutor | Extraer mensajes/rasters independientes antes de separar paquetes. Alta |
| Motor reúne adaptación y políticas de almacenamiento/importación | `src/engine.rs`: 1007 líneas; conserva almacén de marcas, firma preparada, datos importados y OCR DjVu | Separar servicios alrededor del adaptador sin romper transacciones de exportación. Alta |
| Importación concentra varios mecanismos | `src/formats.rs`: 1532 líneas; detección, límites, procesos, metadatos, caché y recuperación | Separar proceso/caché/decodificación conservando límites, cancelación y publicación del manifiesto. Alta |
| Preferencias mezclan modelo y efectos | `src/config.rs`: 464 líneas; modelos, carga, merge con bloqueo y `PreferenceWriter` | Reglas de favoritos/sesión deben probarse sin disco; disco e hilo siguen siendo un único servicio. Alta |
| Plataforma cruza dos fronteras | `platform.rs`: búsqueda de PDFium/rutas junto a `rfd`, revelar archivo y abrir ventana | Separar servicios de escritorio de ubicación de bibliotecas/perfil. Media |
| Ciclo de vida asimétrico del worker | `PdfWorker` guarda/join del hilo de importación; el handle del renderizador no se conserva | Diseñar cierre y pruebas de drenaje antes de ampliar trabajos. Es riesgo de coordinación, no prueba de un bloqueo reproducido. Media |
| Superficie pública amplia | `src/lib.rs` publica los módulos; tests y ejemplos importan `crabpdf::engine`, `worker`, `config`, `ui` | Introducir fachadas/reexportaciones para migrar sin cambiar todo a la vez. Media |
| Pipeline supone un único paquete | `Pack-Portable.ps1` y `Verify-Portable.ps1` leen `packages[0]`; CI no usa `--workspace` | Actualizar junto con el primer workspace; validar miembros y binario correcto. Alta |
| Protección de publicación no conoce futuros paquetes | `Publish-Release.ps1` compara fuentes de `src`, manifiestos y algunas entradas de empaquetado | Incluir `crates/` y entradas nuevas antes de publicar código extraído. Alta |

### Lo que se conserva

El núcleo de geometría/lectura ya es independiente de egui/PDFium. Existen identidad de documento, revisión de contenido y generación de búsqueda; el motor corre fuera de la UI, con importación separada. Hay render por regiones, cachés por bytes, escrituras temporales, bloqueos, recuperación acotada y regresiones de lectura/ventana. La arquitectura nueva debe aprovechar y reforzar estas piezas.

No se trasladan como vigentes todos los hallazgos de la auditoría histórica 0.2: sincronización de marcas, perfiles y otros puntos recibieron avances en 0.3.x. La evidencia actual se consulta en [0.3.6](IMPLEMENTATION_PROGRESS_0_3_6.md), los informes anteriores y sus pruebas.

## 2. Destino de código

Primero separar módulos en el paquete existente. Después extraer paquetes que ya tengan dependencias claras. El destino físico es un workspace de cuatro paquetes: raíz desktop, core, application y runtime. Una crate por formato o por pantalla añadiría fronteras sin independencia suficiente.

Árbol objetivo; las rutas nuevas son propuestas, no archivos ya implementados. Se crean al extraer una responsabilidad real:

```text
Cargo.toml                         # paquete crabpdf + workspace, resolver 3
Cargo.lock                         # único lockfile
build.rs                           # recursos del ejecutable
src/
  main.rs                          # CLI, construcción y arranque de eframe
  lib.rs                           # fachada pública durante la migración
  bootstrap.rs                     # crea runtime/perfil y conecta repaint
  cli.rs                           # parsing; traduce a entradas de aplicación
  branding.rs
  desktop/
    dialogs.rs                     # rfd; construcción en hilo UI
    window.rs                      # ventana, fullscreen, nueva ventana
    shell.rs                       # revelar archivo, servicios del SO
  ui/
    mod.rs                         # ReaderApp como anfitrión de presentación
    tab_view.rs                    # rectángulos, foco, scroll y movimiento
    input.rs                       # teclado/gestos -> acciones
    command_meta.rs                # etiquetas, atajos y menú
    panels/                        # lectura, búsqueda, favoritos, propiedades
    viewer.rs
    chrome.rs
    home.rs
    icons.rs
    motion.rs
    theme.rs
crates/
  crabpdf-core/src/
    lib.rs
    document.rs                    # tipos, contenido y metadatos independientes
    render.rs                      # claves, regiones y buffers CPU
    layout.rs
    reading_position.rs
    annotations.rs                 # valores de marcas, no archivos
    error.rs
  crabpdf-application/src/
    lib.rs
    command.rs                     # entradas tipadas de los casos de uso
    event.rs                       # resultados independientes del ejecutor
    effect.rs                      # trabajo solicitado a adaptadores
    ports.rs                       # contratos pequeños que sí tienen consumidores
    reading.rs                     # navegación, historial y selección lógica
    tabs.rs
    search.rs
    favorites.rs
    library.rs                     # modelo reciente/frecuente, no PNG/PDFium
    profile.rs                     # modelos y políticas de sesión/preferencias
  crabpdf-runtime/src/
    lib.rs
    runtime.rs                     # ejecuta efectos y controla cierre
    worker/
      mod.rs
      scheduler.rs                 # prioridad, deduplicación y cancelación
      render.rs                    # dueño de PDFium y documentos
      import.rs
    pdfium/
      mod.rs
      text.rs
      raster.rs
      annotations.rs
      export.rs
    formats/
      mod.rs
      detect.rs
      prepare.rs                   # coordinación de preparación y recuperación
      process.rs                   # hijos, plazos, límites y cancelación
      cache.rs                     # identidad, bloqueo, manifiesto y commit
      ebook.rs
      comic.rs
      chm.rs
      image.rs
      djvu_text.rs
      import_info.js
    storage/
      mod.rs
      profile.rs                   # TOML, merge bajo lock y escritor
      annotations.rs
      fileops.rs
    covers.rs                      # caché PNG y miniaturas CPU
    source.rs                      # identidad de archivo, sin cambiar claves existentes
    paths.rs                       # AppPaths y descubrimiento de bibliotecas
    cache.rs                       # LRU por bytes
tests/                             # integración real, formato/perfil y contratos
examples/                          # recorridos nativos y evidencia visual
scripts/                           # instalación, validación y distribución
docs/adr/                          # decisiones con contexto y consecuencias
```

Los módulos futuros de impresión, formularios, operaciones PDF, firmas y SyncTeX se ubicarán en application/runtime/desktop según su función al implementar R17–R21. No se crean stubs ni dependencias para presentarlos como avances.

### Reglas de dependencias

| Paquete | Dependencias directas permitidas | Límite |
| --- | --- | --- |
| `crabpdf-core` | `std`, `serde` cuando se conserve serialización | Sin egui/eframe, PDFium, rfd, procesos, acceso a disco ni variables de entorno |
| `crabpdf-application` | core; `serde` para modelos de perfil existentes | Sin adaptadores concretos ni I/O; recibe tiempo, identidades y resultados como datos |
| `crabpdf-runtime` | core, application, PDFium, TOML/JSON, imagen, XML/CRC y std | Sin egui ni rfd; no crea ventanas ni texturas |
| raíz `crabpdf` | core, application, runtime, eframe y rfd | Compone servicios; conserva recursos y adaptación gráfica/nativa |

Las dependencias de pruebas se evalúan aparte; por ejemplo, una integración raíz puede necesitar tipos de PDFium. Los tipos nativos permanecen dentro de las funciones/adaptadores de runtime, no en los mensajes de aplicación.

La CI comprobará la ausencia de dependencias gráficas/nativas en el árbol de core/application con `cargo metadata`, además de compilar y probar esos paquetes por separado. También debe revisar I/O directo: un manifiesto limpio no impide llamar a `std::fs` desde un módulo.

## 3. Contratos y propiedad

### Entrada -> estado -> efectos -> resultado

1. UI/CLI traduce una intención a una acción tipada.
2. El controlador de aplicación valida estado/capacidad, actualiza su modelo y produce efectos pequeños.
3. Runtime ejecuta solicitudes de documento/perfil; desktop ejecuta diálogos/ventana y devuelve su resultado.
4. Un evento del adaptador se aplica al documento/petición vigente; la aplicación descarta resultados obsoletos.
5. La UI lee el modelo y actualiza solamente sus recursos visuales necesarios.

Los nombres de futuros tipos (`AppCommand`, `AppEvent`, `Effect`) son ilustrativos. La primera migración reutiliza `Action` y los eventos actuales donde resulte seguro. El objetivo es un único camino de ejecución para menú, teclado, CLI y pruebas, no un bus global.

El puerto de documentos es asíncrono y orientado a operaciones utilizadas: abrir/cerrar, contenido, búsqueda, render, marcas y exportación. Los mensajes son enums; no se necesita un trait por comando. El antiguo `PdfEngine` puede permanecer como fachada mientras el límite real se mueve al servicio de documentos. Los datos de portada deben salir de `library::Cover` a un tipo compartido de raster antes de extraer el protocolo.

### Identidad y datos obsoletos

- La ruta identifica el origen para el perfil; no sustituye el ID de una instancia abierta ni identifica una pestaña duplicada.
- Conservar la distinción entre instancia/documento, revisión de contenido, generación de búsqueda e identidad de caché del archivo preparado.
- Los resultados dependientes del contenido deben declarar la revisión que produjeron. Hoy `RenderKey` ya la tiene; revisar `Content`, `Details` y búsqueda al extraer el protocolo. Probar explícitamente los resultados que llegan después de una marca, cierre o reapertura.
- No introducir otra caché con una clave reducida a ruta/página. La revisión gobierna invalidación tanto CPU como GPU.
- Tipar mejor identificadores si evita confusiones reales; no cambiar por ello las claves persistentes ni abrir una migración de datos implícita.

### Presentación frente a reglas

`Tab` se divide entre estado lógico de aplicación y `TabViewState` de UI, asociado por ID estable. Selección por índices de texto y página pertenece a aplicación; hit-testing, rectángulos y puntero a UI. Fullscreen, restauración de maximizado y comandos de viewport pertenecen a desktop/UI. El ancla normalizada y el cálculo de layout permanecen independientes.

`Action::label` y los textos de atajos se trasladan a funciones de metadatos de presentación cuando se actualicen sus consumidores. No se intentará implementar métodos inherentes en la raíz sobre un enum definido en otra crate. Si se conservan temporalmente los métodos existentes durante una extracción, se registra su retirada en ese mismo corte de consumidores.

La admisión del scheduler distingue solicitudes reemplazables (render, consulta y precarga) de operaciones que modifican datos (marca y guardado). No se añade un límite genérico que pierda operaciones; se rechaza con estado visible o se aplica presión al productor. Revisar también equidad para que una búsqueda siga progresando durante una lectura larga, sin retrasar la página visible.

Las capacidades reflejan permisos del documento y operaciones reales del backend, incluyendo motivos de indisponibilidad. Se empieza con copiar/anotar ya existentes. Exportar marcas embebidas, imprimir o editar no se habilitan por reconocer una extensión.

### I/O y ciclo de vida

`AppPaths` se resuelve al arrancar y se inyecta; ningún servicio extraído vuelve a buscar por su cuenta el perfil o un directorio global. Se conservan variables heredadas y prioridad de configuración. El modelo de perfil no escribe; el adaptador sigue agrupando solicitudes y realizando merge bajo bloqueo.

Un propietario de runtime gestiona workers, colas y cierre. Guardar marcas/exportar conserva orden y bloqueo actuales. El cierre desconecta receptores, cancela importaciones y libera recursos; se prueba con canal lleno e importación activa antes de añadir un join al renderizador. Un join ingenuo en el frame podría congelar el cierre.

No se añade Tokio ni un contenedor de inyección. Los hilos/std y canales existentes bastan para el diseño inicial. Tampoco se amplía indiscriminadamente el pool de PDFium: sus llamadas nativas continúan bajo un único propietario.

## 4. Correspondencia de rutas

| Origen actual | Destino lógico | Corte y evidencia que debe conservar |
| --- | --- | --- |
| `document.rs`, `layout.rs`, `reading_position.rs` | core | Reexportar los mismos tipos; pruebas de geometría, ancla y lectura |
| `state.rs` | application/reading | Reglas sin contexto egui; límites, historial, render obsoleto |
| `commands.rs` | application/command + ui/command_meta | Enum operativo separado de etiquetas/atajos; inventario y paridad conservados |
| `ui/mod.rs::Tab/SearchState` | application/tabs, search + ui/tab_view | Una fuente de estado; navegación/búsqueda sin ventana y lectura nativa |
| `ui/mod.rs::execute/poll_worker` | application + runtime executor + ui para recursos visuales | Primero favoritos/pestañas, después búsqueda/marcas; no un nuevo controlador gigante |
| `ui/mod.rs::with_config`, `main.rs` | bootstrap + runtime construction | Misma configuración, perfil y tratamiento de errores |
| `config.rs` | application/profile, favorites, library + runtime/storage/profile | Serialización, orden, merge concurrente y propagación de errores |
| `worker.rs` | application/event, effect + runtime/worker | Mismos límites/orden; protocolo sin referencias a módulos de infraestructura |
| `engine.rs` | runtime/pdfium | Separar adaptación de texto/raster/marcas/exportación sin sacar handles del hilo |
| `formats.rs`, `import_info.js` | runtime/formats | Mismos argumentos de herramientas, límites, firmas y recuperación acotada |
| `formats.rs::fingerprint` | runtime/source, reexportación temporal en formats | Misma canonicalización, tamaño, fecha y hash; no convertirlo implícitamente en un hash de contenido ni cambiar los nombres persistentes |
| `annotations.rs`, `fileops.rs` | runtime/storage | Esquema, identidad, bloqueos y reemplazo temporal conservados |
| `library.rs` | runtime/covers + raster compartido | Mantener conversión fuera del hilo PDFium y caché de portada acotada |
| `djvu_text.rs` | runtime/formats/djvu_text | Coordenadas y OCR existente equivalentes |
| `cache.rs` | runtime/cache; consumidor GPU puede reutilizar la utilidad | Presupuesto por bytes; sin otra copia de buffers |
| `app_paths.rs`, parte de `platform.rs` | runtime/paths | Perfil único e inyectado; prueba de procesos |
| resto de `platform.rs` | desktop/dialogs, window, shell | Diálogo creado en hilo UI, argumentos/configuración de nueva ventana |
| `ui/panels.rs` | ui/panels por función | Separar al tocar el caso de uso; conservar tamaños/foco/atajos |
| `ui/viewer.rs`, chrome, motion, icons, theme | UI | Se conservan hasta que un cambio concreto requiera extraerlos |

## 5. Secuencia ejecutable

Cada fila es un hito con cambios pequeños y reversibles. Los hitos con varias funciones se dividen en PRs; no se acumulan en una rama de reescritura. La relación con Rxx permite ampliar el código sin suspender la meta V1.

| ID / estado inicial | Entrega | Depende de | Comprobación para cerrar | Rutas V1 |
| --- | --- | --- | --- | --- |
| A00, pendiente en ejecución | Fijar corpus, pruebas y mediciones antes de modificar código; registrar herramientas/targets y fixtures | Diseño actual | Referencia repetible de funcionalidad y rendimiento; límites de evidencia declarados | R26, R27 |
| A01, pendiente | Consumir metadatos tipados existentes; caracterizar prioridad de título importado, contador y aviso | A00 funcional | Etiquetas de propiedades no cambian lógica; errores visibles; pruebas sin ventana + PDF/importación | R08 |
| A02, pendiente | Extraer modelos/casos de uso en el paquete actual: favoritos, pestañas/navegación, después búsqueda/selección; separar vista de pestaña | A01 | Pruebas de acciones y eventos sin eframe/PDFium; recorridos de lectura, foco y ventana equivalentes | R08–R13, R22, R30 |
| A03, pendiente | Extraer protocolo, scheduler, almacenamiento, proceso/caché de importación y adaptador PDFium por responsabilidad | A01; A02 para los consumidores afectados | Resultados tardíos descartados; invalidación coherente; cierre probado con canal lleno y sin ciclos de espera; pruebas nativas de almacenamiento/importación/marcas | R01–R09, R11 |
| A04, pendiente | Introducir workspace y extraer core; actualizar CI, empaquetado, licencias y protección de publicación en el mismo cambio | A02; contrato necesario de A03 | core se comprueba aislado; consumidores raíz compilan; paquetes detectan crabpdf por nombre, no posición | R09, R24–R27, R29 |
| A05, pendiente | Extraer application, después runtime, en dos cortes; conectar en bootstrap y mantener fachada raíz | A03, A04 | Grafo acíclico real; pruebas independientes; interfaz sin reglas duplicadas ni I/O de perfil/importación en frame | R03, R08–R09 |
| A06, pendiente | Consolidar servicios de desktop y retirar accesos directos/compatibilidad temporal migrada | A05 | Fullscreen desde normal/maximizada, ambos estilos de barra, teclado, arrastre, diálogo y nueva ventana preservados | R13, R21–R22, R30 |
| A07, pendiente | Comparar mediciones, revisar APIs/visibilidad, actualizar arquitectura real y evidencia por plataforma | A04–A06 | Aceptación funcional y de recursos; nuevas funciones V1 utilizan los límites; sin rutas de compatibilidad duplicadas | R24–R29 |

A03 puede avanzar por módulos mientras termina A02, sin cambiar el contrato de una función que esté siendo extraída. A04 no exige que toda la importación esté reorganizada; sí que los tipos de core estén libres de mecanismos.

### Primeros tres cambios preparados para ejecución

**PR 1 — línea base y mediciones (A00).** Reutilizar `tests/reading_session.rs`, `profile_sessions.rs`, `native_pdf.rs`, `formats_and_marks.rs`, `annotation_sync.rs`, `import_recovery.rs` y los ejemplos existentes. Añadir el registro reproducible de mediciones que falta, sin reescribir esos recorridos. Salida: comandos, entorno, corpus y valores de referencia; no una mejora de latencia supuesta.

**PR 2 — metadatos como datos (A01).** Cambiar `ReaderApp::poll_worker` para consumir los tres campos tipados ya disponibles. En el productor, obtener el título como dato y aplicar la precedencia del importador antes de construir la lista de propiedades, sin reconstruirlo buscando una etiqueta. Probar propiedades con etiquetas modificadas, título del importador frente al título PDF, ausencia de título/aviso y contador actualizado. Mantener la tabla de propiedades como presentación. Si se corrige la precedencia actualmente inconsistente del título, registrarla como corrección explícita, no ocultarla como movimiento de código.

**PR 3 — favoritos como primer corte completo (A02a).** Extraer modelo/reglas puras de favoritos de `config.rs` y coordinación de `ui/mod.rs` a `src/application/favorites.rs`, conservando fachada de `Preferences` y TOML. El panel envía intención y consume resultados; el escritor sigue ejecutando guardado fuera del frame. Salida: dos libros aislados, orden por página, quitar/añadir y navegar sin modificar zoom/giro; regresión `document_chrome_smoke`. Después aplicar el patrón a pestañas (A02b) y búsqueda (A02c).

No se necesita crear todavía los cuatro manifiestos para ejecutar estos tres cambios.

## 6. Workspace y distribución

Al llegar a A04, conservar `[package] name = "crabpdf"` y su versión explícita en el manifiesto raíz; añadir `[workspace]`, `resolver = "3"` y miembros reales. Los nuevos paquetes usan `publish = false`. Mantener `Cargo.lock`, perfiles release y recursos en la raíz; heredar lints explícitamente en todos los miembros. No cambiar versiones de dependencias ni motor en el mismo movimiento.

Adaptaciones obligatorias:

- CI: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked --target … -- -D warnings`, `cargo test --workspace --locked --target …`; conservar pruebas nativas explícitas y construir `cargo build -p crabpdf --bin crabpdf --release --locked --target …`.
- Ejecutar además `cargo test -p crabpdf-core -p crabpdf-application --locked` sin bibliotecas nativas ni contexto gráfico. Esto comprueba independencia en ejecución; el árbol de dependencias comprueba el límite de compilación.
- `scripts/Test-Local.ps1`: incluir todos los miembros en lógica/lints; mantener ejemplos y pruebas de integración en el paquete raíz.
- `scripts/Pack-Portable.ps1` y `Verify-Portable.ps1`: seleccionar exactamente `metadata.packages` por nombre `crabpdf` y confirmar un único resultado. Mantener ejecutable, recursos y ruta `target/<target>/release/crabpdf.exe`.
- `Pack-Linux.sh` y `Verify-Linux.sh`: conservar lectura de versión raíz mientras sea explícita, o reemplazarla conjuntamente por selección del paquete; no introducir `version.workspace = true` y dejar el `awk` actual.
- `Export-Licenses.ps1`, `Export-RustLicenses.py` y documentación de avisos: distinguir miembros locales mediante `workspace_members` de dependencias externas, para no tratar el árbol del proyecto como fuente de licencias de terceros.
- `Publish-Release.ps1`: ampliar entradas de fuentes a `crates/` y nuevos manifiestos/recursos/entradas de compilación. No permitir publicar un tag antiguo cuando cambien paquetes internos.
- Verificar ZIP Windows y tar/DEB Linux, DLL/so correspondientes y manifiestos SHA. Una migración de carpetas no debe perder recursos, avisos, conversores ni fuentes distribuidas.

## 7. Pruebas y criterio de aceptación

### Reglas que necesitan pruebas de contrato

1. Cerrar/reabrir o duplicar no permite que un evento anterior actualice otra instancia.
2. Una marca cambia revisión e invalida recursos afectados; texto, búsqueda y detalles tardíos se aceptan o rechazan según su contrato explícito.
3. Favoritos se filtran por libro; navegar a uno conserva los ajustes de lectura.
4. Cambiar consulta invalida sus resultados previos; cancelar no llena la UI con errores de trabajos ya abandonados.
5. Saturación de colas, cierre con trabajos pendientes y fallo del conversor producen un estado recuperable.
6. Guardar/exportar respeta merge, bloqueo, identidad y escritura final; fallar no destruye originales ni registros de marcas.
7. Capturar/restaurar posición respeta tamaño/escala diferente y prioridad de opciones CLI.

Los controladores se prueban con eventos y adaptadores en memoria donde importe el orden de efectos. Los adaptadores se prueban con archivos/procesos y bibliotecas reales. No se sustituyen esos casos por mocks que solamente repitan el nuevo código.

### Regresión gráfica y de plataforma

Mantener `visual_smoke`, `search_smoke`, `reading_session_smoke`, `refinement_smoke`, `transition_smoke`, `formats_smoke` y `document_chrome_smoke`. Para cambios en chrome/fullscreen, usar también `Test-WindowGeometry.ps1`: una captura de egui no mide por sí sola el cliente físico de la ventana.

La CI actual cubre compilación/pruebas de motor en cinco targets; no ejecuta toda la UI. Aceptar la migración requiere recorridos reales Windows x64/x86 y Ubuntu existentes, y evidencias macOS conforme se disponga de ejecución. X86 bajo WOW64 no prueba Windows de 32 bits nativo; Ubuntu no prueba todas las distros ni Wayland. Registrar esas diferencias en R24/R27 sin declararlas resueltas por el workspace.

### Rendimiento y tamaño

Antes/después sobre el mismo hardware, perfil, build release, fixtures y versiones nativas. Medir apertura fría/caliente hasta primera página útil, cambio de página listo/no precargado, desplazamiento, latencia durante importación, CPU en reposo, pico de memoria residente, cachés CPU/GPU, tamaño del ejecutable y tamaño del paquete completo. Registrar procesos conversores y arquitectura; en Linux distinguir X11/Wayland.

Tomar suficientes repeticiones para comparar mediana y dispersión; no reportar p95 con una muestra insuficiente. Los tiempos de compilación de scripts no son latencias de la aplicación.

Como puerta provisional de migración, investigar cualquier empeoramiento repetible superior al 5 % en mediana de latencia o memoria, y cualquier crecimiento de tamaño superior al 2 %. Son umbrales propuestos para revisar, no presupuestos V1 medidos. Un cambio que los supere necesita causa y resolución documentadas; no se oculta con promedios de plataformas distintas.

Preservar los límites existentes: caché CPU de raster 96 MiB x64 / 32 MiB x86, GPU 64 / 32 MiB, texto 16 MiB por caché. No son el máximo de memoria del proceso ni incluyen conversores. La división de crates no autoriza duplicar buffers, perfiles o cachés.

### Reversión

Cada corte conserva una fachada temporal y una implementación activa; revertir el commit restaura el camino anterior. Los movimientos mecánicos se separan de correcciones de comportamiento. No hacer escrituras dobles en perfiles/marcas ni cambiar esquemas en esta migración. Si un cambio de datos resulta imprescindible, prepararlo como entrega separada con prueba de actualización y recuperación antes de continuar.

## 8. Relación con la versión 1.0 y los plugins

La reorganización avanza R08/R09 y facilita las demás rutas; no cierra formularios, firmas, impresión, edición, accesibilidad, integraciones ni la matriz de paridad. [V1_PLAN.md](V1_PLAN.md) y [V1_BACKLOG.csv](V1_BACKLOG.csv) conservan sus condiciones de salida.

Tras el primer corte probado, las nuevas funciones entran por el módulo adecuado; no se espera al final de A07 para continuar V1. Por ejemplo: impresión usa un caso de uso propio y adaptadores del SO; un panel nuevo consume ese caso de uso sin ejecutar procesos o PDFium desde la vista.

Para V2, mantener APIs pequeñas e identidades opacas facilitará un host opcional Rust/Wasm. La decisión de ABI, permisos y runtime de plugins requiere su propio diseño; este plan no añade ese coste a la base V1.

## 9. Entrega de planificación (a231957)

Esta sección conserva el resultado de la revisión de diseño. La ejecución posterior se registra en [PHASE_1.md](architecture/PHASE_1.md).

- Inspección estructural sobre el commit de referencia y registro de tamaño por archivo.
- Alternativas evaluadas y dirección seleccionada en ADR-0001.
- Rutas de destino, propietarios, contratos, secuencia, primeros tres cambios y puertas de aceptación.
- Enlaces de navegación actualizados en la documentación existente.
- No se han movido módulos ni alterado comportamiento, dependencias, esquemas o versión.

En el momento de esa revisión, las mediciones de rendimiento, las nuevas pruebas de contrato y A00–A07 estaban pendientes de ejecución. Los resultados posteriores y los hitos que siguen abiertos están separados en el registro de avance.

## Fuentes consultadas

Consultadas el 23 de octubre de 2026; la selección y el plan son una evaluación propia del proyecto.

- [Cockburn: artículo original de puertos y adaptadores](https://alistair.cockburn.us/hexagonal-architecture), para separar lógica de mecanismos.
- [Cargo: workspaces](https://doc.rust-lang.org/cargo/reference/workspaces.html) y [resolver de Rust 2024](https://doc.rust-lang.org/stable/edition-guide/rust-2024/cargo-resolver.html), para límites de paquetes y configuración de compilación.
- [egui 0.36.2](https://docs.rs/egui/0.36.2/egui/), para conservar el modelo de UI inmediata.
- [Iced: arquitectura de una interfaz](https://book.iced.rs/architecture.html), como contraste de estado/interacción/vista.
- [pdfium-render: multithreading](https://github.com/ajrcarey/pdfium-render/blob/master/README.md#multi-threading), para conservar las restricciones del motor nativo.
- [Fowler: Branch by Abstraction](https://martinfowler.com/bliki/BranchByAbstraction.html), para extracción gradual con fachadas.
