# A03c–A07: workspace y cierre de extracción

Fecha: 26 de octubre de 2026. Base inmediata: `c957bba`, posterior a la publicación 0.4.0. No se cambia la versión ni la publicación existente.

## Cambios implementados

- A03c: coordinador de importación separado del propietario PDFium y del scheduler. Formatos divididos en archivos comprimidos, libros, cómics, MuPDF, procesos, límites y caché; identidad persistente en source.
- A03: almacenamiento separado de sus modelos; PDFium dividido por apertura, metadatos, geometría, texto, rasterización, marcas y exportación. Ningún handle sale de runtime.
- A04: workspace con crabpdf-core; tipos, errores, geometría y anclas conservan su identidad.
- A05: crabpdf-application y crabpdf-runtime extraídos. Catálogo de formatos y reconciliación del perfil son reglas sin I/O. Sólo core/serde son dependencias productivas de application.
- A06: composición de servicios, diálogos, ventanas y etiquetas/atajos en desktop. La UI se divide en acciones, eventos, entrada, solicitudes y sesión. ProfileSession elimina la alternativa de escribir a disco desde un frame y conserva el baseline de cada ventana.
- Cierre: el worker desconecta los canales antes de reunir importador y renderizador; un fallo de construcción también detiene el renderizador ya creado. La prueba del canal lleno cubre ambos productores. La espera no interrumpe una llamada PDFium en curso.
- Repositorio: logo en assets; fixtures PDF en tests/fixtures; ejemplos ejecutables Rust en examples. Se retiran los archivos originales movidos, sin implementaciones duplicadas.
- Distribución: lints/pruebas recorren el workspace; CI comprueba las bibliotecas internas sin motores; inventarios excluyen todos los workspace_members. El guard de publicación incluye crates/assets. Los paquetes Linux aceptan un destino separado, para validar sin sobrescribir artefactos anteriores.

## Contratos conservados

Mismas rutas de perfil, variables de entorno, TOML, esquema 2 de marcas, claves de fuente y caché v4. Mismos límites de colas, prioridades y presupuestos CPU/GPU/texto. Se conserva el descarte por documento/revisión/generación. Las reexportaciones desktop mantienen una sola implementación; la persistencia explícita pasa a ProfileStore.

PDFium y los conversores continúan siendo nativos. El puente MuPDF de metadatos permanece junto a su adaptador; retirarlo sin un sustituto equivalente perdería la navegación por capítulos. [Alcance Rust](../../crates/runtime/src/formats/mupdf/README.md).

## Validación

Código validado: `52dc9be2b8c7a3ed1873dffc19879a376a713f20`. [CI completa](https://github.com/MHAlberto/crabpdf/actions/runs/40392802482), con los cinco jobs finalizados correctamente. [Registro verificable](evidence/a03c-a07-validation.json): hashes de entradas, logs, ejecutables y paquetes.

| Comprobación | Resultado |
| --- | --- |
| Bibliotecas internas sin PDFium/ventana | 10 pruebas core/application, Windows y Linux |
| Windows x64, workspace y pruebas nativas | 105 casos únicos aprobados, incluyendo las omitidas por defecto; no se cuentan dos veces los helpers de procesos |
| Clippy y formato | Workspace completo y ejemplos; sin advertencias. CI en los cinco targets |
| Linux, workspace | 86 casos únicos ejecutados en Debian Bookworm; las 18 omisiones iniciales se cubrieron después con 15 casos nativos en Ubuntu y 3 de marcas/recuperación en Debian |
| Ubuntu instalado, PDFium y formatos | 8 pruebas PDF y 7 de formatos/marcas en release, con bibliotecas del DEB instalado |
| Interfaz Windows x64 | 80 capturas: 16 de interacción, 16 de formatos y 24 por cada estilo de barra; ejecutables dev con eframe/PDFium reales |
| Fullscreen Windows | 3 mediciones físicas por estilo de barra; restauración normal/maximizada y cliente de 1920 × 1080 |
| Interfaz Ubuntu | 45 capturas de interacción, 16 de formatos y una del ejecutable instalado; release con Xvfb/Mesa/Openbox |
| ZIP e instalador Windows x64 | ZIP: 607 entradas verificadas; instalador: 609 entradas de manifiesto más el propio manifiesto; instalación, upgrade, bloqueo y desinstalación conservando archivos ajenos |
| Windows x86 | CI: lints, workspace, PDF nativo, formatos, release, instalador/upgrade/uninstall; no se repite GUI local en esta extracción |
| TAR y DEB Linux | 408 hashes verificados en cada paquete y tras instalar el DEB |
| Dependencias y distribución | 342 dependencias externas en el inventario Windows; los cuatro miembros locales quedan fuera. Se preservan recursos y avisos |
| macOS ARM/Intel | CI aprobada, incluidas pruebas nativas PDF; sin revisión física de interfaz ni paquetes Mac en esta fase |

Las pruebas nuevas protegen el grafo de paquetes, las rutas de servicios gráficos/I/O en las bibliotecas internas, el flush/merge de sesiones asíncronas y la conservación de archivos cuando falla una escritura. Las pruebas gráficas simulan RawInput en una aplicación nativa; no sustituyen toda la matriz de dispositivos y compositores. El instalador no tiene certificado y no se declara revisada manualmente su interfaz.

Los artefactos locales están en `artifacts/migration/`, fuera de Git. La publicación pública 0.4.0 y sus assets permanecen intactos. R09 se cierra como extracción de arquitectura; A00 y la parte cuantitativa de A07 permanecen abiertas.

## Rendimiento y límites

El ejecutable release Windows x64 pasa de 11 473 920 a 11 400 192 bytes (−0,64 %). El ZIP, instalador, TAR y DEB se comparan por separado en el registro de evidencia; el tamaño del ejecutable no equivale al tamaño del paquete.

Se conservó el ejecutable y probe de la base para comparar en la misma máquina. El primer recorrido de Measure-Architecture llegó a la fase 9 (reposo) y agotó el timeout sin emitir timings.json. Ese intento no demuestra una mejora ni un empeoramiento del lector; su log queda en artifacts/migration/performance-before.log. No se publican medianas inventadas ni se compara una muestra parcial con una completa.

A00 y la aceptación cuantitativa de A07 siguen necesitando mediciones comparables del corpus y hardware adecuados. La separación del código puede aceptarse funcionalmente sin declarar cerrada esa evidencia. No equivale a cerrar V1, la equidad del scheduler, el motor Rust puro ni la matriz gráfica completa.
