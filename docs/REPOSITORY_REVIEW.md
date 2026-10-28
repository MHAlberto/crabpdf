# Revisión de mantenimiento

Fecha: 27 de octubre de 2026. Base: `c2401c4`, posterior a publicar 0.4.1. Esta revisión organiza el código y la información; no reemplaza el tag ni los paquetes públicos.

## Hallazgos y cambios

| Hallazgo | Cambio aplicado |
| --- | --- |
| README mezclaba instalación, desarrollo, historial y detalles internos | Resumen del producto, descargas y límites; guías separadas de uso y desarrollo |
| 12 informes antiguos convivían con las instrucciones vigentes | Se conservaron en history, con índice y aviso de contexto |
| 27 enlaces apuntaban a resultados locales ausentes | Se identificaron como registros locales no versionados y se corrigieron las rutas del historial |
| Logo y capturas estaban repetidos; una portada de biblioteca aparecía tres veces | Fuente única en assets y screenshots; el build copia sólo lo publicado |
| CSS compilado se mantenía junto al código fuente | Tailwind genera directamente en dist/site; no se versiona su salida |
| Versión y tamaños web se repetían manualmente | release.json selecciona la publicación; el build sustituye versión y calcula tamaños desde sus assets |
| panels.rs reunía 1.138 líneas de controles, panel lateral y diálogos | Se dividió en panels/mod.rs, sidebar.rs y dialogs.rs, preservando el alcance de métodos y los cuerpos |
| El recorrido gráfico podía no pintar una escena de zoom ya cacheada | Se invalida la UI al avanzar de escena, respetando la separación lógica/UI de egui 0.36 |
| El publicador usaba 0.4.0 por defecto | Obtiene la versión del paquete Cargo cuando no se pasa -Version |
| Arquitectura, plataforma macOS y plan seguían citando un estado antiguo | Se enlazó la publicación 0.4.1 y se distinguió CI de validación física |
| No había entrada para contribuir ni comprobación de enlaces | Guía, plantilla de PR, formulario de errores y workflow documental independiente |

Se retiraron **574 611 bytes** de duplicados y salidas generadas del árbol versionado. Esa cifra describe los seis archivos retirados, no la reducción del binario ni del historial completo de Git. Los recursos siguen presentes una vez en su fuente o se generan al construir.

La web conserva los mismos siete archivos públicos. Eliminar copias del repositorio mejora mantenimiento y tamaño del checkout; no demuestra que la descarga pública se haya reducido en esa misma cantidad.

## Qué se conserva y por qué

- Las licencias repetidas de componentes distintos conservan su procedencia y forman parte del empaquetado.
- Los PDF de prueba son originales, pequeños y necesarios para motores e interacción.
- Los ejemplos gráficos son instrumentos de regresión; no son demos descartables.
- src/lib.rs mantiene una fachada pública de compatibilidad sobre el workspace.
- El puente JavaScript de MuPDF sigue siendo requerido por su API; quitarlo rompería metadatos.
- Las evidencias JSON y hashes publicados no se reescriben para aparentar pruebas nuevas.
- No se borraron cachés, builds, paquetes, perfiles o documentos personales de los directorios locales.

## Prioridades siguientes

1. **Complejidad de UI:** viewer.rs y ReaderApp siguen concentrando coordinación. Extraer por responsabilidad sólo con contratos y recorridos que protejan selección, scroll y cambio de pestañas.
2. **Rendimiento:** completar las comparaciones de A00/A07 con corpus real y mediciones equivalentes. El experimento de caché descartado no debe reaplicarse sin evidencia.
3. **Paridad:** continuar las rutas del backlog; la reorganización no resuelve impresión, edición general, SyncTeX o accesibilidad.
4. **Distribución:** ampliar pruebas físicas de Windows 11, x86 nativo y Wayland; preparar la interfaz y empaquetado macOS.
5. **Publicaciones:** mantener una separación clara entre la versión pública y código posterior, conservando tags y assets inmutables.

## Verificación de esta revisión

La revisión local aprobó formato y Clippy sin advertencias, 90 pruebas de bibliotecas e integración, 47 capturas de refinamiento, el recorrido de favoritos/pestañas y 243 referencias documentales locales. La comparación de la extracción confirma que los cuerpos de los métodos se conservaron. Se comprobó también la generación del sitio desde fuentes canónicas. La web se revisa en móvil y escritorio antes del despliegue. Los logs de trabajo están en artifacts/repository-review; no se presentan como descargas versionadas.

La evidencia de los binarios públicos sigue en [publicación 0.4.1](releases/v0.4.1-publication.json). Los cambios de mantenimiento posteriores se consultan en Git y en sus ejecuciones CI.
