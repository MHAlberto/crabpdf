# Validación local

Fecha: **13 de octubre de 2026**. Host: Windows 10 x64, build **19045**. Rust **1.96.0**, PDFium **7881**, backend gráfico eframe/Glow.

Este informe conserva la evidencia histórica de **0.2.0**. Las pruebas y límites posteriores se registran en [0.3.0](IMPLEMENTATION_PROGRESS.md), [0.3.1](IMPLEMENTATION_PROGRESS_0_3_1.md) y [0.3.2](IMPLEMENTATION_PROGRESS_0_3_2.md), con informes locales separados; no se atribuyen los paquetes ni las capturas siguientes a esas versiones.

La revisión [0.3.3](IMPLEMENTATION_PROGRESS_0_3_3.md) amplía `refinement_smoke` a 46 escenas para pantalla completa y foco/navegación por teclado. Su informe separado identifica las pruebas y los paquetes de esta revisión; las 21 escenas descritas más abajo corresponden al recorrido anterior.

La revisión [0.3.4](IMPLEMENTATION_PROGRESS_0_3_4.md) añade cuatro pruebas de posición de lectura y un recorrido de tres procesos nativos con tamaño/escala diferentes y prioridad CLI. Su evidencia queda separada de la matriz histórica siguiente.

## Resultados comprobados

| Comprobación | x64 | x86 en WOW64 |
| --- | --- | --- |
| Formato y Clippy con warnings como errores | Pasa | Pasa |
| Pruebas de lógica/modelo/CLI/preferencias/guardado/formatos | 19 pasan | 19 pasan |
| Pruebas con PDFium real | 7 pasan | 7 pasan |
| Formatos y marcas, pruebas de motor con bibliotecas reales | 5 pasan | 5 pasan |
| Build optimizada con runtime estático | Pasa | Pasa |
| Recorrido de interfaz nativa | 25 capturas, termina con código 0 | 25 capturas, termina con código 0 |
| Arranque CLI nativo, dos archivos y opciones de vista | Pasa | Pasa |
| Resultado de búsqueda visible en una página larga inicialmente desconocida | Pasa | Pasa |
| Biblioteca, precarga, ancla de zoom, paleta, rueda, modos y diez pestañas | 21 capturas de refinamiento | 21 capturas de refinamiento |
| Palabras, colores, subrayado, persistencia, formatos, biblioteca y rechazo de DRM | 16 capturas | 16 capturas |

El informe reproducible está en `artifacts/validation/results.json`, con comandos, códigos de salida y logs. Se genera con `scripts/Test-Local.ps1`, sin GitHub. Los tiempos de esos comandos incluyen compilación/enlace; **no son benchmarks de la aplicación**.

Tras corregir el fondo de la barra inferior y los fades del viewport se reconstruyó y repitió el recorrido gráfico. Las capturas de README corresponden a la aplicación actual.

## Qué prueban los casos

Los PDFs originales y redistribuibles se generan con `scripts/New-SamplePdf.ps1`:

- `reading-sample.pdf`: texto/vectores/color, página vertical de 600×800 puntos y horizontal de 800×500.
- `features-sample.pdf`: metadatos, dos bookmarks, un enlace URI y un enlace interno.
- `geometry-sample.pdf`: CropBox con origen no nulo y rotación intrínseca de 90 grados.
- `protected-sample.pdf`: cifrado de prueba revision 2; contraseña de fixture `reader-test`.
- `tall-sample.pdf`: página vertical seguida de una página de 400×3000 puntos, para comprobar el salto a un resultado lejano.

Las siete pruebas nativas verifican render, navegación, zoom/HiDPI, conversión de color, errores, worker real, contraseña, índice, enlaces, extracción/búsqueda y copia PDF. Una prueba reconstruye el raster por regiones en cuatro rotaciones y tres colores y mide deriva media menor que 2/255. Otra compara coordenadas de texto con la tinta renderizada tras crop/rotación. Se añadieron una matriz de resolución a 100/125/150/200 % con densidades 1/1.25/1.5/2 y una prueba de caché persistente de portada, lectura del PNG y recuperación de corrupción.

