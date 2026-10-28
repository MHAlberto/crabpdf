# Sitio web

Dirección pública: https://mhalberto.github.io/crabpdf/

La fuente vive en docs: HTML, JavaScript nativo y Tailwind CSS **4.3.3**, fijado en el lockfile. Node sólo interviene al construir; el navegador recibe siete archivos estáticos. El sitio no usa fuentes remotas, CDN de estilos ni analítica.

## Fuentes y salidas

| Elemento | Fuente única |
| --- | --- |
| Contenido y estructura | `docs/index.html` |
| Versión pública | `docs/site/release.json` |
| Tamaños y assets de la entrega | `docs/releases/v<versión>-publication.json` |
| Interacciones | `docs/site/app.js` |
| Estilos | `docs/site/styles/input.css` |
| Logo | `assets/crabpdf-logo.svg` |
| Capturas | `docs/screenshots/` |
| Sitio generado | `dist/site/`, ignorado por Git |

El HTML fuente contiene marcadores como `{{version}}`; no es el archivo servido directamente. build.mjs resuelve la versión y tamaños desde la publicación registrada y copia logo/capturas a la salida. El CSS se genera directamente en dist. No se mantienen copias de imágenes ni CSS compilado dentro de docs/site.

Los colores y radios siguen la aplicación: fondo carbón, superficies compactas y acento azul; el tema claro usa sus equivalentes. La modificación de contenido mantiene esa identidad.

## Construir y comprobar

Con Node.js 24, desde la raíz:

```sh
npm --prefix docs ci
npm --prefix docs run build
npm --prefix docs run check
node docs/tools/check-links.mjs
python -m http.server 4173 --bind 127.0.0.1 --directory dist/site
```

Abre http://127.0.0.1:4173/. `npm --prefix docs run dev` observa cambios del CSS y actualiza dist; después de modificar HTML, metadatos, JS o imágenes, vuelve a ejecutar build.

check.mjs inspecciona la salida real: recursos, fragmentos, referencias accesibles, imágenes y marcadores sin resolver. El build rechaza archivos inesperados en la salida para que sólo se publique la lista explícita. Si quedan archivos de una revisión anterior, revisa esa carpeta antes de retirarlos.

Comprueba temas, navegación móvil, Escape, flechas/Home/End en pestañas, copiado del comando Ubuntu y ausencia de desplazamiento horizontal. El contenido esencial y las descargas funcionan sin JavaScript; se respeta Reducir movimiento.

## Actualizar una entrega

La versión pública actual está en release.json. Debe corresponder a una entrega publicada, con su registro de procedencia y assets en GitHub.

1. Publica y verifica los paquetes con el procedimiento de [Desarrollo](DEVELOPMENT.md).
2. Conserva su registro de publicación y cambia únicamente la versión de release.json.
3. El build obtiene los tamaños de ese registro y genera enlaces y comando Ubuntu con la misma versión.
4. Actualiza las notas y el README; distingue una descarga pública de una compilación en desarrollo.
5. Construye, comprueba y sube los cambios.

macOS se presenta como desarrollo sin instalador público. El aviso de ausencia de firma sólo cambia cuando la entrega tenga una firma válida; no se deduce confianza del logo o del asistente.

## Desplegar

GitHub Pages usa el entorno github-pages y [pages.yml](../.github/workflows/pages.yml). Los cambios de la fuente web, sus recursos canónicos o el registro de publicación disparan el build. Un pull request verifica, pero no despliega.

El workflow sube sólo dist/site, sin documentación interna, dependencias npm o perfiles. Los cambios exclusivamente documentales tienen su [comprobación de enlaces](../.github/workflows/docs.yml) y no necesitan cinco compilaciones nativas.

Las rutas públicas siguen siendo relativas bajo /crabpdf/. Si cambia el repositorio o el dominio, actualiza canonical, etiquetas sociales y esta dirección.

Referencias: [Tailwind CLI](https://tailwindcss.com/docs/installation/tailwind-cli) y [GitHub Pages con Actions](https://docs.github.com/en/pages/getting-started-with-github-pages/using-custom-workflows-with-github-pages).
