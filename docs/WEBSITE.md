# Sitio web de CrabPDF

Dirección: https://mhalberto.github.io/crabpdf/

La web vive en este repositorio, dentro de `docs`. Usa HTML semántico, JavaScript nativo y Tailwind CSS **4.3.3**, fijado en el lockfile. No utiliza un framework de interfaz, fuentes remotas, CDN de estilos ni servicios de analítica.

## Archivos

- `index.html`: contenido, enlaces de descarga, versión pública y estado del proyecto.
- `site/app.js`: tema persistente, menú móvil, pestañas accesibles y copia del comando Ubuntu.
- `site/styles/input.css`: tokens de la aplicación y entrada Tailwind.
- `site/assets/site.css`: CSS minificado, versionado para abrir la página sin instalar herramientas.
- `site/assets/`: logo original y tres capturas reales con documentos de prueba. No añadir libros personales.
- `site/build.mjs`: copia únicamente siete archivos públicos a `dist/site`; no publica los documentos técnicos ni node_modules.
- `site/check.mjs`: verifica recursos, IDs, destinos internos, referencias accesibles y dimensiones.
- `../.github/workflows/pages.yml`: validación y despliegue independiente de la aplicación Rust.

Los colores y radios siguen `src/ui/theme.rs`: fondo carbón, superficies compactas y acento azul. El tema claro conserva los colores equivalentes de la aplicación.

## Editar y comprobar

Se necesita Node.js 24 (la CI usa esa versión). Desde la raíz:

```sh
npm --prefix docs ci
npm --prefix docs run build
npm --prefix docs run check
python -m http.server 4173 --bind 127.0.0.1 --directory dist/site
```

Abre http://127.0.0.1:4173/. Para regenerar los estilos al editar, ejecuta `npm --prefix docs run dev` en otra terminal. Esa orden actualiza el CSS de docs; vuelve a ejecutar build para refrescar la copia de dist.

Tras modificar HTML, clases o estilos, regenera y guarda también `site/assets/site.css`. El navegador recibe el CSS ya compilado; Tailwind y Node sólo se necesitan para construir.

Antes de subir, comprueba escritorio y móvil, ambos temas, menú con Escape, flechas/Home/End en pestañas, las capturas, los enlaces de descarga y el comando de Ubuntu. El contenido esencial, las descargas y las preguntas frecuentes siguen disponibles con JavaScript desactivado. Se respeta Reducir movimiento del sistema.

## Versiones y descargas

La publicación pública es **0.4.0**. **0.4.1** existe en el código y como compilación local; la web no presenta sus archivos como descargas públicas. macOS se presenta como desarrollo, sin un instalador disponible.

Cuando se publique una entrega:

1. Verifica la publicación y los nombres exactos de sus assets en GitHub.
2. Actualiza en index.html los enlaces, tamaños, versión pública, notas, hashes y comando Ubuntu.
3. Actualiza el estado de desarrollo por separado. No convertir una compilación local en un enlace de descarga inexistente.
4. Conserva el aviso de ausencia de firma digital hasta disponer de firma válida; no declarar una certificación de seguridad.
5. Regenera el CSS, comprueba la página y guarda los cambios.

Las capturas y el logo se sirven localmente. Las descargas, fuentes y licencias enlazan a este mismo proyecto en GitHub.

## Despliegue

Pages debe usar **GitHub Actions**, con el entorno `github-pages`. El workflow construye el sitio, comprueba sus referencias y sube únicamente `dist/site`. El despliegue tiene permisos de Pages y OIDC; la construcción sólo tiene lectura.

Un push a main que cambie los archivos de la web dispara la publicación. Un pull request sólo construye y verifica. También se puede ejecutar manualmente desde Actions. La CI Rust omite cambios exclusivamente web para evitar cinco compilaciones nativas innecesarias.

El sitio utiliza rutas relativas para funcionar bajo `/crabpdf/`. Si se cambia el nombre del repositorio o se añade un dominio, hay que actualizar canonical, las etiquetas sociales y esta dirección. No hay router ni servicio que requiera un servidor adicional.

Referencias de implementación: [Tailwind CLI](https://tailwindcss.com/docs/installation/tailwind-cli) y [workflows de GitHub Pages](https://docs.github.com/en/pages/getting-started-with-github-pages/using-custom-workflows-with-github-pages).
