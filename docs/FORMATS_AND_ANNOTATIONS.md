# CrabPDF 0.3.3 — formatos y marcas de texto

Esta entrega añade lectura de los formatos básicos de SumatraPDF y anotaciones de texto. Sigue siendo una versión en desarrollo hacia la 1.0. La referencia de formatos es la [documentación de SumatraPDF](https://www.sumatrapdfreader.org/docs/Supported-document-formats); la lista actual de upstream también incluye formatos nuevos y servicios opcionales que todavía no se declaran implementados aquí.

## Formatos disponibles

| Familia | Extensiones |
| --- | --- |
| PDF | `.pdf`, `.ai` con contenido PDF |
| EPUB sin DRM | `.epub` |
| Kindle/Mobipocket/PalmDOC sin DRM | `.mobi`, `.azw`, `.azw3`, `.azw4`, `.prc`, `.pdb` |
| FictionBook | `.fb2`, `.fb2z`, `.fbz`, `.zfb2`, `.fb2.zip` |
| XPS y variantes | `.xps`, `.oxps`, `.xod`, `.dwfx` |
| DjVu | `.djvu`, `.djv` |
| Ayuda HTML compilada | `.chm` |
| Cómics y archivos de imágenes | `.cbz`, `.cbr`, `.cb7`, `.cbt`, `.zip`, `.rar`, `.7z`, `.tar`, `.ora` |
| Imágenes | `.png`, `.jpg`, `.jpeg`, `.jfif`, `.gif`, `.tif`, `.tiff`, `.bmp`, `.dib`, `.ico`, `.tga`, `.webp`, `.jp2`, `.j2k`, `.jpx`, `.jpf`, `.jpm`, `.j2c`, `.svg` |
| Texto y Markdown | `.txt`, `.log`, `.nfo`, `.md`, `.markdown` |
| HTML local | `.html`, `.htm`, `.xhtml` |

El diálogo muestra estos formatos y permite seleccionar otros archivos para recibir un diagnóstico explícito. Arrastrar archivos y abrirlos desde la línea de comandos utiliza el mismo despacho. Los cómics se ordenan por números: página 2 precede a página 10. TIFF admite varias páginas. OpenRaster usa su imagen compuesta.

La aplicación conserva el documento original. PDF se abre directamente en PDFium. La apertura explícita de otros formatos y la preparación de portadas sin caché ocurren en el hilo de importación; el worker PDF recibe el documento preparado para render por tiles, precarga, miniaturas y texto. Falta verificar la prioridad con un decodificador lento bajo carga. EPUB/FB2/Kindle conservan el texto seleccionable; se recuperan título e índice cuando el documento los proporciona. DjVu conserva su capa OCR existente, que se incorpora por página al solicitar texto; esto no realiza OCR nuevo.

En Windows, los decodificadores acompañan al ejecutable en `format-tools`: MuPDF 1.28.5, libmobi 0.12, DjVuLibre 3.5.29 y 7-Zip 26.04. MuPDF y libmobi se compilan como herramientas x86 con CRT estático. En Linux también se empaquetan MuPDF 1.28.5 y libmobi 0.12, compilados desde los mismos archivos fuente; DjVuLibre y 7-Zip se instalan desde la distribución. Los procesos de conversión terminan al preparar el documento; leer PDF no inicia estos procesos. No se añaden un navegador ni un servicio en la nube.

## Seleccionar, resaltar y subrayar

1. Selecciona una palabra con doble clic, o arrastra para seleccionar una frase de una página.
2. Abre **Marcas** en la barra, **Texto** en el menú o el menú contextual sobre la página.
3. Elige **Resaltar selección** o **Subrayar selección**. Amarillo es el color inicial; también hay verde, azul, rosa, naranja, rojo y selector de color personalizado.
4. **Deshacer última marca** elimina la última marca local de ese documento. Las pestañas que abren el mismo documento se actualizan.
5. **Guardar una copia** exporta un PDF que contiene las anotaciones reales y que otros lectores PDF pueden mostrar.

| Acción | Atajo |
| --- | --- |
| Resaltar selección | Ctrl/⌘ + Mayús + L |
| Subrayar selección | Ctrl/⌘ + Mayús + U |
| Deshacer última marca | Ctrl/⌘ + Z |
| Guardar copia PDF | Ctrl/⌘ + S |
| Copiar selección | Ctrl/⌘ + C |

Las marcas se conservan automáticamente en el almacén local y reaparecen al abrir el mismo archivo en CrabPDF. No se sobrescribe el original. Para compartirlas, exporta la copia PDF. Las restricciones de selección/anotación del PDF se respetan. Un archivo sin texto seleccionable requiere una capa de texto u OCR antes de marcar palabras.

Antes de exportar, CrabPDF refresca las marcas guardadas por otra instancia bajo el bloqueo del almacén. Cada cambio de marcas incrementa una revisión de contenido: se invalidan los rasters del motor y se ignoran respuestas anteriores. La caché visual conserva temporalmente la imagen compatible ya mostrada hasta recibir su reemplazo, dentro del mismo presupuesto, para evitar un fotograma blanco al marcar. Esas instantáneas no cuentan como renders actuales ni alteran lo exportado. [Corrección y evidencia](https://github.com/MHAlberto/crabpdf/blob/main/docs/INTERACTION_FIXES.md). Las pruebas nativas cubren dos motores abiertos a la vez y un refresco de varias páginas. La sincronización entre procesos, los conflictos y los fallos de escritura todavía requieren más pruebas; R01/R02 siguen abiertos. Véase [el avance](IMPLEMENTATION_PROGRESS.md).

El color elegido se guarda con las preferencias. La asociación de marcas incluye ruta, tamaño y fecha de modificación: un archivo cambiado externamente no recibe las marcas de una versión anterior. Una marca local dañada produce un aviso y no impide leer el documento; su archivo se conserva.

Esa asociación no es una huella criptográfica del contenido: cambiar un PDF sin alterar sus metadatos puede pasar inadvertido. El esquema 2 guarda además la firma del PDF preparado; si un libro viejo no tiene firma o una conversión cambió, conserva sus marcas sin aplicarlas y muestra un aviso. Mover el archivo o migrar esas marcas a una ubicación equivalente aún requiere anclas de texto/contexto; R07 sigue abierto.

## Caché, límites y plataformas

Los PDFs internos y sus metadatos se guardan bajo `documents/`, junto al directorio de preferencias; las marcas, bajo `annotations/`. El directorio compatible de Windows sigue siendo `%APPDATA%\rust-reader`. `CRABPDF_DATA_DIR` permite elegir otra ubicación. Los archivos temporales se crean en un directorio propio y se eliminan al terminar.

La importación admite hasta 512 MiB por archivo/trabajo, 128 MiB por recurso de archivo comprimido, 10 000 recursos y 3 minutos por proceso de decodificación. La caché de documentos tiene un presupuesto de 512 MiB; las marcas se conservan aparte. Las rutas de archivos comprimidos se validan y sus recursos se extraen individualmente, sin ejecutar su contenido. Cerrar una pestaña o terminar el worker cancela la importación y los procesos auxiliares en ejecución, con espera y limpieza.

Son límites de controles actuales, no una cuota de RAM. La importación cancelable tiene un plazo total de tres minutos. La espera del bloqueo de cada entrada de caché se comprueba cada 25 ms, respeta cierre/cancelación y acaba con error tras 15 segundos si otra importación no libera la entrada. Ciertas copias de archivo aún no se interrumpen a mitad de la operación. Los registros de listado de archivos y metadatos generados por los decodificadores se limitan antes de cargarse en memoria. La caché v4 publica manifiestos al final y verifica tamaños/CRC antes de reutilizar. Si PDFium rechaza el PDF interno durante una apertura, se reconstruye en el hilo de importación y se reintenta una sola vez; las portadas y algunos conflictos entre procesos todavía no se recuperan de la misma forma. `--config` y `CRABPDF_DATA_DIR` determinan el perfil y sus datos derivados; para pruebas usa siempre un perfil aislado.

Los paquetes Windows x64 y x86 incluyen todos los decodificadores. El paquete x64 ejecuta los ayudantes x86 mediante WOW64. Se comprobaron en Windows 10 x64; ejecutar x86 mediante WOW64 no sustituye una prueba en un Windows de 32 bits. Windows 11 y macOS todavía necesitan validación propia. En Ubuntu, `mutool` y `mobitool` proceden de `format-tools/`; `ddjvu`, `djvutoxml` y `7z` proceden de APT. El tar Linux requiere instalar estos últimos por separado. Las demás distribuciones necesitan comprobación propia.

## Límites de paridad

Los libros usan paginación fija de lectura, no reflujo al cambiar el ancho de la ventana. La conversión no conserva todos los enlaces/acciones, formularios ni particularidades de CHM; el índice recuperado permite navegar capítulos. GIF se presenta como imagen estática e ICO como una imagen decodificada. No se declaran AVIF, HEIC, JPEG XL, JPEG XR, TCR, PostScript, EPS ni DVI. No se elimina DRM. La edición de anotaciones existentes de terceros, selección entre páginas, borrado individual mediante ratón y escritura de anotaciones dentro del EPUB/FB2 original siguen pendientes. Estos límites impiden declarar paridad completa con SumatraPDF.

## Verificación y reproducción

- `scripts/Get-FormatTools.ps1` descarga entradas fijadas por SHA-256 y compila las herramientas necesarias; no instala sus visores ni 7-Zip en el sistema.
- `scripts/New-FormatSamples.ps1` genera EPUB, FB2, XPS, cómics y documentos originales de prueba; se utilizan también ejemplos distribuidos por libmobi, DjVuLibre y 7-Zip, con su procedencia conservada.
- `tests/formats_and_marks.rs` abre formatos reales, comprueba texto Unicode y rasters, orden de cómics, índice de EPUB, TIFF multipágina, MOBI Huffman/KF8 frente a DRM, persistencia/colores/exportación/deshacer en páginas recortadas y giradas, copias abiertas simultáneamente y recuperación ante marcas locales dañadas. `tests/annotation_sync.rs` comprueba la exportación tras una marca guardada por un segundo proceso. `tests/import_recovery.rs` confirma que un caché con CRC válido pero rechazado por PDFium se reconstruye y el original no cambia.
- El resaltado debe cubrir al menos un 20 % de la región de texto; el subrayado debe abarcar más del 60 % de su eje de lectura. Esto detecta una marca reducida a una raya lateral por un orden incorrecto de esquinas.
- `examples/formats_smoke.rs` comprueba doble clic real mediante eventos de ratón, atajos de marcas, deshacer, reapertura y seis documentos de distintas familias en una ventana nativa; guarda 16 capturas por arquitectura. Un libro con DRM muestra el rechazo y no presenta un diálogo de contraseña PDF.
- `scripts/Test-Local.ps1` integra esas pruebas con los recorridos anteriores. `scripts/Verify-Portable.ps1` comprueba contenido, hashes y ejecutable de los paquetes.

Las licencias, hashes, fuentes y opciones de compilación de los ayudantes se conservan junto a los paquetes. La licencia MIT de la aplicación no reemplaza las licencias de estos componentes.
