# Auditoría de CrabPDF hacia 1.0

Fecha: **13 de octubre de 2026, zona America/Mexico_City**. Base: código local **0.2.0**. Alcance: arquitectura, flujos de lectura/importación/anotaciones, persistencia, interfaz, pruebas, distribución y distancia respecto a la referencia de SumatraPDF. Esta entrega modifica documentación e inventario; **no corrige todavía los defectos descritos**.

Esta auditoría conserva el estado histórico de 0.2. Los cambios posteriores y los riesgos que siguen abiertos se registran en [0.3.0](IMPLEMENTATION_PROGRESS.md) y [0.3.1](IMPLEMENTATION_PROGRESS_0_3_1.md).

## Conclusión

CrabPDF ya tiene una base de lectura útil: interfaz propia, pestañas superiores, biblioteca, render por regiones, lectura continua, búsqueda, formatos básicos y marcas de texto exportables. Conviene continuar sobre ella, con cambios incrementales.

La 1.0 todavía requiere mucho más que habilitar extensiones. Faltan funciones completas —impresión, edición avanzada, formularios, integraciones— y cerrar comportamientos de lectura, persistencia y recuperación. También faltan ejecución real en Linux/macOS, Windows 11 y Windows 10 de 32 bits, y mediciones que respalden «rápida y ligera».

La primera ruta debe asegurar que abrir, marcar, cambiar de ventana y exportar conserven resultados coherentes. Añadir funcionalidades encima de los problemas de sincronización haría más costosa su corrección. Después se amplía la paridad por familias, manteniendo validación por plataforma desde el comienzo.

## Base y grado de evidencia