Las unitarias cubren LRU/presupuesto, layouts, tiles visibles, navegación/invalidación, argumentos inválidos y Unicode, TOML con recientes/sesión/favoritos y escritura que falla sin truncar el archivo anterior.

`visual_smoke` usa preferencias aisladas. Introduce texto y teclado reales en egui, solicita capturas del framebuffer y comprueba estados antes de avanzar. Incluye búsqueda, modos, giro, colores, propiedades, contraseña/desbloqueo, errores, paleta, miniaturas, selección, duplicar, reabrir y cerrar a izquierda. Las imágenes quedaron en `artifacts/validation/x64-ui/` y `x86-ui/`; se inspeccionaron muestras de lectura, búsqueda, selección y protección.

`cli_smoke` usa el parser real y `ReaderApp::with_launch`. Comprueba dos pestañas, página 2, 200 %, enfrentadas y giro de 90 grados. Requiere que la página de destino quede visible y renderizada, incluso con desplazamiento horizontal.

`search_smoke` avanza al último resultado de una página alta desconocida y comprueba que su rectángulo intersecte el viewport real. La búsqueda conserva el destino pendiente hasta recibir la geometría correcta.

[Casos y criterios todavía pendientes](parity/cases.md).

`refinement_smoke` recorre una ventana real con 21 capturas: verifica que la página vecina tenga una vista preparada antes de desplazar, conserva el centro al pasar a 200 %, vuelve de Inicio a un documento con Ctrl+Tab, ejecuta el segundo resultado de la paleta con flecha abajo/Enter, aplica Ctrl+rueda, recorre F11/F5/Escape, comprueba maximizar/restaurar con el estado del sistema, retira historial conservando el PDF y abre diez pestañas. También prueba Ctrl+Mayús+`=` sin ampliar la interfaz. Sus capturas DPI usan `set_zoom_factor` de egui; no se cambió el DPI de Windows.

El icono SVG se comprobó con alpha 0 en el fondo y alpha 255 en el dibujo, y se extrae el icono del ejecutable mediante Windows. Se conservan imágenes y hashes en `artifacts/validation/branding/`. Las operaciones de arrastre de ventana, esquinas, minimizar, el menú Snap de Windows 11 y sistemas no ejecutados requieren comprobación manual adicional.

## Paquetes

`scripts/Pack-Portable.ps1` produce ZIPs de desarrollo x64/x86 en `artifacts/packages/`. Comprueba arquitectura PE, procedencia y SHA-256 de PDFium; incluye ejecutable, DLL, textos/avisos y manifiesto de archivos. Se conserva la opción de licencia correspondiente al conjunto de avisos recopilado; queda la revisión final para distribución estable.

`scripts/Verify-Portable.ps1` lee los ZIPs sin extraerlos y compara longitud/SHA-256 de cada archivo con su manifiesto, además del hash del ejecutable recién compilado.

La CI remota del 15 de octubre detectó fallos de preparación: `windows-latest` usaba Visual Studio 18 y no tenía la ruta CRT v143 esperada, y el runner macOS arm64 agotó el límite de la API GitHub sin autenticación. La matriz Windows se fijó a `windows-2022`, y la consulta de metadatos PDFium acepta `GITHUB_TOKEN` en CI sin enviarlo a la descarga del asset. Estos cambios de infraestructura no sustituyen la validación funcional local ni acreditan por sí solos una nueva ejecución remota aprobada.

`scripts/Build-LinuxFormatTools.sh` compila MuPDF 1.28.5 y libmobi 0.12 para Linux x64 desde archivos fuente con SHA-256 fijado. `scripts/Export-RustLicenses.py` recopila los textos de dependencias Linux. `scripts/Pack-Linux.sh` comprueba arquitectura ELF, procedencia y SHA-256 de PDFium y conversores, e incluye `SHA256SUMS` en un tar portable y un `.deb` de desarrollo para Ubuntu 24.04. El `.deb` declara las bibliotecas gráficas, OpenSSL 3, DjVuLibre y 7-Zip como dependencias APT. El tar requiere instalar las dependencias del sistema descritas en `INSTALL-LINUX.md`. La prueba real en Ubuntu 24.04 dentro de Docker con Xvfb se registra en [el informe Linux 0.3.3](IMPLEMENTATION_PROGRESS_0_3_3_LINUX.md).

