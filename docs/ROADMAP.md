# Meta del producto: v1 y v2

Alcance actualizado por el usuario el **13 de octubre de 2026**.

La versión pública actual es **0.4.1**, de desarrollo. El [plan ejecutable](V1_PLAN.md), el [backlog R00–R30](V1_BACKLOG.csv) y la [validación](VALIDATION.md) distinguen lo realizado de lo pendiente. Los informes anteriores están en el [historial](history/README.md).

## Versión 1: paridad completa con SumatraPDF

La v1 termina cuando CrabPDF reproduce todas las funciones y los comportamientos visibles de la revisión de SumatraPDF elegida como referencia. Incluye los detalles pequeños: menús contextuales, atajos, gestos, estados habilitados/deshabilitados, foco, tooltips, diálogos, navegación, persistencia y recuperación de errores. La apariencia conserva la identidad propia y el diseño discreto solicitado; la comparación de comportamiento se hace control por control.

La aplicación y su lógica se desarrollan en Rust. Se estudian las responsabilidades y estrategias de SumatraPDF para adaptarlas a nuestra arquitectura. El planteamiento original autoriza bibliotecas existentes y una implementación independiente, sin traducir todo el C/C++. La implementación actual usa PDFium y herramientas nativas para otros formatos. Esto no constituye un motor íntegramente Rust, y reescribirlo no es un requisito previo acordado para V1.

La v0.3 en desarrollo añade controles de consistencia, perfiles e importación a la base 0.2 de formatos y marcas. La equivalencia parcial de abrir, avanzar, retroceder, zoom o tema no equivale a cerrar la paridad de esas funciones.

## Referencia reproducible

