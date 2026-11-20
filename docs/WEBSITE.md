# Sitio web de CrabPDF

[Web pública](https://mhalberto.github.io/crabpdf/)

La web usa HTML, CSS y JavaScript nativos. Node ejecuta el ensamblado y las comprobaciones. El navegador recibe archivos estáticos. Los estilos, el logo y las capturas se sirven desde el sitio.

| Elemento | Fuente |
| --- | --- |
| Encabezado, pie y metadatos | `docs/index.html` |
| Contenido de las páginas | `docs/site/home.html`, `catalog.html`, `documentation.html` y `history.html` |
| Versión, paquetes y notas | `docs/site/release.json` y `docs/releases/` |
| Catálogo | `docs/site/extensions.json` |
| Interacciones y estilo | `docs/site/app.js` y `docs/site/styles/site.css` |
| Capturas de CrabPDF | `docs/screenshots/0.6.0-reading.png` y `docs/screenshots/0.6.0-search.png` |
| Resultado | `dist/site/`, ignorado por Git |

Las dos capturas muestran CrabPDF 0.6.0 en Windows con Series de Fourier, un PDF original preparado como documento de muestra.

## Construir y comprobar

Desde la raíz, con Node.js 22 o posterior y los ZIP de extensiones preparados en `artifacts/site-extensions/`:

```sh
node docs/site/build.mjs
node docs/site/check.mjs
node docs/tools/check-links.mjs
python -m http.server 4173 --bind 127.0.0.1 --directory dist/site
```

Abre [la vista local](http://127.0.0.1:4173/). Para previsualizar sin todos los ZIP, usa `CRABPDF_CATALOG_PREVIEW=1`. El modo de publicación exige todos los paquetes configurados. Si hay recursos que ya no pertenecen al sitio en `dist/site/`, retira sólo esos archivos generados antes de reconstruir.

Comprueba las descargas de los tres sistemas, los enlaces de las cuatro páginas, el tema, el copiado y la navegación en pantallas estrechas. Las descargas y los códigos siguen disponibles con JavaScript desactivado.

## Actualizar y publicar

1. Publica los paquetes con el procedimiento de [Desarrollo](DEVELOPMENT.md).
2. Añade su registro de procedencia en `docs/releases/` y actualiza la versión de `docs/site/release.json`.
3. Construye la web y ejecuta las comprobaciones. GitHub Pages despliega la salida validada de `main` mediante [pages.yml](../.github/workflows/pages.yml).

Las rutas públicas incluyen `/`, `/extensions/`, `/documentacion/` y `/historial/`, bajo `/crabpdf/`. El lector consulta `/extensions/catalog-v1.json`.