Se revisaron imports PE: los ejecutables usan CRT integrado y PDFium sólo necesita sus DLL de sistema. Esto elimina la dependencia observada de VCRUNTIME140.dll; no prueba compatibilidad con todos los controladores gráficos.

Windows no tiene instalador ni firma; Linux tiene un `.deb` sin firma. No hay paquete macOS firmado/notarizado. Las bibliotecas permanecen junto al ejecutable. `target/release/crabpdf.exe` contiene la build Windows x64 actual; el antiguo `target/release/rust-reader.exe` se actualiza como compatibilidad para accesos directos anteriores.

## Límites de esta evidencia

- Windows x86 se ejecutó bajo WOW64 en Windows 10 x64; **no** se probó un SO de 32 bits.
- Windows 11, macOS Intel/Apple Silicon y otras distros Linux no se han ejecutado aquí. Ubuntu 24.04 se ejecutó en Docker con Xvfb; esto no sustituye una prueba en un escritorio físico ni una prueba de Wayland. La CI remota necesita verificación propia.
- El selector nativo, portapapeles, drag/drop del SO, impresión, servicios de carpeta/ventanas y todo el foco/teclado requieren comprobación manual adicional.
- Los fixtures pequeños no cubren documentos escaneados/grandes, todas las fuentes, RTL/CJK, cifrado AES moderno ni PDFs malformados extremos.
- No se han medido arranque, CPU/memoria en reposo, latencia y tamaño instalado contra SumatraPDF con un corpus equivalente.
- Faltan impresión, edición general de anotaciones/formularios/firmas, formatos avanzados, integraciones y muchos comandos/comportamientos del inventario.

La versión sigue siendo **0.2.0 en desarrollo hacia v1**. El registro propio incorpora resaltar, subrayar y deshacer marcas; el inventario de paridad de upstream continúa parcial. Ninguna función se certifica en paridad completa. Estos resultados no cumplen todavía el criterio de cierre de v1 definido en [ROADMAP](ROADMAP.md).

Las cinco pruebas nuevas comprueban apertura/raster/texto de las familias básicas, índice y título EPUB, cómics RAR/ZIP/7z/TAR ordenados por números, TIFF multipágina, ICO/TGA/DIB, Kindle Huffman/KF8 y rechazo de DRM, marcas de cuatro colores en crop/rotación, persistencia/exportación/deshacer, concurrencia entre copias y conservación de un almacén dañado. El control geométrico exige superficie de resaltado y longitud de subrayado, además de cambios de píxeles. [Detalles](FORMATS_AND_ANNOTATIONS.md).

Los ayudantes se verifican como PE x86 en ambos paquetes; `mutool` se recompiló porque la descarga oficial 1.28.5 es x64. La build local usa CRT estático y sus imports sólo incluyen KERNEL32/ADVAPI32. DjVuLibre usa las DLL x86 de CRT incluidas localmente. Los paquetes se acompañan del archivo de fuentes de los decodificadores.

## Revisión 0.3.5

El [informe independiente de transiciones e interfaz](IMPLEMENTATION_PROGRESS_0_3_5.md) registra las ejecuciones del 23 de octubre de 2026 y sus límites. `transition_smoke` añade eventos de teclado, inversión rápida, capturas intermedias, offsets monótonos, movimiento reducido, comprobación del borde inferior y clic real de cierre. Sus resultados están en `artifacts/validation-ui-20261023/`; no actualizan retroactivamente las cifras de este informe histórico.

## Revisión 0.3.6

[Informe independiente](IMPLEMENTATION_PROGRESS_0_3_6.md). `document_chrome_smoke` comprueba dos libros con favoritos, navegación sin cambiar zoom, duplicación de una pestaña inactiva y cancelación de arrastre. `Test-WindowGeometry.ps1` añade lecturas Win32 de cliente, origen y monitor, con pantalla completa desde ventana normal/maximizada y ambas barras de título. Esta prueba cubre un recorte exterior al framebuffer que las capturas anteriores no detectaban.