- **Referencia fija:** [`3316ad49c5c24f6f78a3849a28eea04517baa833`](https://github.com/sumatrapdfreader/sumatrapdf/tree/3316ad49c5c24f6f78a3849a28eea04517baa833), registrada en [parity/reference.json](parity/reference.json). Es una revisión de desarrollo, no la versión 3.5.2 mostrada en la captura del usuario. No se actualizó la referencia durante esta auditoría.
- Se contrastaron fuentes fijadas de [comandos](https://github.com/sumatrapdfreader/sumatrapdf/blob/3316ad49c5c24f6f78a3849a28eea04517baa833/src/shared/Commands.h), [menús](https://github.com/sumatrapdfreader/sumatrapdf/blob/3316ad49c5c24f6f78a3849a28eea04517baa833/src/shared/MenuDefs.cpp), [atajos](https://github.com/sumatrapdfreader/sumatrapdf/blob/3316ad49c5c24f6f78a3849a28eea04517baa833/src/shared/Accelerators.cpp), [despacho de motores](https://github.com/sumatrapdfreader/sumatrapdf/blob/3316ad49c5c24f6f78a3849a28eea04517baa833/src/shared/EngineCreate.cpp), `DisplayModel.h`, `DocController.h` y [notas de plataformas](https://github.com/sumatrapdfreader/sumatrapdf/blob/3316ad49c5c24f6f78a3849a28eea04517baa833/src/ng/README.md). Copias de consulta en `artifacts/audit/upstream/`.
- Se inspeccionaron módulos Rust, scripts de preparación/empaquetado, CI, pruebas, ejemplos e informes locales. Al comenzar, los **85 archivos** del manifiesto histórico `artifacts/validation/source-files.json` coincidían por SHA-256. Ese informe describe la entrega anterior; los documentos de esta auditoría son posteriores.
- El informe anterior `artifacts/validation/release.json`, de **2026-10-14 01:49 UTC** —todavía 13 de octubre en México—, registra **17 comprobaciones, 0 fallos y 124 capturas**, en Windows 10 x64, build 19045.
- Los logs anteriores registran por arquitectura **19 pruebas unitarias, 7 de PDF nativo y 5 de formatos/marcas**, todas correctas. X86 se ejecutó bajo WOW64 en el mismo equipo x64. No son pruebas nuevas realizadas en esta auditoría.
- Aquí se comprobaron también el inventario de comandos y el árbol de features de Cargo, sin reconstruir la aplicación. Los escenarios de fallo siguientes se deducen de rutas concretas del código; **no se presentan como reproducciones gráficas nuevas**. Sus pruebas de regresión aún deben escribirse.

No se puede asignar un porcentaje fiable de avance. El inventario inicial de 332 identificadores no incluye todos los diálogos, preferencias, argumentos, estados o gestos. Tampoco se ejecutó una comparación completa contra un binario de SumatraPDF de esa revisión.

## Lo que conviene conservar

| Decisión actual | Por qué ayuda |
| --- | --- |
| Rust con egui/eframe y adaptador PDFium | Coincide con el planteamiento original de implementación independiente usando bibliotecas existentes. No hace falta comenzar otro motor PDF. |
| Modelos de documento/estado/geometría fuera de egui | Permiten comprobar navegación y coordenadas sin una ventana ni SDK nativo. |
| Un dueño de PDFium | Evita compartir indiscriminadamente objetos nativos entre hilos. |
| Tiles visibles, overview y precarga de vecinos | Reduce trabajo y mantiene continuidad mientras llega resolución final. La precarga **ya existe**. |
| Cachés por bytes y límites inferiores en x86 | Es una base mejor que contar páginas sin considerar su tamaño. |
| Importaciones explícitas en otro hilo y cola acotada | La apertura normal de otros formatos no necesita congelar el render PDF; hay una excepción en portadas, F01. |
| Escrituras temporales y rechazo del original como destino de copia | Protege documentos durante exportación y fallos de escritura. |
| Marcas locales con bloqueo y actualización antes de editar | Ya contempla concurrencia básica; quedan exportación e invalidación, F02/F03. |
| Fixtures y pruebas con PDFium real | Comprueban CropBox, giros, búsqueda, quad points y salida exportada; no solo mocks. |
| Paquetes con hashes, fuentes y avisos de herramientas | Hay trazabilidad útil para consolidar la distribución. |

## Distancia funcional respecto a V1

«Parcial» significa que existe una función relacionada, no que todos sus detalles coincidan. «Pendiente» no presupone que cada función deba cargarse al arrancar: puede implementarse bajo demanda.

| Familia | Estado comprobable en el código | Trabajo que falta |
| --- | --- | --- |
| Abrir/archivos | Varias rutas, selector, arrastrar, recargar, recientes, copias y errores básicos. | Detección por contenido además de extensión, abrir sin historial, navegación de carpeta, archivos movidos/red/temporales, renombrar/eliminar, adjuntos y documentos embebidos; cancelación y diagnóstico por fase. |
| Pestañas/ventanas | Inicio, duplicar, reordenar, cerrar por grupos de posición, reabrir, nueva ventana. | Grupos guardados, MRU completo, colores, sesiones entre ventanas, traspaso de estado y resolución de cambios concurrentes. |
| Lectura/geometría | Página, enfrentadas, libro, continuo, manga, rotación, pan, vertical al ancho. | Persistir el punto exacto, márgenes/área visible, ancho uniforme, autorrotación, grids/guías, scroll automático/barra de lectura y detalles de presentación blanca/negra/puntero. |
| Zoom | 8–6400 %, personalizado, real, página/ancho/alto y ancla básica. | Presets fraccionarios, contenido/visible/selección, reducir para caber, ajuste por orientación; comprobar cursor, teclado, trackpad y DPI mixto. |
| Texto/búsqueda | Ratón, doble clic, página actual, copiar, mayúsculas/palabra completa y resultados progresivos. | Selección entre páginas y por teclado, texto bidireccional/ligaduras/CJK, conservar contexto, selección como imagen, copiar enlaces/comentarios/imágenes y búsquedas independientes entre pestañas. |
| Índice/miniaturas/favoritos | Paneles básicos y saltos. | Árbol expandible por nivel, foco/navegación, edición/ordenación, página actual, generación de índice y búsqueda de anotaciones. |
| Anotaciones | Resaltar/subrayar con colores, persistir, deshacer la última y exportar PDF. | Seleccionar/editar/borrar marcas existentes, mostrar/ocultar, panel, rehacer, cortar/copiar/pegar; notas, texto libre, líneas, figuras, tinta, sellos, enlaces y adjuntos. |
| Edición PDF | Exportación de copia con marcas. | Insertar texto/imágenes, operaciones de páginas, combinar/extraer, cifrar/descifrar, comprimir, exportar texto/imágenes, aplanar y redacción efectiva. Distinguir una marca de redacción de eliminar realmente el contenido. |
| Formularios/firmas | Rasterización pasiva del documento. | Interacción, foco, valores/guardado y resaltado de campos; firma visual y firma/certificación criptográfica son funciones diferentes. Verificar el comportamiento exacto de referencia. |
| Impresión | No hay implementación. | Diálogo, impresora, copias, rangos, selección, orientación, escala, errores/cancelación y CLI; integración nativa en cada sistema. |
| Menús/atajos/preferencias | Menú, contexto, paleta, atajos fijos y TOML básico. | Argumentos/prefijos, personalización, opciones avanzadas, estados habilitados, foco, accesos por teclado, traducciones y ayuda completa. |
| Accesibilidad | Algunos controles estándar de egui; PDF dibujado en canvas. | Árbol semántico del documento y de controles personalizados, navegación/caret, lectores de pantalla, alto contraste y pruebas reales. Una dependencia AccessKit no demuestra integración completa. |
| Integración con sistema | Abrir/revelar archivo y CLI básica. | Asociaciones, impresión CLI, instancia existente, SyncTeX directo/inverso, visores externos, preview/búsqueda en Windows y equivalentes por plataforma. |
| Funciones recientes de la referencia | Sin implementación funcional en CrabPDF de estas familias. | Lectura en voz alta, herramientas externas de búsqueda/traducción/IA, acciones configurables y scripts PDF. Inventariar los disparadores y controles; optativas, sin envío o ejecución automáticos. |
| Distribución | Portable Windows x64/x86 de desarrollo. | Instaladores, desinstalación/migración, `.app`, Linux, firmas/notarización según canal, actualización controlada, pruebas en máquinas limpias y bibliotecas de la ABI correcta. |

Las funciones recientes están en la revisión ya fijada; no se proponen como ampliación arbitraria ni se posponen a plugins V2. Los comandos internos de diagnóstico deben clasificarse por su exposición real: `CmdDebugCrashMe` o marcadores de rango no son funciones de lectura que deban incorporarse a la interfaz pública.

### Formatos: aceptar una extensión no basta

| Familia | Ruta actual | Brecha de fidelidad/cobertura |
| --- | --- | --- |
| PDF y AI con PDF | PDFium directo. | Corpus más amplio: fuentes, transparencias, capas, color, cifrado moderno, permisos, formularios, adjuntos y archivos dañados. Determinar las opciones de referencia relacionadas con estos elementos. |
| EPUB/FB2/MOBI/Kindle/PalmDOC | Preparación local a PDF con MuPDF/libmobi y metadatos. | Reflujo, tamaño/familia de letra, CSS, imágenes/tablas, notas/enlaces y anclas estables; más variantes por familia. Sin eliminación de DRM. |
| DjVu | DjVuLibre; OCR existente recuperado. | Documentos indirectos, índices/enlaces y corpus mixto; no hay OCR nuevo de imágenes. |
| XPS/OXPS/XOD/DWFX | Conversión local. | Tipografías, enlaces, color y documentos complejos; validar cada variante. |
| CHM/HTML | Extracción/preparación a PDF. | Semántica de navegación de ayuda, enlaces, índice/búsqueda/recursos y caracteres. Convertir páginas no equivale a un visor CHM completo. |
| CBZ/CBR/CB7/CBT y archivos de imágenes | Extracción individual validada y orden natural. | Variantes RAR/TAR/7z, páginas grandes, metadatos, carpetas, contraseñas si la referencia las admite, memoria y apertura progresiva. |
| Imágenes/TIFF/OpenRaster | Normalización/conversión; TIFF multipágina, ORA compuesto. | Variantes, orientación y color. GIF/ICO se muestran como imagen decodificada; cotejar animación/múltiples imágenes con la referencia antes de definir paridad. |
| Texto/Markdown | Paginación fija. | Codificaciones, recursos locales, tipografía y navegación; evitar depender de recursos remotos. |
| Formatos adicionales | No declarados en 0.2. | Completar el despacho fijado: PS/EPS, DVI, LIT, TCR, formatos Office/imágenes adicionales que admita el motor de referencia y carpetas de imágenes. Enumerar extensiones reales, opciones y dependencias antes de prometerlos. |

La lista exacta de extensiones implementadas y sus límites permanece en [FORMATS_AND_ANNOTATIONS.md](FORMATS_AND_ANNOTATIONS.md). Las muestras existentes cubren familias; no acreditan todas sus variantes ni todos los documentos reales. Abrir PDF no necesita los decodificadores de libros; mantener esa independencia.

## Hallazgos de código priorizados

**Alta:** puede perder coherencia de datos o bloquear una operación central. **Media:** limita robustez, mantenimiento o fidelidad y debe resolverse para V1. Las líneas son orientativas para la base 0.2; la función citada es el localizador estable. Las rutas Rxx se desarrollan en [V1_PLAN.md](V1_PLAN.md) y [V1_BACKLOG.csv](V1_BACKLOG.csv).

### F01 — Alta: las portadas pueden bloquear al dueño de PDFium

**Evidencia:** `src/worker.rs`, rama `Command::Cover`; `src/library.rs:14`, `load`, abre mediante `engine.open(path)` si no hay PNG; `src/engine.rs:71` llega a `formats::prepare`. La cola prioriza render, pero una conversión ya iniciada por esta ruta es síncrona en el mismo hilo.

**Escenario:** Inicio pide portada de un EPUB grande con caché fría; el usuario vuelve al PDF abierto y desplaza. El render espera al importador externo aunque la apertura explícita de EPUB normalmente use otro hilo.

**Corrección/aceptación:** R04. Encauzar preparación de portada por el servicio de importación y enviar al dueño de PDFium solo un documento preparado. Probar con un ayudante lento controlado: un render visible debe completarse antes de liberar esa importación. No basta probar orden de tareas aún no iniciadas.

### F02 — Alta: exportar puede omitir marcas guardadas en otra instancia

**Evidencia:** `src/engine.rs:236`/`:262` bloquean y refrescan antes de añadir/deshacer. `save_copy` en `:618` no refresca. `src/platform.rs`, `new_window`, crea un proceso separado; la sincronización del worker solo recorre sus documentos locales.

**Escenario:** A y B abren el mismo archivo. A marca; B exporta sin editar o recargar. Su documento nativo todavía representa el estado anterior del almacén.

**Corrección/aceptación:** R01. Definir una instantánea consistente para exportar y refrescar bajo el protocolo de revisión/bloqueo. Dos instancias deben producir una copia con las marcas confirmadas antes de empezar la exportación; probar además conflicto y fallo de escritura sin dañar el destino anterior.

### F03 — Alta: la invalidación de marcas es de una sola página

**Evidencia:** `src/worker.rs`, `Command::Mark`/`UndoMark`, invalidan la página de la operación. `refresh_marks` puede actualizar muchas páginas. `src/ui/mod.rs:913` también invalida solo la página del evento; `RenderKey` (`src/document.rs:83`) no contiene revisión del contenido.

**Escenario:** B conserva un raster de página 1; A añade una marca allí; B añade otra en página 2. El refresco incorpora ambas al documento, pero B puede seguir mostrando la página 1 cacheada sin la marca. Para duplicados locales se limpia toda la caché del worker, pero la UI sigue recibiendo solo una página.

**Corrección/aceptación:** R02. Emitir páginas afectadas o una revisión de contenido e invalidar de forma coherente CPU/GPU/resultados pendientes. Regresión de dos páginas, dos instancias y pestaña duplicada; incluir deshacer y respuestas anteriores a la edición.

### F04 — Alta: escritura atómica de preferencias sin reconciliación

**Evidencia:** `src/config.rs`, `Preferences::load/save`, leen y escriben todo el TOML. `src/ui/mod.rs:576`, `save_preferences`, guarda el estado completo de esa instancia. No hay bloqueo ni fusión entre ventanas.

**Escenario:** dos ventanas parten del mismo archivo de preferencias, modifican favoritos/recientes y cierran en distinto orden. La última escritura puede eliminar cambios de la otra. La operación también sincroniza disco en el hilo de UI.

**Corrección/aceptación:** R03. Revisiones y reconciliación por tipo de dato, sesiones identificadas por ventana, escritura diferida fuera de la UI. Probar intercalaciones, cierre/crash y disco no disponible. Un mutex dentro de un proceso no resuelve dos procesos.

### F05 — Media: `--config` no determina todas las rutas del perfil

**Evidencia:** `ReaderApp::with_launch` usa `options.config`; `src/formats.rs:107`, `data_directory`, consulta variables/ruta global en lugar de ese perfil; `new_window` no transmite `--config`. Portadas, importaciones y marcas pueden quedar bajo raíces distintas.

**Corrección/aceptación:** R03. Resolver una estructura `AppPaths` al arrancar y propagarla a worker, importación y nuevas ventanas. Probar dos perfiles CLI con los mismos documentos; no deben mezclar datos. Mantener overrides explícitos y compatibilidad de rutas antiguas.

### F06 — Alta: las marcas no versionan el contenido preparado

**Evidencia:** `src/annotations.rs:11` usa un fingerprint de ruta/tamaño/mtime; almacena `TextMarkup` por página e índices de caracteres. El esquema no registra la versión de paginación/importador. `src/formats.rs` versiona la caché de PDF, pero no el ancla de la marca.

**Escenario:** una actualización cambia la paginación de un EPUB sin cambiar el archivo original; la marca puede reaplicarse sobre otros caracteres válidos. Mover/renombrar el archivo también cambia la asociación actual.

**Corrección/aceptación:** R07. Versionar documento preparado y almacén; anclas con identidad verificable y contexto del texto. Para libros con reflujo, ubicación lógica de origen. Migrar con copia de seguridad; conservar como no resuelta una marca ambigua. Probar actualización del importador, renombrado y modificación externa.

### F07 — Alta: caché de importaciones sin transacción completa

**Evidencia:** `src/formats.rs:144`, `prepare`, reutiliza PDF por cabecera/tamaño (`valid_pdf:282`), publica sidecars mediante copia y el PDF por separado; no hay bloqueo por clave entre procesos. `prune_cache:872` usa antigüedad de modificación y no conoce documentos activos.

**Riesgos:** un PDF de caché corrupto con cabecera válida se reutiliza; dos importaciones compiten sobre PDF/metadatos; un origen que cambia durante la conversión no se vuelve a comprobar. La poda no ofrece una garantía explícita de retener la entrada recién producida o en uso.

**Corrección/aceptación:** R05. Entrada transaccional con manifest/versiones, bloqueo por clave, revalidación del origen, recuperación acotada ante caché inválida y protección de entradas activas. Probar fallos entre publicaciones, dos procesos, disco lleno y límite de caché. No borrar marcas al limpiar caché.

### F08 — Alta: límites y cancelación no abarcan todo el trabajo externo

**Evidencia:** `src/formats.rs:329`, `run`, limita 180 segundos por proceso, no por importación compuesta. `try_wait` comprueba éxito antes del último control de tamaño; errores de sondeo/tamaño retornan sin un guard que mate y espere al hijo. `PdfWorker::drop` señala parada, sin unir todos los hilos ni cancelar un hijo activo. El bloqueo de anotaciones es una espera sin plazo en el worker.

**Corrección/aceptación:** R06. Guard de proceso con `kill/wait` en rutas de error, plazo total, cuotas comprobadas al finalizar, cancelación y cierre coordinado; bloqueo de almacén con espera acotada fuera del render. Probar ayudante colgado, fallo de acceso, cancelación entre etapas y cierre de la aplicación; no deben quedar procesos escribiendo temporales.

Los límites de archivo/trabajo actuales no limitan memoria RSS ni aíslan un parser nativo. Esto es una brecha de robustez; no se ha demostrado aquí una vulnerabilidad explotable.

### F09 — Media: cola llena deja una apertura sin finalizar

**Evidencia:** `src/worker.rs`, `open_with_password`, usa correctamente `try_send` sobre una cola acotada y retorna error si está llena. `src/ui/mod.rs`, `open_path`, ya creó la pestaña; su manejo `self.error(error)` no limpia `state.opening`. `viewer.rs` muestra «Abriendo documento…» mientras ese campo exista.

**Corrección/aceptación:** R04. Toda solicitud termina en éxito, fallo o cancelación explícitos, incluso si nunca entró en cola. Probar saturación determinista de la cola, permitir reintento y cerrar sin estados colgados.

### F10 — Media: metadatos funcionales codificados como texto de presentación

**Evidencia:** `src/ui/mod.rs:774–787` busca «Aviso de anotaciones», «Marcas locales» y «Título» dentro de `properties: Vec<(String,String)>`. El número de marcas y avisos dependen de etiquetas en español.

**Corrección/aceptación:** R08. Campos tipados (`title`, `annotation_count`, `warnings`, capacidades); generar etiquetas solo al presentar. Cambiar el idioma no debe cambiar disponibilidad ni estado de una acción.

### F11 — Media: coordinación demasiado concentrada

**Evidencia:** `ui/mod.rs` tiene aproximadamente 1 877 líneas; `formats.rs` 1 022, `ui/panels.rs` 931 y `engine.rs` 921 en esta base. Despacho de acciones, teclado, eventos, diálogos y persistencia se acumulan en el coordinador. El trait `PdfEngine` solo representa apertura/render; el worker usa capacidades concretas de PDFium para el resto.

**Corrección/aceptación:** R08/R09. Extraer responsabilidades al tocarlas: sesión/pestaña, entrada, metadatos/capacidades e importadores. Mantener dependencias dirigidas y pruebas existentes. No justificar una reescritura por tamaño de archivo ni construir un framework universal de motores sin casos reales.

### F12 — Media: recuperación de lectura incompleta

**Evidencia:** `StoredDocument` en `src/config.rs` conserva página, zoom, layout, continuo, giro y colores; no conserva ancla de scroll ni manga. El estado de navegación tampoco representa completamente una región visitada.

**Corrección/aceptación:** R10. Esquema versionado de punto de lectura normalizado, estado por documento/ventana y política para archivos cambiados. Reiniciar en mitad de una página larga debe volver al mismo lugar con DPI/tamaño de ventana distinto.

### F13 — Media: búsqueda global y geometría lineal necesitan pruebas de escala

**Evidencia:** `Pending.search` en `src/worker.rs` es un único `Option`; comenzar otra búsqueda sustituye la anterior. El planificador atiende búsqueda después de render/precarga/portadas. `PageLayout::build` construye todas las posiciones continuas y `viewer.rs` realiza búsquedas lineales; posiciones acumuladas usan `f32`.

**Implicación:** faltan garantías de progreso al cambiar pestañas o bajo carga sostenida. La geometría de libros extremos puede consumir tiempo y perder precisión. No se ha medido cuánto limita el uso real.

**Corrección/aceptación:** R11/R26. Definir trabajos por documento/generación o pausa explícita y reanudación; medir antes de introducir índices de rangos/prefijos y acumulación de mayor precisión. Pruebas con dos búsquedas, precarga sostenida, páginas mixtas y libros muy largos. No reemplazar la caché LRU solo por su complejidad teórica.

### F14 — Media: límites de lectura y diagnósticos insuficientes

**Evidencia:** `annotations::load` limita a 4 MiB **después** de `read_to_string`; preferencias se leen completas. `formats::run` clasifica protección por palabras en logs y llama «falta el decodificador» a cualquier fallo de spawn. El enum de errores conserva categorías centradas en PDF.

**Corrección/aceptación:** R06/R08. Lecturas acotadas desde el stream, errores estructurados por fase/proveedor y progreso cancelable. Diferenciar ausencia, permisos, formato corrupto, incompatibilidad, DRM, cuota y disco. Probar fixtures grandes/malformados y causas de salida controladas, sin volcar contraseñas o contenido privado en logs.

### F15 — Media: texto accesible e internacionalización aún incompletos

**Evidencia:** etiquetas y atajos están dispersos; la selección trabaja con caracteres y reglas básicas alfanuméricas. `eframe` se configura sin features por defecto. El árbol consultado contiene `accesskit` a través de egui, pero eso no acredita el adaptador nativo ni un árbol semántico del PDF; no hay pruebas de lectores de pantalla en la evidencia actual.

**Corrección/aceptación:** R12/R14. Auditar features/adaptadores reales, exponer semántica y navegación, catálogo de mensajes y atajos por plataforma, tratamiento de clusters y orden de lectura. Probar NVDA, VoiceOver y Orca con foco, selección, cuadros de diálogo e informe de errores; incluir español/inglés, RTL/CJK y DPI alto.

### F16 — Media: carga de herramientas y empaquetado ligados al desarrollo

**Evidencia:** `formats::tool` incluye `CARGO_MANIFEST_DIR` y después PATH; la búsqueda PDFium admite directorio de trabajo/cargador del sistema. El portable x64 depende de ayudantes x86. El empaquetador reutiliza un directorio de destino; su manifest puede incluir residuos de una preparación anterior.

**Corrección/aceptación:** R24/R25/R29. Separar descubrimiento de desarrollo y release, verificar versión/ABI/procedencia, preparar staging limpio y comprobar contenido esperado además de hashes. Lanzar desde otra carpeta en una máquina sin SDK/PATH de desarrollo. Probar instalación, actualización y retirada sin tocar documentos del usuario.

### F17 — Media: cobertura de validación menor que la matriz de producto

**Evidencia:** `cargo test` normal omite las 12 pruebas nativas. `Test-Local.ps1` sí las ejecutó en Windows. `.github/workflows/ci.yml` prepara otros targets, pero no se ejecutó; formatos/marcas están condicionados a Windows y no incluye recorridos gráficos. `Cargo.toml` declara MSRV 1.95 mientras el toolchain probado es 1.96.

**Corrección/aceptación:** R00/R24/R25/R27. Separar suites independientes de Windows, fixtures reproducibles y comandos locales equivalentes en cada SO; verificar MSRV o corregir la declaración. Registrar host/target/runtime/SDK/hashes. Obtener evidencia real, sin necesitar un remoto para empezar.

### F18 — Media: el inventario se había quedado detrás del código

**Evidencia:** antes de esta auditoría, `commands.csv` tenía 74 parciales, 253 pendientes y 5 referencias internas. Resaltado, subrayado, exportación de marcas y varias acciones ya existentes seguían como pendientes; `cases.md` decía que anotaciones y otros formatos no estaban implementados.

**Corrección documental realizada:** se relacionaron **9 entradas adicionales** con código/evidencia: guardar anotaciones en copia, vertical al ancho, olvidar reciente, resaltar, subrayar, ajustar al alto, deshacer marcas, Inicio y seleccionar página. Quedan **83 parciales, 244 pendientes y 5 referencias internas; 0 en paridad verificada**. La palabra «parcial» conserva sus límites.

**Trabajo pendiente:** R00. Expandir cada familia en comportamientos observables, mapear alias/argumentos y menús dinámicos. No declarar inventario completo con esta reconciliación.

## Rendimiento y tamaño: qué sabemos y qué medir

Los ZIP locales 0.2 pesan **43 482 490 bytes (x64)** y **43 145 281 bytes (x86)**; el archivo de fuentes de herramientas pesa **82 299 919 bytes**. Son cifras del informe anterior, no comparaciones con SumatraPDF ni tamaño instalado. La distribución incluye varios motores/herramientas: PDFium, MuPDF, libmobi, DjVuLibre y 7-Zip. La lista de dependencias Cargo contiene componentes de compilación y de otras plataformas; contar entradas no equivale al coste en ejecución.

No existe todavía un benchmark reproducible frente a SumatraPDF. Tampoco una medición de RAM máxima del árbol de procesos, GPU, coste de una importación fría, búsqueda en corpus real o autonomía en portátil. Por ello no se puede certificar que CrabPDF sea igual de rápido o ligero.

R26 debe medir, en el mismo equipo/corpus y con builds identificadas: arranque sin documento, primera página útil, apertura fría/caliente, importación completa, latencia de scroll/zoom/búsqueda/cancelación, CPU en reposo, RSS/commit máximo incluyendo ayudantes, texturas y tamaño instalado. Publicar mediana, p95, muestras, condiciones de caché y versión de driver. Mantener separados PDF directo e importaciones.

Primero medir; después ajustar carga bajo demanda, colas, extracción incremental o duplicación de bibliotecas. Evaluar un motor común puede ser un experimento posterior con fidelidad/tamaño/licencias comparados, no una razón para reemplazar ahora el stack autorizado.

## Pruebas y decisiones que faltan

1. Corpus con procedencia: PDF real complejo, escaneados, protegidos, dañados, fuentes RTL/CJK, libros extensos, CHM con enlaces, variantes Kindle, DjVu OCR, archivos comprimidos hostiles y tamaños extremos. Fixtures sintéticos detectan fallos concretos; no reemplazan ese corpus.
2. Regresiones de procesos/perfiles simultáneos, caché dañada y migración. Las pruebas actuales de «copias abiertas» no cubren por sí solas exportar desde otra instancia ni invalidación multipágina.
3. Manuales por plataforma para selector/portapapeles, arrastrar, IME, monitores con DPI distinto, impresoras, suspensión/reanudación, asociaciones y lectores de pantalla. Las capturas no verifican estos servicios.
4. Definir versiones mínimas de macOS y de cada distro, ABI nativa y canal de paquete. Son decisiones técnicas pendientes, no compatibilidad ya prometida por una matriz de YAML.
5. Antes de distribuir, revisar las obligaciones de cada dependencia y el contenido concreto del paquete. MIT del código propio no describe automáticamente las herramientas GPL/AGPL/LGPL incluidas. Conservar fuentes y avisos; cualquier elección de licencia necesita revisión específica, no una conclusión jurídica en esta auditoría.
6. Establecer un proceso de actualización de bibliotecas nativas y seguimiento de avisos de seguridad. No se hizo un análisis de CVE ni fuzzing de todos los parsers en este trabajo; no afirmar ausencia de vulnerabilidades.

## Orden recomendado

Primero **R01–R07** para datos, importaciones y cancelación, junto a **R00** para una referencia medible. Después **R08–R14** para contratos y lectura completa; **R15–R23/R30** para familias funcionales faltantes. **R24–R29** se inician pronto y acompañan el desarrollo: plataformas, rendimiento, robustez y distribución no deben quedar para el último día.

La secuencia detallada, archivos propuestos, dependencias y criterios de cierre están en [V1_PLAN.md](V1_PLAN.md). Esta auditoría identifica riesgos concretos y familias faltantes; no sustituye la verificación de cada comportamiento de la referencia.
