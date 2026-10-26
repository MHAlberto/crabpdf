# Correcciones de marcado, ventanas y desplazamiento

Fecha: 26 de octubre de 2026. Base: desarrollo 0.3.6, después de A03a. Aplicación: [`ca0c08e`](https://github.com/MHAlberto/crabpdf/commit/ca0c08e77ddd9b84f4f279abca42a165dc20a0a6); recorridos reproducibles: [`96e6ed0`](https://github.com/MHAlberto/crabpdf/commit/96e6ed0848171108b046ed11ab4793a74d84afcc). Este último sólo aísla la entrada simulada y no cambia la aplicación.
Este trabajo atiende la regresión visual y la respuesta durante la lectura antes de continuar A03b.

## Marcado sin vaciar la página

La UI eliminaba todas las texturas del documento al recibir `Changed` y volvía a eliminar las de la página con `Marked`. El siguiente fotograma sólo podía dibujar el fondo blanco mientras PDFium generaba la nueva revisión.

Ahora mantiene las instantáneas en la misma caché limitada por bytes. Se solicita una vista previa de la revisión actual con prioridad sobre los mosaicos de mayor resolución. Durante la espera, sólo se dibujan imágenes del mismo documento, página, rotación y modo de color. La revisión más reciente tiene prioridad sobre la antigüedad de uso. Un mosaico anterior no puede cubrir una vista previa más nueva; sus dimensiones, zoom, densidad y región deben coincidir exactamente.

La instantánea sólo sirve para presentación. No satisface `has_current_render`, no modifica la revisión del motor ni puede exportarse como contenido nuevo. El porcentaje de zoom se calcula con la geometría conocida de la página, sin depender de una textura terminada: evita el salto transitorio a «100 %» y conserva la base correcta de Ctrl+rueda durante el marcado. El guardado de marcas, recuperación, sincronización entre ventanas y deshacer siguen a cargo del motor y del almacén existente.

## Arrastrar a otra ventana

Soltar una pestaña fuera de la barra abre el libro en otro proceso de CrabPDF. La ventana original conserva su pestaña y no agrega otra. El menú de contexto también ofrece «Duplicar en otra ventana».

Se transfieren página, zoom, disposición, lectura continua, rotación, colores, manga y ancla normalizada. La nueva ventana usa el mismo perfil y directorio de datos. La transferencia utiliza argumentos estructurados de `Command`, sin construir una línea de shell ni escribir las preferencias como canal de transporte. Los archivos con espacios, Unicode o guion inicial se conservan correctamente.

La CLI acepta `--colors` y `--position` para reconstruir la vista. Se validan esquema y coordenadas; una posición que no pertenece al documento se descarta al conocer el número de páginas.

## Flechas y selección

El desplazamiento vertical por flechas avanza en cada fotograma mientras la tecla está pulsada. La repetición del sistema operativo no determina su cadencia. Una pulsación breve tiene un destino de 40 puntos; mantener la tecla continúa a 650 puntos/s con un seguimiento suave de 35 ms. Soltarla termina el movimiento pendiente; rueda, interacción con la página, campos de texto, diálogos, pérdida de foco o un cambio explícito de vista lo interrumpen. En página única se conserva la navegación al llegar al borde.

Los comandos verticales de pantalla completa/media pantalla y J/K utilizan el mismo seguimiento. Se mantiene «Reducir movimiento» y se limita el intervalo integrado a 50 ms para que reanudar una ventana oculta no produzca un salto grande.

La selección calcula una sola distancia por carácter y termina al encontrar texto bajo el cursor. Un arrastre inmóvil evita repetir la búsqueda. La geometría seleccionada se calcula cuando cambia el rango y une cajas adyacentes de una línea; páginas, columnas y líneas distintas siguen separadas.

## Evidencia

La [evidencia](interaction-fixes-validation.json) registra 96 pruebas únicas aprobadas: 43 unitarias y 53 de integración, incluidas las pruebas nativas normalmente ignoradas. También aprobaron formato, Clippy con `--all-targets -D warnings` y la compilación optimizada.

Los recorridos produjeron 237 capturas finales: 123 durante la interacción, 26 para cada tipo de barra de ventana —incluida una ventana hija real por recorrido—, 16 de formatos/marcas y 46 de zoom/navegación. Las seis muestras Win32 de pantalla completa cubren el monitor con todo el cliente. Se revisaron las imágenes del subrayado y la ventana separada. Los recorridos de barras, formatos y refinamiento usan dev; la aplicación principal y el recorrido de interacción usan release con ThinLTO. Las 46 escenas de refinamiento incluyen Ctrl+rueda, escalas 100/125/150/200 %, foco del buscador, zoom, pantalla completa, presentación y navegación horizontal.

Observaciones del recorrido optimizado local:

| Operación | Resultado observado |
| --- | --- |
| Seleccionar una palabra | 9.47 ms |
| Primer render actual tras resaltar | 37.72 ms |
| Primer render actual tras subrayar / deshacer | 20.17 / 23.42 ms |
| Flecha abajo mantenida, sin repetición OS | 917 llamadas de UI, 407.42 puntos; intervalo máximo 2.15 ms |
| Flecha arriba mantenida, sin repetición OS | 1 014 llamadas de UI, 407.29 puntos; intervalo máximo 1.51 ms |
| Coste mediano de UI durante abajo / arriba | 0.149 / 0.132 ms |
| Hit test anterior / nuevo, medianas de ocho rondas | 32.82 / 8.18 µs por consulta |

La región de texto conserva al menos 2 688 muestras de contraste durante el marcado, frente a 2 986 en la referencia. La prueba detecta vaciado de la página en esa región; no sustituye una evaluación visual de todo documento o fotograma físico. Las latencias de marcado/selección son observaciones individuales con coste de captura, no percentiles globales.

Los primeros intentos del instrumento se conservan: un umbral RGB confundía el tinte azul con texto ausente, las teclas sintéticas necesitaban solicitar su primer repaint y la entrada dependía del foco real del escritorio. Se aisló RawInput para conservar las respuestas de captura e inyectar sólo el foco/teclado/puntero del escenario. `native-controlled` aporta los resultados finales. Se comprueba renderizado nativo con entrada simulada; las llamadas de UI no miden la presentación física del compositor o monitor. Los logs, capturas y reportes quedan en `artifacts/interaction-fixes`.

La [CI de la aplicación](https://github.com/MHAlberto/crabpdf/actions/runs/40348264963) aprobó Windows x64, Ubuntu y macOS Intel/ARM. Windows x86 aprobó Rust/PDFium y falló por un timeout descargando una herramienta externa. Se solicitó el reintento de ese job; el build x86 de esta revisión queda pendiente de su resultado. El JSON registra el estado observado y la URL para consultar su avance. Los cambios posteriores de los recorridos sólo aíslan RawInput y se verificaron localmente con formato, Clippy para todos los targets y los cinco recorridos nativos.

Se preparó un paquete portátil Windows x64 en `artifacts/interaction-fixes/packages/crabpdf-0.3.6-win-x64.zip`. Incluye PDFium, herramientas de formatos, avisos y licencias; se verificaron sus 607 archivos del manifiesto y el ejecutable optimizado. Es una compilación local de desarrollo y no reemplaza una publicación pública con la misma versión.

Se comprueban los contratos de instantáneas, movimiento sin repetición, equivalencia del hit test y transferencia real a otra ventana. El recorrido de interacción examina capturas durante resaltar, subrayar y deshacer; también registra desplazamiento y coste de UI durante flechas mantenidas.

El microbenchmark compara el algoritmo anterior y el nuevo sobre 6 000 caracteres con consultas idénticas y orden alternado. Sólo mide esa operación sintética. No acredita por sí solo la velocidad global del lector, documentos escaneados, conversores, compositor o hardware diferente.

El ejecutable Windows x64 ocupa 11 432 448 bytes frente a 11 412 480 en A03a (+19 968, aproximadamente 0.2 %). No se agregan dependencias ni se amplían los presupuestos de caché. Esto no es una medición del pico de RAM o de CPU en reposo.

## Pendientes de rendimiento

La lectura mantiene un solo propietario de PDFium. Una operación nativa larga todavía puede retrasar el render siguiente; las instantáneas evitan vaciar lo ya visible durante el marcado, pero no reducen la duración de esa operación. A03b debe conservar prioridades/cancelación y caracterizar equidad antes de modificar el planificador.

También quedan por medir el coste del layout en documentos extensos, páginas con texto especialmente denso, latencias de conversión y el comportamiento gráfico nativo en Linux/macOS. Las comprobaciones de CI de esas plataformas y las pruebas visuales locales tienen alcances diferentes.