- Repositorio: [sumatrapdfreader/sumatrapdf](https://github.com/sumatrapdfreader/sumatrapdf).
- Revisión fijada: [`3316ad49c5c24f6f78a3849a28eea04517baa833`](https://github.com/sumatrapdfreader/sumatrapdf/tree/3316ad49c5c24f6f78a3849a28eea04517baa833).
- Fecha del commit: 13 de octubre de 2026, 14:05:07 UTC.
- Objetivo: el repositorio en esa revisión, incluidas sus funciones posteriores a la versión estable 3.6.1. Nuevos cambios de upstream se incorporarán por actualización explícita del inventario.

La captura visual del usuario muestra 3.5.2, pero la referencia funcional del proyecto está fijada al commit indicado. No se debe mezclar una prueba de una versión estable distinta con funciones que sólo existen en esa revisión de desarrollo.

El repositorio actual también contiene [`src/ng`](https://github.com/sumatrapdfreader/sumatrapdf/blob/3316ad49c5c24f6f78a3849a28eea04517baa833/src/ng/README.md), que declara targets macOS, Linux y WebAssembly y adaptaciones de atajos. Es una referencia para equivalentes por plataforma. Aquí no se han compilado ni probado esos targets de SumatraPDF.

## Inventario de paridad

La extracción inicial está en [parity/commands.csv](parity/commands.csv), con procedencia en [parity/reference.json](parity/reference.json). Contiene **332 entradas con identificador numérico explícito** de `src/shared/Commands.h`. Incluye identificadores internos, un marcador duplicado y comandos de diagnóstico; **no son 332 funciones distintas ni un inventario completo de la interfaz**. La extracción tampoco cubre todos los alias simbólicos o argumentos.

Tras reconciliar nueve entradas con el código 0.2, hay **83 parciales, 244 pendientes y 5 referencias internas**. Las parciales enlazan implementación y pruebas cuando existen; las comprobaciones manuales aún ausentes se indican. Ninguna se declara en paridad completa. `referencia_interna` identifica entradas cuyo uso real hay que clasificar, incluidos marcadores y diagnósticos; no representa una exclusión automática de funciones visibles.

Cada función deberá tener estos datos antes de considerarse terminada:

1. Fuente y revisión de referencia, con los controles y comandos involucrados.
2. Secuencia de acciones, precondiciones y resultado esperado, incluidos errores y cancelación.
3. Comportamientos pequeños: foco, habilitación, tooltip, gesto, atajo, persistencia y selección.
4. Implementación Rust y servicios nativos utilizados.
5. Prueba y evidencia por plataforma; separación entre compilación y ejecución funcional.
6. Estado: pendiente, parcial, implementada sin validar o paridad verificada.

Los casos que requieran un equivalente específico del sistema permanecen abiertos hasta comprobar ese equivalente. No se marcan completos por tener un botón o un diálogo vacío.

## Estado de implementación hacia v1

| Grupo | Base actual | Trabajo pendiente para v1 |
| --- | --- | --- |
| Lectura | Una página, enfrentadas, libro, continuo, manga, saltos, historial en memoria y punto de lectura persistente | Historial completo persistente, coherencia entre ventanas y pruebas exhaustivas. |
| Zoom y geometría | 8–6400 %, personalizado, página/ancho/alto, rotación, pan y tiles | Presets fraccionarios, contenido/visible/selección, cursor, gestos y recorte de lectura. |
| Paneles | Miniaturas reales, índice, favoritos y búsqueda | Jerarquía/collapse del índice, edición y orden de favoritos, paneles adicionales. |
| Texto y enlaces | Selección de una página, copiar, buscar, resultados visibles y enlaces | Selección entre páginas/por teclado, copiar imagen/enlace, gestos completos y búsqueda avanzada. |
| Archivos y ventanas | Pestañas, duplicar/cerrar/reabrir, ventanas, recientes, sesión, recarga y drag/drop | Grupos, renombrar/eliminar, sesiones entre ventanas y pruebas de servicios nativos. |
| Menús y comandos | Acciones compartidas, menús/contexto, paleta y atajos | Comandos restantes, argumentos/prefijos de paleta, personalización y foco exhaustivo. |
| PDF protegido | Contraseña, reintentar/cancelar; permisos de copia del SDK | AES moderno/corpus adicional y permisos de otras operaciones. |
| Impresión | Sin implementar | Diálogo nativo, rangos, selección, escala y opciones de referencia; impresión desde CLI. |
| Modificar PDF | Resaltar/subrayar texto con colores, persistir, deshacer y exportar una copia PDF | Editar anotaciones existentes, otras anotaciones y herramientas de edición, rehacer, guardar original, formularios, firmas y herramientas PDF de referencia. |
| Colores y preferencias | Tema claro/oscuro, inversión/grises, TOML y preferencias de lectura | Colores personalizados, idiomas y preferencias avanzadas. |
| Formatos | PDF directo y formatos básicos mediante preparación local; libros, cómics, XPS, DjVu, CHM e imágenes | Reflujo, detalles de enlaces/CHM, formatos avanzados y comprobación por plataforma. Véase [alcance de 0.2](FORMATS_AND_ANNOTATIONS.md). |
| Integraciones | CLI con múltiples rutas, página, zoom, modos, búsqueda y presentación | Parámetros restantes, SyncTeX, asociaciones e integración de vista previa/búsqueda. |
| Funciones recientes | Sin implementar | Lectura en voz alta, scripts PDF, acciones y paneles opcionales de herramientas externas presentes en la revisión. |
| Rendimiento | Caché LRU por bytes, tiles visibles, overview, precarga de vecinos, búsqueda incremental y preparación de portadas fuera del worker PDF | Prueba de latencia con importador lento, cancelación de todas las fases, corpus grande, mediciones comparativas y memoria extrema. |
| Distribución | Builds Windows x64/x86 y Linux x64 con ZIP, tar y DEB con avisos | Instaladores Windows, otras distros/macOS, firmas, auditoría de distribución, actualización e instalación/desinstalación. |

La tabla organiza el trabajo y no sustituye el inventario exhaustivo. Se deben expandir especialmente submenús, preferencias, comandos parametrizados y comportamientos sin un comando propio.

Las exclusiones originales de anotaciones, EPUB, edición e IA correspondían a la primera v0.1. El objetivo posterior de paridad completa amplía ese alcance. Las funciones que usen herramientas o servicios externos deben funcionar como opciones explícitas y conservar la lectura local sin cuentas ni Internet obligatorio; no se habilitan ni ejecutan automáticamente. Su presencia en el inventario no significa que ya estén implementadas.

## Plataformas de entrega

| Plataforma | Objetivo de v1 | Estado actual |
| --- | --- | --- |
| Windows 10 x64 | Ejecutable, portable e instalador; pruebas funcionales | Compilación y recorrido de lectura probados. |
| Windows 10 x86 | Build `i686-pc-windows-msvc`, PDFium x86 y pruebas en SO de 32 bits | Build y recorrido x86 bajo WOW64 en Windows 10 x64; falta SO x86. |
| Windows 11 x64 | Ejecutable y pruebas funcionales propias | Sin prueba funcional. |
| Aplicación x86 en Windows 11 x64 | Compatibilidad bajo WOW64 y bibliotecas x86 | Probada sólo bajo Windows 10 x64; falta Windows 11. Windows 11 no tiene una edición del sistema de 32 bits. |
| macOS Intel y Apple Silicon | Builds x86_64/arm64, `.app`, paquete y pruebas | CI de 0.4.1 (`6eb980a`): build y ocho pruebas nativas PDF por arquitectura; sin UI, otros formatos ni paquete `.app`. |
| Linux x64 | Ubuntu, Debian, Linux Mint, Fedora, Arch y openSUSE; versiones concretas en la matriz de pruebas | Desde 0.3.3: Ubuntu 24.04 en Docker con Xvfb; tar glibc 2.36 y DEB. Sin escritorio físico ni otras distros. |

En Linux hay que probar X11 y Wayland, GNOME y KDE, portales, fuentes, impresión y compatibilidad de las bibliotecas empaquetadas. Las versiones mínimas de distro y macOS se fijarán al obtener evidencia de compilación y ejecución. Los paquetes pueden usar AppImage/Flatpak y, según la entrega, DEB/RPM; no se declara compatibilidad universal con Linux por producir un binario.

Microsoft documenta [requisitos de Windows 11](https://www.microsoft.com/en-us/windows/windows-11-specifications) y [ejecución de aplicaciones de 32 bits bajo WOW64](https://learn.microsoft.com/en-us/windows/win32/winprog64/running-32-bit-applications). Un proceso x86 necesita DLL x86; no puede cargar la DLL x64 actual.

## Arquitectura que se debe ampliar

Los primeros cuatro componentes ya tienen implementación inicial. La paridad avanzada y los servicios faltantes continúan abiertos:

- **Registro de comandos:** una misma acción alimenta menús, barra, atajos y paleta; disponibilidad según documento, selección y plataforma. Evitar handlers duplicados por control.
- **Controlador por documento:** navegación, historial, búsqueda, selección y estado de cada pestaña. Referencias: `DocController`, `DisplayModel` y `WindowTab` de SumatraPDF.
- **Modelo de geometría:** espacios de coordenadas de documento/página/pantalla, transformaciones de zoom/rotación y páginas visibles. Es necesario para selección, enlaces, anotaciones y desplazamiento continuo.
- **Renderizador y caché:** tiles para zoom alto, presupuesto en bytes y prioridad de viewport implementados; preparación de páginas próximas pendiente. `RenderCache` es referencia de comportamiento; sus constantes y estructuras no se copian automáticamente.
- **Motores por capacidades/formato:** el adaptador PDFium ya ofrece texto, búsqueda, enlaces y metadatos. Debe ampliarse para cambios, firmas, reflujo y otros formatos. PDFium no proporciona automáticamente todos los motores y servicios que usa SumatraPDF.
- **Servicios de plataforma:** impresión, portapapeles, apertura de enlaces, asociaciones, ventanas, instalación y accesibilidad. Mantener el modelo principal en Rust independiente del SO.

Se mantiene egui/eframe mientras cubra el comportamiento requerido. Las limitaciones de accesibilidad, foco o controles nativos se comprobarán con pruebas concretas antes de dar esas partes por equivalentes.

## Rapidez y tamaño

Usar Rust no demuestra rendimiento equivalente. Hay que medir ambas aplicaciones con los mismos documentos y equipo: arranque en frío/caliente, apertura, latencia de navegación/zoom/búsqueda, CPU en reposo, memoria máxima y tamaño instalado completo. Los presupuestos cuantitativos se fijan a partir de esa comparación, con resultados publicados en el proyecto.

El corpus debe incluir documentos largos, escaneados, con fuentes complejas, tamaños mixtos, rotaciones, protección y errores. Hay que comprobar equipos modestos, drivers gráficos y el menor espacio de direcciones de x86. Tiles y caché deben respetar un presupuesto de memoria por plataforma.

Las funciones de paridad requeridas para v1 siguen formando parte del producto aunque su implementación se cargue bajo demanda. No se posponen a plugins de v2 para declarar prematuramente terminada la v1.

## Orden de desarrollo

1. **Referencia y consistencia:** expandir subcasos y corregir sincronización de marcas/exportación, perfiles, caché e importaciones (R00–R07).
2. **Lectura y aplicación:** contratos tipados, selección, búsqueda, zoom, paneles, sesiones, ventanas y accesibilidad (R08–R14/R30).
3. **Paridad avanzada:** formatos fieles, impresión, edición/anotaciones/formularios/firmas e integraciones de la revisión (R15–R23).
4. **Trabajo transversal desde el inicio:** corpus, sistemas reales, empaquetado, robustez y rendimiento (R24–R27). Builds/DLL x86 ya existen; faltan otros entornos de ejecución.
5. **Cierre de v1:** verificar cada comportamiento y completar la matriz y entrega (R28–R29).

La validación local se automatiza con `scripts/Test-Local.ps1` y entornos Linux separados. El usuario autorizó publicar avances en https://github.com/MHAlberto/crabpdf. La CI preparada no constituye evidencia de ejecución.

## Criterio de cierre de v1

V1 requiere inventario exhaustivo, paridad funcional y visual verificada, diferencias de plataforma resueltas, pruebas de todos los formatos, evidencia de rendimiento y entrega instalable/portable validada. No basta que compile la UI. La revisión de dependencias y avisos debe corresponder a los paquetes concretos entregados.

La entrega pública es **0.4.1, de desarrollo**. R09 se completó como extracción de arquitectura; la paridad funcional y la aceptación cuantitativa A00/A07 siguen abiertas. [Resultados y límites actuales](VALIDATION.md).

## Versión 2: extensiones opcionales en Rust y WebAssembly

La v2 añade capacidad de ampliar o modificar el comportamiento de la v1 conservando su instalación y lectura básica ligeras. La ruta preferida es escribir plugins en Rust y compilarlos a WebAssembly.

Diseño previsto:

- API de host versionada para comandos, eventos, operaciones de documento y puntos de extensión de UI.
- Instalar, habilitar, deshabilitar y cargar una extensión sólo cuando se utiliza; límites de CPU/memoria y liberación al desactivarla.
- Runtime/host de WebAssembly opcional, cargado bajo demanda o distribuido en un componente aparte. Medir su coste; no prometer que sólo instalar plugins mantiene idéntico tamaño o uso de memoria.
- Acceso explícito a archivos, red y modificaciones del documento; un fallo de la extensión no debe bloquear la lectura.
- Compatibilidad de plugins entre Windows, Linux y macOS y pruebas de cada punto de extensión.

Se pueden preparar límites internos claros durante v1, pero no desarrollar todavía el runtime, una tienda o un sistema de plugins. La API pública se decide cuando existan casos reales de extensión y el núcleo haya alcanzado la paridad requerida.

## Licencias y fuentes

La lectura del código fuente se utiliza para identificar responsabilidades y comportamiento. No se incorporó código C/C++ de SumatraPDF a la aplicación Rust. El proyecto contiene archivos de referencia GPLv3 y otros BSD: conservar su procedencia y revisar la licencia de cada archivo si posteriormente se traduce o reutiliza código. Traducirlo a Rust no elimina las condiciones de la licencia original. La licencia MIT actual del código propio no se extiende a código ajeno.

Las fuentes consultadas y la revisión están en `parity/reference.json`; el inventario deriva de nombres/IDs de comandos, no de una copia de sus implementaciones. Los avisos de los datos de referencia se conservan en `parity/NOTICE.md` y `parity/COPYING.BSD`.
