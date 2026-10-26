# Migración A02c: búsqueda y selección

Base de aplicación: 0.3.6, revisión anterior `3a3a3e8`. Código del corte: [`ea38997`](https://github.com/MHAlberto/crabpdf/commit/ea38997507fbe73150b302703b735771bce241e3). Continúa [A02b](PHASE_2.md) y el [plan de migración](../ARCHITECTURE_MIGRATION.md).

## Propiedad y efectos

| Componente | Responsabilidad |
| --- | --- |
| `application/search.rs::SearchSession` | Reinicio por generación, snapshot de solicitud, aceptación/progreso de lotes, límite de coincidencias y avance circular |
| `SearchIdentity`, `SearchDestination`, `SearchUpdate` | Datos de cancelación y destino/revelado; sin reloj, worker, GUI ni handles |
| `application/selection.rs::SelectionState` | Rango por página, palabra, extensión del arrastre y selección completa pendiente por documento/página |
| `ReadingTab` | Aporta identidad, página y número de páginas; cancela selección pendiente al navegar; usa la misma ruta para texto cacheado y asíncrono |
| `ui/tab_view.rs` | Traduce el destino de búsqueda a navegación y revelado, conservando la cancelación de movimiento anterior |
| `ui/mod.rs`, `ui/viewer.rs`, `ui/panels.rs` | Foco, debounce de 200 ms, eventos de teclado/puntero, geometría de pantalla, pintura y caché de contenido |
| `worker.rs` | Ejecuta solicitudes; cancela la identidad indicada, incluida una solicitud en cola o retirada de la cola antes de activarse |

No hay una segunda copia del rango o de los resultados en la UI. El visor convierte el puntero a un índice de glifo y delega palabra/arrastre al modelo. Copiar, marcar y pintar consultan el mismo rango, incluyendo arrastres en sentido inverso y glifos con índices no contiguos.

## Búsqueda

Al editar consulta/opciones se invalidan los resultados inmediatamente mediante la siguiente generación aportada por el adaptador. El modelo devuelve la identidad anterior para cancelarla. La vista conserva el temporizador y sólo construye/envía el snapshot de la consulta vigente cuando vence.

Una consulta vacía no genera una solicitud de búsqueda vacía global: cancela únicamente la identidad anterior de esa pestaña. Así no detiene una búsqueda de otro documento. Se conservan el texto de la consulta, los resultados y los atajos al ocultar la barra; ocultarla no equivale a borrar la consulta.

Los lotes se aceptan sólo para el documento/generación actuales y un progreso válido que avance dentro del número de páginas esperado. Se descartan lotes duplicados, fuera de orden y de consultas anteriores. La primera coincidencia produce un destino de navegación; los lotes siguientes actualizan resultados/progreso sin volver a saltar a la primera coincidencia.

La colección de resultados se limita antes de extenderla a 10 000 coincidencias. Alcanzarlo devuelve una única solicitud de detener esa identidad y descarta lotes posteriores. El límite corresponde al estado de aplicación; no promete limitar por sí solo la extracción nativa de todos los glifos o el resultado temporal de una página.

## Selección y correcciones

La bandera global por pestaña `select_all_pending` se sustituye por una solicitud exacta `(document_id, page)`. Escape, una selección manual y la navegación cancelan esa solicitud. Si el usuario sale de la página y vuelve, contenido antiguo no reactiva la selección. Un resultado de otra página o documento no satisface la solicitud actual.

El contenido cacheado y el que llega por el worker pasan por `ReadingTab::accept_content`. Una página vacía consume la solicitud y deja el rango vacío; un error de contenido cancela sólo su solicitud correspondiente. Las reglas de palabra conservan el criterio alfanumérico anterior, los índices de glifo y el texto Unicode.

Se corrigen tres comportamientos además de mover código:

- Contenido tardío podía volver a seleccionar después de Escape o de navegar; ahora debe corresponder a una solicitud aún vigente.
- Borrar la consulta enviaba una búsqueda vacía que cancelaba globalmente la búsqueda del worker; ahora se detiene sólo la identidad anterior.
- `stop_search` sólo retiraba el trabajo ya activo; ahora también retira la solicitud en cola y recuerda la generación cancelada si la solicitud acaba de salir de ella. Ese registro se limpia al procesar el cierre del documento.

La cancelación nativa sigue siendo entre páginas: una llamada de PDFium que ya está ejecutándose puede terminar y emitir un lote, que el modelo descarta cuando corresponde. Se conserva una búsqueda nativa activa a la vez; A03 revisará la planificación/equidad sin introducir handles nativos en aplicación.

Las acciones Primera/Última pasan por el mismo adaptador de navegación que los demás destinos, para cancelar también la espera de selección y el movimiento anterior. Historial y cambios de página por scroll cancelan la selección pendiente.

## Validación

Aprobados localmente en Windows 10 x64 con Rust 1.96.0 y PDFium 7881:

- Formato y Clippy con `--all-targets -D warnings`.
- 33 pruebas unitarias y 45 de integración, incluidas las nativas normalmente ignoradas: 78 pruebas únicas. Los helpers de procesos hijos pertenecen a su prueba de integración.
- Trece contratos nuevos en `tests/search_selection_contract.rs`: generaciones/IDs, consulta vacía, progreso, límite, avance circular, duplicación de pestañas, Unicode, arrastre inverso y contenido tardío/cancelado.
- Una prueba nueva del mailbox simula cancelación antes/después de retirar la solicitud, trabajo activo, otra pestaña y limpieza del registro al cerrar.
- `search_smoke` ampliado, ejecutado en debug: coincidencia visible al final de una página alta, vaciado de consulta mediante teclado y sustitución rápida por la consulta más reciente.

Comando de regresión:

```powershell
$env:RUST_READER_PDFIUM = "$PWD/native/pdfium.dll"
$env:CRABPDF_DATA_DIR = "$PWD/artifacts/architecture-phase3/checks/data"
cargo test --locked --offline --target x86_64-pc-windows-msvc --lib --tests -- --include-ignored --test-threads=1
```

El ejecutable principal, el instrumento y los tres ejemplos gráficos se reconstruyeron con el perfil release normal (ThinLTO). Aprobaron también:

- `search_smoke` ampliado en release, con entrada de teclado real en egui.
- `transition_smoke` normal/reducido, incluyendo selección, resaltado y navegación interrumpida. Se revisaron capturas de selección y marca.
- `document_chrome_smoke` con barra propia/nativa: 50 capturas y seis muestras Win32 con el cliente ocupando el monitor completo.

La [evidencia funcional](evidence/a02c-windows-validation.json) contiene conteos, hashes y geometría. La [CI del código ea38997](https://github.com/MHAlberto/crabpdf/actions/runs/40021230402) terminó correctamente en Windows x64/x86, Ubuntu x64 y macOS Intel/ARM. La CI ejecuta compilación y pruebas sin ventana; los recorridos gráficos descritos aquí son locales de Windows x64.

## Recursos

La [evidencia de recursos](evidence/a02c-measurements-windows-x64.json) conserva siete series completas (29 procesos): referencia previa, primera extracción/control, confirmaciones de cinco procesos y cinco pares alternados. También conserva tres informes de tiempo de una confirmación interrumpida por el observador, sin inventar las muestras de recursos que no llegaron a escribirse. Los ejecutables de referencia se preservaron antes de editar código y se verificaron por hash.

Las series consecutivas mostraron mucha dispersión. La confirmación por grupos dio medianas de apertura PDF de 75.97 ms para la referencia y 129.14 ms para la extracción; la reapertura pasó de 75.47 a 174.84 ms. Por ello se alternaron los mismos ejecutables, invirtiendo también el orden dentro de los pares pares:

| Operación, mediana de cinco procesos en ms | Referencia alternada | Extracción alternada |
| --- | --- | --- |
| Apertura PDF | 102.19 | 83.01 |
| Arranque hasta disponibilidad | 442.60 | 348.46 |
| Página precargada | 27.11 | 22.43 |
| Última página sin precargar | 47.15 | 37.98 |
| Scroll con estabilidad de 80 ms | 96.22 | 97.18 |
| Reapertura en el mismo proceso | 76.82 | 62.34 |
| EPUB sin caché interna | 195.09 | 145.28 |

La comparación alternada no reproduce la ralentización anterior, pero tampoco demuestra una mejora de velocidad. Se mantienen ambas observaciones y sus muestras; no se acredita una regresión estable ni una ganancia estadística con este entorno. La evaluación completa de latencia sigue abierta en A00.

El ejecutable principal pasa de 11 397 120 a 11 408 896 bytes (+0.10 %). El pico mediano de working set en los pares fue 118.25 MiB en la referencia y 118.56 MiB en la extracción (+0.26 %), dentro del umbral provisional del 5 %. Los intervalos de reposo tuvieron actividad irregular de frames/CPU en ambos ejecutables, incluso en la referencia; se conservan todos los intervalos y no se usa una mediana cero como garantía de reposo.

Se corrigió además una carrera del observador: si `phase.txt` está ocupado mientras el instrumento lo escribe, se omite esa muestra de recursos en vez de abortar el recorrido. La captura sigue exigiendo al menos dos muestras de CPU válidas. `-Runs 1` permite construir pares alternados; una ejecución individual no es evidencia estadística.

Las capturas, logs y reportes completos quedan en `artifacts/architecture-phase3`. Siguen los límites de [MEASUREMENTS.md](MEASUREMENTS.md): Windows x64, caché del sistema sin vaciar, disponibilidad tras enviar UI, sin medir compositor físico, conversores ni asignación GPU.

## Continuación

A02 queda implementado dentro del paquete actual: favoritos (A02a), pestañas/restauración (A02b) y búsqueda/selección (A02c). Su independencia de compilación respecto a GUI/SDK sigue pendiente de A04/A05.

La continuación de protocolo y errores por generación se implementó en [A03a](PHASE_4.md). Al cerrar A02c, se preparó el siguiente alcance de A03: separar el protocolo de aplicación y las responsabilidades del worker, manteniendo la fachada pública, el único propietario de PDFium, los presupuestos de caché y el orden de guardado. En esa revisión se identificaron dos límites: los errores de búsqueda se notificaban mediante un error genérico de detalles y el payload de portadas dependía de `library::Cover`. Ambos se resolvieron en A03a antes de la extracción de paquetes.

Debe probar resultados tardíos, cancelación, equidad y cierre con canal lleno antes de modificar el ciclo de vida de los hilos. Los adaptadores de almacenamiento, importación y motor se extraerán en cortes separados.

La versión de desarrollo sigue siendo 0.3.6. El corte no publica instaladores ni completa por sí solo la versión 1.0.
