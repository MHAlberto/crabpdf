# Adaptador MuPDF

La conversión, los límites, la cancelación, la validación y la publicación de caché están implementados en Rust.

`import_info.js` es el pequeño puente que ejecuta `mutool run` para consultar el título, el autor y el índice del libro con la misma paginación de MuPDF (612 × 792, fuente de 12 puntos). No ejecuta scripts aportados por los documentos. Se mantiene junto a su adaptador porque las APIs de layout y resolución de enlaces pertenecen a MuPDF y no a PDFium.

La separación en crates no sustituye PDFium, MuPDF, libmobi, DjVuLibre ni 7-Zip por implementaciones Rust. Retirar este puente exige conservar las páginas y destinos del índice en todos los formatos; eliminarlo sin ese reemplazo perdería funcionalidad. Las definiciones NSIS y los scripts de empaquetado son herramientas de plataforma, no dependencias de la lógica de lectura.
