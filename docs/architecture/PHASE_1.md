# Migración de arquitectura: primeros cortes

Base de aplicación: 0.3.6. Dirección: [ADR-0001](../adr/0001-modular-architecture.md). Secuencia completa: [ARCHITECTURE_MIGRATION.md](../ARCHITECTURE_MIGRATION.md).

## Implementación

| Corte | Resultado | Estado |
| --- | --- | --- |
| A00 | Instrumento de disponibilidad de UI, observador Windows, corpus determinista y cinco muestras anteriores al cambio | Base local registrada; el hito completo sigue abierto |
| A01 | Consumidor de título, contador y aviso tipados; precedencia del título importado obtenida desde datos | Implementado y probado en Windows x64 |
| A02a | Modelo y reglas de favoritos en `src/application/favorites.rs`; acciones y resultados compartidos por teclado y panel | Implementado y validado en Windows x64 |
| A02b–A07 | Pestañas lógicas, búsqueda, servicios, workspace y demás extracciones | Pendientes al cerrar este corte; continuación de A02b en [PHASE_2.md](PHASE_2.md) |

No se han introducido los cuatro paquetes aún. `src/application` es el primer módulo de aplicación dentro del paquete actual, conforme a la secuencia incremental.

## Metadatos

`Tab::apply_details` utiliza `DocumentDetails.title`, `annotation_count` y `annotation_warning`. Las etiquetas de `properties` quedan para presentación. PDFium obtiene el título desde su tipo de metadato y el importador lo sustituye cuando aporta uno no vacío.

Se corrige una inconsistencia visible: el backend ya proporcionaba el título importado, pero la biblioteca elegía la primera fila «Título», que podía ser la del PDF interno. Ahora conserva la precedencia del libro original. Los avisos mantienen su explicación de que la lectura sigue disponible.

Las pruebas cubren ausencia de título, fallback al título PDF, título importado vacío o presente, etiquetas cambiadas o contradictorias, contador tipado y aislamiento de entradas de biblioteca.

## Favoritos

`Favorite` se define una sola vez en el módulo de aplicación y se reexporta desde `config`. Se mantienen los campos `path/page/label`, el TOML, las etiquetas antiguas y las firmas públicas existentes de `Preferences`.

Las acciones producen `Changed`, `Navigate` o `Unchanged`. El módulo no abre archivos, escribe perfiles, crea ventanas ni carga handles nativos. La UI aplica navegación al estado de lectura y solicita el guardado al escritor existente. Se conserva el guardado inmediato solicitado por el toggle y la agrupación diferida de eliminaciones del panel.

Se mantienen aislamiento por libro, orden por página, retirada de duplicados antiguos y navegación acotada si cambia el número de páginas. Las pruebas de contrato verifican compatibilidad de perfiles y conservación de zoom, giro, modo, manga y colores.

## Mediciones y evidencia

[Método, reproducción y límites](MEASUREMENTS.md). La referencia previa está en [a00-before-windows-x64.json](evidence/a00-before-windows-x64.json). Los informes locales completos quedan en `artifacts/architecture-phase1/`.

La línea base funcional anterior al cambio pasó 28 pruebas unitarias, 7 pruebas nativas PDF, formatos/marcas y pruebas de procesos, posición de lectura y recuperación. La nueva prueba nativa de metadatos eleva el grupo PDF a 8 casos.

La validación posterior pasó:

- Formato y Clippy para todos los targets del paquete en Windows x64.
- 32 pruebas unitarias y la suite completa de integración, incluidas 8 nativas PDF, 7 de formatos/marcas, sincronización entre procesos, recuperación, perfiles y posición de lectura.
- Contrato de favoritos con perfil antiguo y conservación de ajustes de vista.
- 50 capturas del recorrido de favoritos/pestañas con barra propia y nativa; [seis mediciones de pantalla completa](evidence/a02-windows-geometry.json).
- Transiciones normales y reducidas; sesión de lectura en tres procesos con cambio de escala/tamaño y prioridad CLI.

La [CI del código 06e381e](https://github.com/MHAlberto/crabpdf/actions/runs/40002182785) comprueba los cinco targets existentes: Windows x64/x86, Ubuntu x64 y macOS Intel/ARM. La ejecución local descrita aquí es Windows x64; la CI no incluye todos los recorridos gráficos.

La [evidencia de mediciones](evidence/a02-measurements-windows-x64.json) conserva cuatro series de cinco recorridos: referencia inicial, primera extracción, control con el ejecutable original verificado por hash y confirmación de la extracción.

| Operación, mediana en ms | Referencia inicial | Extracción inicial | Original como control | Extracción confirmada |
| --- | --- | --- | --- | --- |
| Apertura PDF | 40.6 | 85.8 | 95.3 | 80.2 |
| Arranque hasta disponibilidad | 296.6 | 352.0 | 355.3 | 342.1 |
| Página precargada | 15.2 | 28.5 | 25.9 | 25.0 |
| Última página sin precargar | 28.9 | 40.9 | 41.5 | 41.2 |
| Scroll con estabilidad de 80 ms | 85.0 | 90.1 | 94.1 | 89.4 |
| Reapertura en el mismo proceso | 56.0 | 62.3 | 66.5 | 63.4 |
| EPUB sin caché de importación | 142.1 | 146.0 | 144.5 | 144.4 |

El mismo binario original también cambió de latencia: hay deriva del entorno. La confirmación no muestra una ralentización repetible superior al umbral provisional del 5 % frente al control contemporáneo en estas rutas. No es una garantía estadística ni una mejora atribuida a la migración. Se conservan todas las series; no se descarta la primera medición desfavorable.

La mediana del pico de working set fue aproximadamente 119.4 MiB en el control y 118.8 MiB en la confirmación. El ejecutable principal pasó de 11 390 976 a 11 402 240 bytes (+0.10 %). Los intervalos de CPU en reposo fueron de cuatro segundos y se conservan individualmente en JSON; no se usa su mediana cero como promesa de consumo nulo.

Las mediciones de working set son del proceso de la app; CPU/RSS de conversores, asignación GPU, tamaño del paquete completo, rendimiento con corpus grande y el resto de plataformas siguen pendientes. No se declara una mejora de rendimiento por esta extracción.

No se publicaron paquetes nuevos ni se cambió la versión: son cortes de desarrollo independientes, identificados por commit. Los releases existentes conservan sus artefactos.

## Próximo corte

El siguiente corte previsto al cerrar esta fase fue A02b: separar la pestaña lógica de su estado visual, conservando el ID estable, la posición normalizada y las reglas de navegación. Su implementación y validación se registran en [PHASE_2.md](PHASE_2.md).
