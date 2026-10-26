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

El registro final de comandos, plataformas, paquetes y resultados se completa después de ejecutar los checks. Las pruebas nuevas protegen el grafo de paquetes, la ausencia de rutas de servicios gráficos/I/O en las bibliotecas internas, el flush/merge de sesiones asíncronas y la conservación de archivos cuando falla una escritura.

## Rendimiento y límites

Se conservó el ejecutable y probe de la base para comparar en la misma máquina. El primer recorrido de Measure-Architecture llegó a la fase 9 (reposo) y agotó el timeout sin emitir timings.json. Ese intento no demuestra una mejora ni un empeoramiento del lector; su log queda en artifacts/migration/performance-before.log. No se publican medianas inventadas ni se compara una muestra parcial con una completa.

A00 y la aceptación cuantitativa de A07 siguen necesitando mediciones comparables del corpus y hardware adecuados. La separación del código puede aceptarse funcionalmente sin declarar cerrada esa evidencia. No equivale a cerrar V1, la equidad del scheduler, el motor Rust puro ni la matriz gráfica completa.
