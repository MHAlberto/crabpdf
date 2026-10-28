# Contribuir a CrabPDF

Gracias por ayudar a mejorar el lector. El trabajo actual avanza hacia 1.0; consulta el [alcance](docs/ROADMAP.md) y el [backlog](docs/V1_BACKLOG.csv) antes de proponer una función de paridad.

## Preparar un cambio

1. Trabaja desde una rama con un objetivo concreto.
2. Consulta la [guía de desarrollo](docs/DEVELOPMENT.md) y reproduce el problema con un documento que puedas compartir.
3. Mantén el cambio acotado y explica el comportamiento esperado.
4. Ejecuta las comprobaciones adecuadas y registra plataforma, comandos y resultados.
5. Actualiza las guías cuando cambien el comportamiento, las rutas o los requisitos.

No necesitas descargar PDFium para probar core y application. Los cambios gráficos o de importadores requieren además sus motores y recorridos nativos.

## Límites del código

- `core` contiene tipos y geometría; `application`, reglas de lectura y perfil. No acceden a UI, disco o motores.
- `runtime` mantiene la propiedad de PDFium y adapta cachés, archivos e importadores.
- `desktop` compone servicios nativos y `ui` presenta e interpreta interacciones.
- Evita I/O síncrono en un frame, copias grandes sin medición y dependencias añadidas sólo por comodidad.
- Conserva límites de memoria/importación, cancelación y compatibilidad de perfiles.
- Las restricciones de unsafe y Clippy están en Cargo.toml; la dirección de dependencias se comprueba en tests/architecture.rs.

[Arquitectura completa](docs/ARCHITECTURE.md).

## Validar

Para Rust, ejecuta formato, Clippy y las pruebas del workspace descritas en Desarrollo. Añade o ajusta una prueba cuando proteja un comportamiento relevante; no basta con reproducir la implementación en otro archivo.

Para documentación:

```sh
node docs/tools/check-links.mjs
```

Para la web:

```sh
npm --prefix docs ci
npm --prefix docs run build
npm --prefix docs run check
```

Revisa móvil y escritorio, temas, teclado y movimiento reducido. La salida pública se genera en dist/site. [Guía de la web](docs/WEBSITE.md).

Para rendimiento, conserva corpus, entorno y muestras. Distingue apertura, CPU, GPU, memoria y tamaño de distribución. Un microbenchmark o una sola máquina no demuestra que toda la aplicación sea más rápida.

## Qué incluir en un PR

Describe el problema, el resultado del cambio y su validación. Señala los límites que no hayas comprobado. Para cambios visuales, añade capturas con documentos de prueba.

No incluyas perfiles personales, PDFs privados, descargas nativas, binarios compilados, node_modules ni logs locales. Conserva las licencias y la procedencia de los recursos redistribuidos.

Las etiquetas y assets públicos de una versión no se reemplazan durante una limpieza. El procedimiento de publicación está en [Desarrollo](docs/DEVELOPMENT.md).
