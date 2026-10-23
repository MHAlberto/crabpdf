# Acciones y atajos

El inventario ejecutable se obtiene de `Action::ALL`, no de etiquetas de menú.
Cada acción comparte `ReaderApp::execute` y la comprobación `enabled` entre
menús, teclado, contexto y paleta. Exportación:

```sh
cargo run --locked --offline --example command_inventory -- docs/actions.csv
```

## Estado de las funcionalidades

| Grupo | Estado y límites |
| --- | --- |
| Apertura, pestañas, Inicio y biblioteca | Implementadas y conectadas. Hasta 64 documentos, historial de 30, portadas diferidas y caché de 64 PNG. |
| Zoom, rotación y modos de página | Implementadas y conectadas. Zoom 8–6400 %, posición conservada; modo vertical ajustado al ancho. |
| Menús, contexto y paleta | Implementadas con acciones compartidas. Paleta: filtro, arriba/abajo, Enter, Escape, atajos y comandos deshabilitados por contexto. |
| Navegación, índice, enlaces y favoritos | Implementadas y conectadas; navegación y estado por documento. |
| Selección y copia | Parciales frente al alcance general de v1: selección de una página, sin selección por teclado o entre páginas. La copia respeta permisos. |
| Búsqueda | Implementada con mayúsculas/palabra completa y resultados progresivos; máximo 10 000 coincidencias y sin búsqueda avanzada. |
| Ventana, pantalla completa y lectura | Implementadas. Barra integrada con operaciones de viewport; decoraciones nativas opcionales. Las particularidades de cada gestor de ventanas requieren pruebas en ese SO. |
| Preferencias, recientes y sesión | TOML compatible con archivos anteriores; punto relativo a página y modo manga al reabrir, sesión opcional. Vista explícita CLI tiene prioridad; historial completo pendiente. Sin persistir contraseñas. |
| Formatos y marcas de texto | Apertura por formato, resaltado/subrayado, colores, persistencia local, deshacer y copia PDF. [Alcance y límites](FORMATS_AND_ANNOTATIONS.md). |
| Impresión, edición general, formularios, firmas, formatos avanzados y SyncTeX | Pendientes; no se presentan como controles funcionales. |

No se encontró una función de la lista anterior con implementación terminada
pero inaccesible desde todo el sistema de acciones. Se corrigió la ausencia de
la propia paleta en el registro general y se conectaron Inicio, lectura vertical
y maximizar/restaurar.

Los controles de página se deshabilitan en los extremos; copiar necesita una
selección y permisos; buscar siguiente necesita coincidencias; volver/avanzar
necesitan historial; guardar/zoom/rotar necesitan documento. Estas condiciones
son disponibilidad de contexto, no implementaciones provisionales.

## Mapa principal

| Acción | Windows/Linux | macOS |
| --- | --- | --- |
| Abrir / cerrar / guardar copia | Ctrl+O / Ctrl+W / Ctrl+S | ⌘+O / ⌘+W / ⌘+S |
| Inicio | Ctrl+Mayús+H | ⌘+Mayús+H |
| Paleta | Ctrl+K | ⌘+K |
| Buscar | Ctrl+F | ⌘+F |
| Zoom | Ctrl y +/−, Ctrl+rueda | ⌘ y +/−, gesto de zoom |
| Ajustar página / real / ancho | Ctrl+0 / Ctrl+1 / Ctrl+2 | ⌘+9 / ⌘+0 o ⌘+1 / ⌘+2 |
| Página anterior/siguiente | P/N; izquierda/derecha cuando se ajusta al ancho | Igual |
| Desplazar | Flechas, H/J/K/L, Re Pág/Av Pág, Espacio | Igual |
| Ir a página | Ctrl+G | ⌘+Alt+G |
| Cambiar pestaña | Ctrl+Tab / Ctrl+Mayús+Tab, incluye Inicio | Igual |
| Una / enfrentadas / libro | Ctrl+6 / Ctrl+7 / Ctrl+8 | ⌘+6 / ⌘+7 / ⌘+8 |
| Pantalla completa / lectura | F11 / F5 | Igual |
| Salir de un panel o modo | Escape: primero el panel activo, después el modo | Igual |

Los atajos de navegación y zoom no interceptan escritura en campos de texto.
Los comandos globales de apertura, búsqueda, ventanas y paneles siguen disponibles.
La paleta consume sus flechas y Enter antes del editor para evitar que muevan el PDF.

## Interfaz de lectura en 0.3.5

Los saltos cercanos en modo continuo animan el desplazamiento durante 160 ms; los largos son inmediatos. **Reducir movimiento** en preferencias desactiva las animaciones. La rueda, el ratón y una nueva navegación interrumpen el movimiento.

El visor ocupa también el espacio del antiguo pie inferior. La versión y el límite de resolución se consultan al situar el puntero sobre CrabPDF en la barra superior. Los avisos de éxito aparecen flotantes sin desplazar el documento.

Marcas agrupa resaltar/subrayar, seis muestras circulares con indicador de selección, color personalizado, deshacer y guardar copia. Requiere documento; las acciones mantienen las comprobaciones de selección/permisos. Las cruces vectoriales de pestaña y ventana disponen de áreas de clic de 24×24 y 44×30 puntos respectivamente. [Evidencia y límites](IMPLEMENTATION_PROGRESS_0_3_5.md).

## Favoritos y arrastre en 0.3.6

Favoritos muestra únicamente el documento activo. Permite guardar/quitar la página actual, recorrer páginas guardadas en orden y eliminar cada entrada. Navegar desde el panel conserva zoom, modo y giro. Los favoritos de otros libros permanecen guardados en el perfil y reaparecen al abrirlos.

Arrastrar el nombre de una pestaña fuera de la barra y soltar crea una copia en otra pestaña de la misma ventana, conservando página, zoom y punto de lectura. El original permanece abierto. Volver a la barra o pulsar Escape cancela el gesto; la zona vacía de la barra mantiene el movimiento de ventana. [Informe](IMPLEMENTATION_PROGRESS_0_3_6.md).
