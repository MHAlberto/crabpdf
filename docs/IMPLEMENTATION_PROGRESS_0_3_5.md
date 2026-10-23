# CrabPDF 0.3.5 — navegación e interfaz

**Fecha:** 23 de octubre de 2026. Revisión de interfaz sobre [0.3.4](IMPLEMENTATION_PROGRESS_0_3_4.md). Avanza parcialmente R13, R16 y R22 del [plan V1](V1_PLAN.md); no cierra esas rutas ni declara paridad 1.0.

## Cambios y causas

- El área de lectura ya no reserva el pie de atajos/versión. La versión y el aviso de resolución limitada se consultan sobre el nombre CrabPDF de la barra superior.
- Las barras de desplazamiento flotan sobre un viewport estable, sin variar el ancho de ajuste ni dibujar degradados blancos en sus bordes. Se evita solicitar otro raster por una oscilación de ancho.
- Los saltos cercanos en lectura continua recorren la distancia en 160 ms con aceleración y frenado suaves. La animación parte de la posición visible y puede interrumpirse con rueda, ratón o una nueva navegación. Una corrección de geometría desplaza ambos extremos.
- Se descarta el objetivo cinético anterior de egui al aplicar navegación programática. Ese objetivo podía devolver la vista hacia la página anterior en el frame siguiente.
- Se precargan vistas acotadas de los pliegos vecinos también fuera del modo continuo. Un destino frío conserva brevemente la vista anterior mientras llega una vista previa del documento y revisión correctos, con un máximo de espera de 200 ms. La alternativa a un tile nítido exige la misma revisión del contenido.
- Inicio, cambios de documento y carga tienen una entrada de opacidad leve sobre un lienzo opaco. Búsqueda y panel lateral abren y cierran con la animación de panel de egui. No se mezclan los textos de dos páginas. Los saltos largos, la restauración y los cambios de zoom no recorren todo el documento.
- Marcas usa seis muestras circulares, selección con borde y check vectorial, nombre del color, selector personalizado y código hexadecimal. Resaltar, subrayar, deshacer y exportar conservan sus acciones compartidas; sin documento el menú se deshabilita.
- La cruz de ventana tiene un área de 44×30 puntos, trazo vectorial y fondo rojo al pasar el puntero. Las pestañas tienen su propia cruz de 24×24. Se dibujan también minimizar, maximizar y restaurar sin depender de glifos de una fuente.
- Los avisos de éxito se muestran flotantes durante cuatro segundos; su aparición no cambia la altura del PDF. Los errores y avisos de configuración conservan su presentación persistente.
- Preferencias incorpora **Reducir movimiento** (`reduce_motion` en TOML). Desactiva el desplazamiento y las entradas animadas sin cambiar las preferencias antiguas.

La implementación sigue en Rust/egui/eframe y mantiene el worker, PDFium, los presupuestos de caché y las herramientas nativas. No se añadieron dependencias ni un segundo framework. PDFium 7881 y los conversores MuPDF 1.28.5/libmobi 0.12 se reutilizan de los paquetes 0.3.4, comprobando los SHA-256 publicados y su procedencia; no se atribuye una recompilación de esos motores.

## Regresión reproducible

`examples/transition_smoke.rs` genera un PDF propio de tres páginas de colores y usa una ventana nativa. Envía flechas, invierte rápidamente la navegación, cambia al modo no continuo, abre Marcas, aplica resaltado mediante la acción compartida, cambia a Inicio y vuelve, abre preferencias y pulsa la cruz real de la ventana. Captura frames intermedios y registra el desplazamiento.

Comprueba que avanzar/volver no retrocedan dentro de una transición, que el modo normal tenga posiciones intermedias y el reducido salte directamente, que el lienzo llegue al borde inferior y que el centro del destino no se sustituya por un frame blanco en los saltos comprobados. Incluye dos pruebas unitarias de continuidad, inversión y ajuste geométrico.

