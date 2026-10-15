# CrabPDF 0.3.3 — pantalla completa y navegación por teclado

Implementación: 14 de octubre de 2026; validación final: 15 de octubre. Esta revisión corrige los fallos de lectura reportados y avanza parcialmente R13 y R22 del [plan V1](V1_PLAN.md). Continúa [0.3.2](IMPLEMENTATION_PROGRESS_0_3_2.md). La paridad completa y las demás plataformas siguen pendientes; no se creó repositorio remoto.

## Cambios

- **Pantalla completa:** F11 oculta título/pestañas, menú, herramientas, estado y panel lateral mientras se lee un documento. Al acercar el puntero al borde superior aparece una barra flotante con navegación, página, ajuste al ancho, búsqueda, panel, comandos y salida. Se oculta al alejar el puntero y no reduce el área del documento. Los avisos de error y la búsqueda solicitada siguen disponibles.
- **Espacio de lectura:** los márgenes exteriores pasan de 24 a 4 puntos lógicos; la separación horizontal entre páginas enfrentadas pasa de 16 a 8. Las barras de desplazamiento flotan sobre el lienzo. Se conserva el zoom elegido; el usuario puede ajustar al ancho desde los controles. Los márgenes internos que forman parte del PDF se conservan.
- **Restauración:** F12 abre o cierra el panel temporalmente en pantalla completa. Al volver a ventana reaparece su visibilidad anterior. F11 sale también desde F5 cuando la presentación comenzó en una ventana normal; Escape sigue cerrando primero la búsqueda o el diálogo activo. F5 conserva zoom y modo, y Escape vuelve al estado de pantalla anterior a la presentación.
- **Flechas:** un botón con foco ya no bloquea los atajos del lector. Izquierda/Derecha y H/L pasan de página cuando el pliego actual cabe al ancho, incluso con ajuste al alto o zoom porcentual. Si está ampliado, desplazan horizontalmente; otra pulsación al alcanzar el borde cambia de página. Campos de texto, menús y diálogos conservan sus teclas de edición/selección.

Se reutilizan las acciones e iconos vectoriales existentes. La geometría compacta permanece en `layout.rs`, independiente de egui/PDFium; no hay cambio de framework, motor, dependencias ni presupuestos de caché.

## Regresión gráfica

`examples/refinement_smoke.rs` se amplió de 21 a 46 escenas. Además de los recorridos anteriores comprueba:

- foco de botón con Tab, navegación de ida/vuelta al 50 % y ajuste al alto;
- desplazamiento horizontal de ida/vuelta al 200 %;
- edición real de la consulta: escribir `reader`, mover el cursor a la izquierda e insertar `X` produce `readeXr` sin cambiar de página;
- coincidencia del viewport del documento con el de la ventana en pantalla completa, incluida presentación;
- aparición/ocultación de los controles al mover el puntero, navegación por flechas en pantalla completa y apertura/cierre del panel con F12;
- restauración del panel al volver a ventana y salida con F11 desde presentación iniciada en ventana normal.

## Validación

`cargo fmt --all -- --check`, `cargo clippy --locked --offline --all-targets -- -D warnings` y `cargo test --locked --offline` pasaron en Windows 10 x64. El test ordinario conserva las pruebas nativas ignoradas por defecto. [Registro de desarrollo](../artifacts/validation-fullscreen-20261014/development-checks.json). [Hashes de las fuentes, lockfile y scripts](../artifacts/validation-fullscreen-20261014/sources.json).

La ejecución del 14 de octubre se interrumpió durante la compilación x86 y no produjo informe consolidado; se conservan sus logs y capturas en `artifacts/validation-fullscreen-20261014`. El 15 se comprobó que los hashes de las fuentes seguían coincidiendo y se retomó la matriz con `scripts/Test-Local.ps1 -Architecture both -ReportRoot artifacts/validation-fullscreen-20261015`. Terminó con **17 comprobaciones aprobadas**: formato, Clippy, suite release con `--include-ignored`, compilación, CLI, búsqueda y recorridos de UI, refinamiento y formatos/marcas. Cada arquitectura produjo **25 + 46 + 16 capturas**. Se ejecutó sobre Windows 10 x64 build 19045; x86 corrió bajo WOW64. [Resultado consolidado y logs](../artifacts/validation-fullscreen-20261015/results.json).

Se inspeccionaron las capturas x64 de pantalla completa sin controles, controles flotantes, edición del cursor en búsqueda, EPUB y resaltado; en la ejecución del 15 se revisaron de nuevo los controles flotantes y la restauración del panel. En x86 se inspeccionaron la lectura limpia en pantalla completa, el avance a la segunda página con la flecha derecha y la restauración de ventana/panel. Las capturas de desarrollo también comprobaron apertura del panel en pantalla completa y su restauración en ventana.

## Paquetes locales

Se generaron con `Pack-Portable.ps1 -Architecture x64 -SkipBuild` y su equivalente x86, reutilizando los binarios release validados. `Verify-Portable.ps1` aprobó los **607 archivos de cada paquete**, incluidos los ejecutables idénticos a los recién compilados, PDFium, decodificadores, recursos y avisos. Se conservaron las versiones anteriores.

| Paquete | Tamaño completo del ZIP |
| --- | ---: |
| [Windows x64](../artifacts/packages/crabpdf-0.3.3-win-x64.zip) | 43 556 286 bytes |
| [Windows x86](../artifacts/packages/crabpdf-0.3.3-win-x86.zip) | 43 199 843 bytes |

[SHA-256, plataforma y comandos](../artifacts/validation-fullscreen-20261015/packages.json). [Registro de verificación](../artifacts/validation-fullscreen-20261015/verify-packages.log). Los conversores no cambiaron y sus fuentes correspondientes siguen en [el archivo separado](../artifacts/packages/crabpdf-0.2.0-format-tools-sources.zip), también verificado. El inventario de dependencias mantiene 47 entradas sin texto de licencia propio entre todos los targets; la revisión de avisos para distribución pública sigue pendiente, como en 0.3.2. Son paquetes locales de desarrollo, sin instalador ni firma.

## Límites

El recorrido usa eventos egui en una ventana nativa, PDFium real y preferencias aisladas. No certifica todas las combinaciones de menús, clics sobre controles, lectores de pantalla, monitores o DPI del sistema. Los avisos y paneles solicitados pueden ocupar espacio de lectura. El ajuste al ancho conserva el contenido completo del PDF; no recorta márgenes impresos. Windows 11, Windows 10 con SO de 32 bits, Linux y macOS requieren ejecución propia. Estas correcciones no cierran R13/R22 ni convierten la aplicación en 1.0.
