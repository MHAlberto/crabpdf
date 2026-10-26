# Migración A03b: planificador y cierre

26 de octubre de 2026. Continúa [A03a](PHASE_4.md), sobre la base de código `bdf8239`. La aplicación conserva la versión de desarrollo 0.3.6.

## Corte implementado

`src/worker/scheduler.rs` concentra comandos privados, mailbox, colas, deduplicación, promoción de precarga, retención de renders, generaciones canceladas y coordinación atómica de importaciones. Sus campos son privados: el motor y el importador ya no manipulan el mutex, las colas o la condición directamente. No depende de PDFium, egui ni I/O de conversión.

`PdfWorker` conserva la fachada pública. La propiedad de PDFium, los motores abiertos, cachés, conversores y emisión de eventos siguen en el adaptador actual. No se crean crates vacíos ni una segunda implementación del planificador.

El registro/envío de una importación y su publicación final se coordinan bajo el mismo mutex que cerrar el documento. Un fallo al enviar retira el token; una finalización posterior al cierre no reabre la pestaña. El registro también rechaza un mailbox ya detenido. Se conservan la identidad por documento y el flujo de reparación.

## Política conservada

La prioridad sigue siendo:

1. Abrir/cerrar, búsqueda inicial y escrituras, en orden dentro de esa clase.
2. Render visible.
3. Contenido de texto/enlaces.
4. Render especulativo.
5. Portadas.
6. Siguiente página de búsqueda progresiva.

Una búsqueda puede quedar aplazada si nunca cesa la lectura. Es una caracterización de la política existente, no una promesa de equidad: cambiarla requiere un corte posterior con medidas de latencia. Las colas mantienen límites de 96 renders visibles y 24 especulativos. Una petición visible promueve una precarga idéntica sin duplicarla.

El cierre conserva la secuencia: cancelar importaciones y detener mailbox, desconectar solicitudes y receptor de eventos, después esperar al importador. Desconectar el receptor libera envíos bloqueados cuando el canal de dos eventos está lleno. El hilo nativo todavía no se conserva para hacer join; no se ha cambiado su ciclo de vida en esta extracción.

## Verificación

En Windows x64 y x86 bajo WOW64, aprobaron 101 pruebas únicas por arquitectura: 48 unitarias y 53 contratos/integraciones, incluidas pruebas nativas normalmente ignoradas. Los dos helpers de procesos hijos se cuentan con sus contratos, no como nuevas funciones.

Se mantienen los tres contratos previos del planificador. Cinco pruebas nuevas comprueban prioridad/orden de escrituras y reanudación de búsqueda, límites/deduplicación/promoción, importación cerrada o rechazada, parada del mailbox y cierre con canal de eventos lleno antes del join. También aprobaron formato y Clippy para todos los targets locales x64.

El recorrido nativo `interaction_smoke` terminó con 16 capturas durante selección, resaltar, subrayar, deshacer y flechas mantenidas; `search_smoke` comprobó resultado al final de página, consulta vacía y sustitución rápida. Usan la aplicación y PDFium reales con entrada simulada. No miden cada fotograma del monitor ni representan un corpus general de rendimiento.

El ejecutable principal x64 ocupa 11 473 920 bytes, frente a 11 432 448 antes del corte: +41 472 bytes, aproximadamente 0.36 %. No cambian dependencias ni presupuestos CPU/GPU/texto. No se atribuye una mejora de velocidad a mover el código; A00 sigue abierto.

Se añadieron además los [instaladores Windows](../WINDOWS_INSTALLER.md), con verificaciones separadas de instalación/desinstalación. La descarga de herramientas de formatos incorpora reintentos acotados y un espejo GNU con el mismo hash para GPL-3.0: el reintento x86 de la CI anterior volvió a fallar en esa descarga, después de compilar los conversores. Se comprobó el espejo real y se inyectaron fallos de red y hash en el helper para verificar recuperación y rechazo.

La [evidencia local](evidence/a03b-windows-validation.json) registra fuentes, binarios, paquetes, pruebas y limitaciones. Los archivos completos quedan en `artifacts/installer-a03b`.

## Siguiente corte

A03b queda implementado dentro del paquete actual. A03 completo sigue abierto. A03c separará la coordinación de importación y sus adaptadores de procesos/caché; después se extraerán almacenamiento y adaptación PDFium antes de A04/A05.

Permanecen pendientes la equidad bajo carga continua, el ciclo de vida completo del hilo nativo, mediciones comparables de A00 y evidencia gráfica Linux/macOS. Los paquetes Linux no se rediseñaron en esta entrega. La paridad SumatraPDF y la versión 1.0 conservan los criterios del backlog.

## Continuación

[A03c–A07 y workspace](PHASE_6.md) registra la extracción posterior. Las rutas `src/worker.rs` y `src/worker/scheduler.rs` de esta fase histórica corresponden ahora a `crates/runtime/src/worker/`.