```sh
cargo run --locked --example transition_smoke -- artifacts/transitions/normal
cargo run --locked --example transition_smoke -- artifacts/transitions/reduced --reduced
cargo run --locked --example transition_smoke -- artifacts/transitions/dark --dark
```

PDFium es necesario. Las pruebas usan perfiles y documentos aislados. El script Windows `Test-Local.ps1` incorpora los tres recorridos para futuras ejecuciones; esta entrega utilizó los comandos individuales indicados abajo, no atribuye una ejecución completa a ese script.

## Evidencia de esta entrega

Informe local: `artifacts/validation-ui-20261023/`. Se ejecutaron formato, Clippy con advertencias como errores y `cargo test --locked --offline`: 27 unitarias, cuatro de posición de lectura y la regresión de perfil entre procesos. Las suites nativas ignoradas por defecto se ejecutaron por separado cuando se indican.

| Entorno | Comprobaciones ejecutadas |
| --- | --- |
| Windows 10 x64, build 19045 | Siete pruebas PDF nativas, cinco de marcas PDF y exportación entre procesos. Recorridos de transición normal/reducido/oscuro; 46 escenas de refinamiento, 25 del visor y reapertura de lectura en tres procesos. |
| Windows x86 bajo WOW64 | Compilación de producción, siete pruebas PDF nativas y recorridos de transición normal y reducido, con clic real de cierre. |
| Ubuntu 24.04 x86_64 en Docker | Instalación del DEB con sus dependencias, comprobación de manifiestos DEB/tar, siete pruebas PDF nativas, cinco de marcas y exportación entre procesos. Ventanas con Xvfb/Mesa: transiciones normal/reducido/oscuro, 25 escenas del visor y apertura del ejecutable instalado. |

Los binarios de producción se compilaron con el perfil release habitual. Los ayudantes Linux de UI y pruebas nativas se enlazaron sin LTO entre crates para acortar su compilación; usaron la biblioteca de producción. En Windows x86 los ayudantes usan el LTO habitual: el intento de desactivarlo solo en el ejecutable no fue compatible con los objetos MSVC existentes y se repitió con el perfil normal.

## Límites

Una ventana virtual de Ubuntu no prueba un escritorio físico, Wayland, accesibilidad o todas las distribuciones. WOW64 no equivale a un sistema Windows de 32 bits. Windows 11 y la interfaz macOS requieren su propia ejecución. La CI se informa por ejecución, sin confundir compilar macOS con probar su interfaz.

No se volvió a ejecutar aquí el corpus completo de conversores/Kindle: las pruebas de marcas PDF sí se ejecutaron explícitamente. La captura del libro del usuario no proporciona su PDF original; los casos de transición usan un fixture reproducible. La espera de 200 ms es acotada y no garantiza ocultar la carga de cualquier página compleja ni bajo presión extrema de memoria.

Continúan pendientes impresión, selección entre páginas, editor completo de anotaciones, formularios, firmas, integración del sistema y el resto del inventario 1.0. Los cambios de Marcas mejoran el control existente; no implementan esas funciones pendientes.

## Paquetes verificados

Los ZIP contienen 607 archivos verificados cada uno; DEB y tar, 408 cada uno. Los ejecutables coinciden con los binarios de producción compilados. Los manifiestos registran los hashes individuales.

| Paquete | Bytes | SHA-256 |
| --- | ---: | --- |
| `crabpdf-0.3.5-linux-x64.tar.gz` | 40599201 | `e9386ccc0a1d9c33560518eb451a665128a7b017599ce0b9cbff132e597ed995` |
| `crabpdf-0.3.5-ubuntu24.04-amd64.deb` | 30610284 | `52b0d113f6354eb46f3cbd74fc0b5781e8bfab5acc33987439e1f587e04e747e` |
| `crabpdf-0.3.5-win-x64.zip` | 43570904 | `555e6851f85c560e657cc0c8f29480d9a75945a126fdbca7fb6b092aa4e991d8` |
| `crabpdf-0.3.5-win-x86.zip` | 43220685 | `fb3bb5e697176389020020db6edc28f72825a49b03811b0f6419867189f12ffa` |
