# Refinamiento de CrabPDF

> Documento histórico. Describe una revisión anterior. Consulta el [índice actual](../README.md) para las instrucciones vigentes.


Solicitud: 13 de octubre de 2026. Se conservan Rust, egui/eframe 0.36.2,
Glow, PDFium 7881, el worker de documentos y las preferencias TOML.
La revisión previa a los cambios está concluida. Las capturas de SumatraPDF
y CrabPDF se recibieron y confirman la barra superior integrada y la biblioteca.

Esta página conserva el diagnóstico y la entrega del 13 de octubre. La revisión [0.3.3](IMPLEMENTATION_PROGRESS_0_3_3.md) cambia F11 a lectura sin barras permanentes, reduce márgenes y corrige las flechas según foco y zoom. Los controles aparecen al acercar el puntero al borde superior; F12 abre el panel temporalmente y salir restaura su visibilidad en ventana. El recorrido actual tiene 46 escenas, con comprobaciones adicionales de lienzo, foco, desplazamiento y restauración.

## Diagnóstico inicial

| Área | Estado encontrado | Trabajo de esta etapa |
| --- | --- | --- |
| Ventana y pestañas | Título nativo y fila de pestañas independiente. | Barra superior integrada, operaciones nativas y alternativa de decoraciones nativas. |
| Inicio | Lista de cinco recientes al cerrar todos los PDF. | Pestaña Inicio, portadas, eliminación de historial, caché y generación diferida. |
| Lectura continua | Desactivada inicialmente; render hasta 80 puntos fuera del viewport. | Modo vertical ajustado al ancho y precarga limitada de páginas vecinas. |
| Zoom | Escalones 8–6400 %, vuelve al inicio de página. | Conservar región de lectura y rueda con modificador. |
| Paleta | Filtro, clic y Enter sobre primer comando habilitado. | Flechas, selección, atajos y estados de disponibilidad. |
| Calidad | Zoom/DPI en la clave, tiles visibles, imágenes lineales; redondeos independientes. | Comprobar dimensiones y alineación física, fallback temporal y escalas habituales. |
| Pantalla | F11 y F5 existentes; presentación no restaura zoom/layout. | Separar maximizar, pantalla completa y lectura; restaurar vista y posición. |
| Estado | TOML con recientes, sesión y favoritos; cachés en memoria. | Metadata y portadas de biblioteca manteniendo compatibilidad. |
| Logo | SVG suministrado con fondo blanco. | Eliminar sólo el fondo y conservar dibujo y sombra. |

Las acciones se centralizan en commands.rs y ReaderApp::execute.
Impresión, edición, firmas y otros formatos siguen siendo trabajos independientes
de v1; no se agregarán controles de esas funciones sin implementación.

## Fases y verificación

1. Transparencia y lectura vertical continua con precarga limitada.
2. Barra superior integrada y operaciones de ventana.
3. Biblioteca, historial, portadas y caché persistente.
4. Atajos, zoom, menús y paleta.
5. Calidad de raster y alineación con DPI.
6. Modos de ventana y restauración de la lectura.
7. Compilación, Clippy, pruebas funcionales x64/x86 y capturas de UI.

Se utilizan las operaciones de viewport de eframe; véase el
[ejemplo oficial de barra personalizada](https://github.com/emilk/egui/blob/main/examples/custom_window_frame/src/main.rs).
Movimiento y redimensionamiento se delegan al sistema. Windows 11, macOS
y los distintos gestores Linux requieren comprobaciones propias.

## Resultados por fase

| Fase | Resultado | Evidencia | Comprobación personal útil |
| --- | --- | --- | --- |
| Auditoría | Componentes y límites identificados antes de modificar; se revisaron ambas capturas. | Diagnóstico anterior, registro real de 74 acciones en actions.csv. | Comparar la organización con los documentos que usas. |
| Transparencia y lectura | Fondo blanco retirado; continuo para documentos nuevos, botón Vertical, cola de precarga subordinada al render visible y vista temporal. | Prueba de prioridad del worker y escena vertical-ready, con segunda página preparada antes de desplazar. | Abrir un libro largo, pulsar Vertical y recorrer varias páginas. |
| Barra superior | Inicio y PDF en la barra superior; títulos truncados, scroll de pestañas, región libre de arrastre, doble clic y controles de ventana. Se retiró la fila anterior. | Capturas de pestañas y diez documentos; maximizar/restaurar verificado con estado nativo. | Arrastrar la región libre, redimensionar desde bordes/esquinas y minimizar. |
| Biblioteca | Recientes/frecuentes, título/archivo/posición, portadas diferidas, caché PNG persistente, retirar historial y errores de portada con reintento. | Escenas library/history-removal y prueba de cache/reparación de PNG corrupto. | Reabrir la aplicación, mover temporalmente un PDF y comprobar su tarjeta. |
| Controles | Atajos compartidos, Ctrl+rueda, ancla de zoom, paleta con flechas/Enter/Escape y atajos. | Escenas zoom-anchor, palette-arrow-enter, control-wheel y keyboard-plus. | Probar + del teclado principal y numérico, y escritura dentro de búsqueda. |
| Calidad | Geometría común en f64, celda redondeada compartida entre layout y render, pintura alineada a píxeles físicos y fallback temporal limitado. | Matriz de render nativo 100/125/150/200 % con densidades 1/1.25/1.5/2; capturas con escalado de egui. | Comparar el PDF ISLP y otros documentos en los monitores que utilizas. |
| Modos (entrega original) | Maximizar, F11 con interfaz y F5 sólo documento eran estados distintos. F5 conserva zoom/layout y recuerda la pantalla completa anterior. Escape cierra primero el panel activo. F11 se revisó en 0.3.3, como se describe arriba. | Escenas históricas maximized/restored y transiciones fullscreen-interface/minimal-reading/back-to-window. | Entrar desde ventana normal y maximizada; comprobar Escape y posición de lectura. |
| Verificación | Unitarias, PDFium real, recorrido anterior y recorrido adicional, Clippy y paquetes x64/x86. | artifacts/validation/results.json y capturas por arquitectura. | Validación de Windows 11, Linux y macOS aún necesaria. |

## Alcance del diagnóstico de nitidez

La resolución ya incluía zoom y DPI. Se encontraron dos fuentes concretas de
remuestreo innecesario: celda de páginas enfrentadas redondeada en layout pero
calculada independientemente al pedir el raster, y destinos de textura ubicados
entre píxeles físicos. Se unificaron esas dimensiones y se ajustó la pintura.
PDFium ya activaba el antialiasing de texto, imágenes y trazados; se conserva.
Las claves siguen separando zoom, densidad, rotación, color y región.

No se atribuye toda posible falta de nitidez de cualquier PDF a esas causas.
El archivo ISLP de la captura no se proporcionó como fixture. Los bitmaps de
prueba verifican resolución y límites; la matriz gráfica usa escalado de egui,
no un cambio del ajuste DPI de Windows. Falta contrastar documentos reales y
monitores distintos. La precarga reduce esperas, sin prometer latencia cero al
saltar arbitrariamente por un PDF complejo.

## Particularidades de la barra

En Windows/Linux se emplea una barra propia con operaciones nativas de eframe;
en macOS se conserva el control nativo sobre un área de contenido extendida.
No se incluye una implementación específica del menú Snap al posar el ratón
sobre maximizar en Windows 11. La opción **Usar barra de título nativa** permite
volver a las decoraciones del sistema en Windows/Linux. Movimiento, bordes,
minimización y las particularidades de los gestores de ventanas necesitan
comprobación manual en cada sistema, además de las pruebas de estado realizadas.
