import { readFile, readdir } from "node:fs/promises";
import path from "node:path";

export async function sitePages(source, docs, version, escape) {
  const repo = "https://github.com/MHAlberto/crabpdf";
  const main = source.match(/<main id="contenido"[^>]*>([\s\S]*?)<\/main>/)?.[1];
  if (!main) throw new Error("Missing site template main.");
  const section = (id) => {
    const result = main.match(new RegExp(`<section id="${id}"[\\s\\S]*?<\\/section>`))?.[0];
    if (!result) throw new Error("Missing site section: " + id);
    return result;
  };
  const downloads = section("descargas").replace('<h2 id="downloads-title"', '<h1 id="downloads-title"').replace('</h2>', '</h1>');
  const extensions = section("extensiones").replace('<h2 id="extensions-title"', '<h1 id="extensions-title"').replace('</h2>', '</h1>');
  const home = main.replace(section("descargas"), "").replace(section("extensiones"), "")
    .replace(section("proyecto"), "").replace('<h1 id="hero-title"', '<h2 id="hero-title"').replace('</h1>', '</h2>');

  const records = (await readdir(path.join(docs, "releases")))
    .filter((name) => /^v\d+\.\d+\.\d+\.md$/.test(name))
    .sort((a, b) => b.localeCompare(a, "en", { numeric: true }));
  const timeline = await Promise.all(records.map(async (name) => {
    const tag = name.slice(0, -3);
    const notes = await readFile(path.join(docs, "releases", name), "utf8");
    const versionLine = notes.split(/\r?\n/).find((line) => line && !/^[#>*-]/.test(line)) ?? "";
    const summary = versionLine.replace(/\[([^\]]+)\]\([^)]+\)/g, "$1").replaceAll("**", "").replaceAll("`", "");
    const published = await readFile(path.join(docs, "releases", tag + "-publication.json"), "utf8")
      .then((text) => JSON.parse(text).published_at_utc?.slice(0, 10))
      .catch((error) => {
        if (error.code !== "ENOENT") throw error;
        return "";
      });
    return `<li class="timeline-item">
      <div class="timeline-meta"><strong>${tag}</strong>${published ? `<time datetime="${escape(published)}">${escape(published)}</time>` : ""}</div>
      <div><p>${escape(summary.slice(0, 300))}</p><a class="text-link" href="${repo}/blob/main/docs/releases/${name}">Notas de la versión</a></div>
    </li>`;
  }));

  const pages = [
    { path: "index.html", title: "Descargas", content: downloads + '<section id="extensiones" class="shell py-8"><a class="text-link" href="./extensions/">Ver catálogo de extensiones</a></section>' + home },
    { path: "extensions/index.html", title: "Extensiones", content: extensions.replace("desde CrabPDF 0.5.2.", "desde CrabPDF 0.5.2. OCR y Pomodoro requieren la 0.5.5.") },
    { path: "documentacion/index.html", title: "Documentación", content: `<section class="shell py-16 sm:py-20">
      <h1 class="text-4xl font-semibold tracking-tight">Documentación</h1>
      <p class="mt-5 text-muted">Guías en Markdown para consultar con o sin conexión.</p>
      <div class="mt-8 max-w-2xl border-b border-line">
        <a class="download-choice" href="./uso.md" download>Descargar guía de uso<svg class="icon" aria-hidden="true"><use href="#i-download"/></svg></a>
        <a class="download-choice" href="./extensiones.md" download>Descargar guía de extensiones<svg class="icon" aria-hidden="true"><use href="#i-download"/></svg></a>
        <a class="download-choice" href="${repo}/blob/main/docs/README.md">Abrir el índice completo en GitHub</a>
      </div></section>` },
    { path: "historial/index.html", title: "Historial", content: `<section class="shell py-16 sm:py-20">
      <h1 class="text-4xl font-semibold tracking-tight">Historial de CrabPDF</h1>
      <p class="mt-5 text-muted">Cambios de cada versión. La descarga pública actual es la ${version}.</p>
      <ol class="mt-8 max-w-3xl">${timeline.join("\n")}</ol>
    </section>` },
  ];
  return pages.map((page) => {
    const root = page.path === "index.html" ? "./" : "../";
    const nav = pages.map(({ path, title }) => {
      const href = root + path.replace(/index\.html$/, "");
      return `<a class="nav-link" href="${href}"${path === page.path ? ' aria-current="page"' : ""}>${title}</a>`;
    }).join("\n");
    let html = source.replace(main, "\n" + page.content + "\n")
      .replace(/<nav id="main-nav"[^>]*>[\s\S]*?<\/nav>/, `<nav id="main-nav" aria-label="Navegación principal" class="site-nav">${nav}</nav>`)
      .replace(/<title>[^<]*<\/title>/, `<title>${page.title} · CrabPDF</title>`)
      .replace(/(<link rel="canonical" href="https:\/\/mhalberto\.github\.io\/crabpdf\/)[^"]*/, "$1" + page.path.replace(/index\.html$/, ""));
    if (root === "../") {
      html = html.replaceAll("./site/", "../site/")
        .replace('href="./" aria-label="CrabPDF, inicio"', 'href="../" aria-label="CrabPDF, inicio"')
        .replaceAll('href="#descargas"', 'href="../#descargas"');
    }
    return { ...page, html };
  });
}
