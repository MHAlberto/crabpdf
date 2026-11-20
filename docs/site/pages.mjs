import { readFile, readdir } from "node:fs/promises";
import path from "node:path";
import { escape } from "./extensions.mjs";

const repo = "https://github.com/MHAlberto/crabpdf";
const site = "https://mhalberto.github.io/crabpdf/";

const routes = [
  { route: "", file: "home", name: "Descargas", title: "CrabPDF | Lector PDF de escritorio",
    description: "Descarga CrabPDF para Windows, Linux o macOS. Lee y anota PDFs desde tu equipo." },
  { route: "extensions/", file: "catalog", name: "Extensiones",
    description: "Códigos y compatibilidad de las extensiones OCR, Pomodoro e Información de la selección." },
  { route: "documentacion/", file: "documentation", name: "Documentación",
    description: "Guías de uso, extensiones y desarrollo de CrabPDF." },
  { route: "historial/", file: "history", name: "Historial",
    description: "Notas y fechas de publicación de las versiones de CrabPDF." },
];

function render(source, values) {
  return source.replace(/\{\{\s*([a-z_]+)\s*\}\}/g, (_, name) => {
    if (!Object.hasOwn(values, name)) throw new Error("Unknown website token: " + name);
    return values[name];
  });
}

function downloadGroups(release) {
  const { version, assets } = release;
  const item = (suffix, label) => {
    const name = "crabpdf-" + version + suffix;
    const asset = assets.find((entry) => entry.name === name);
    const url = repo + "/releases/download/v" + version + "/" + name;
    if (!asset || asset.url !== url || !Number.isSafeInteger(asset.bytes) || asset.bytes <= 0) {
      throw new Error("Missing or invalid release asset: " + name);
    }
    const size = Math.round(asset.bytes / 1_000_000);
    return `<li><a href="${escape(url)}"><strong>${escape(label)}</strong><span>${size} MB</span></a></li>`;
  };
  const group = (id, label, support, entries, details = "") => `
    <section class="platform" id="${id}" aria-labelledby="${id}-title" data-download-label="Descargas para ${label}">
      <h3 id="${id}-title">${label}</h3>
      <p>${support}</p>
      <ul class="download-list">${entries.join("\n")}</ul>
      ${details}
    </section>`;
  const linuxHelp = `<details>
    <summary>Instalar el DEB</summary>
    <div class="command-line">
      <code id="ubuntu-command">sudo apt install ./crabpdf-${version}-ubuntu24.04-amd64.deb</code>
      <button type="button" class="button secondary" hidden data-copy="ubuntu-command" data-copied="Comando copiado." aria-describedby="copy-ubuntu">Copiar</button>
    </div>
    <p id="copy-ubuntu" class="copy-status" role="status"></p>
  </details>`;
  const groups = [
    group("windows", "Windows", "Windows 10 y 11", [
      item("-win-x64-setup.exe", "Instalador x64"),
      item("-win-x86-setup.exe", "Instalador x86"),
      item("-win-x64.zip", "Portable x64"),
      item("-win-x86.zip", "Portable x86"),
    ]),
    group("linux", "Linux", "x64 · Ubuntu 24.04 o glibc 2.36+", [
      item("-ubuntu24.04-amd64.deb", "Paquete DEB"),
      item("-linux-x64.tar.gz", "Archivo TAR.GZ"),
    ], linuxHelp),
  ];
  const mac = release.verification?.macos ?? [];
  if (mac.length > 0) {
    const minimums = [...new Set(mac.map((entry) => entry.minimum_macos))];
    const support = minimums.length === 1 ? "macOS " + escape(minimums[0]) + " o posterior" : "Consulta la compatibilidad de cada paquete";
    const links = mac.flatMap(({ architecture }) => {
      if (!["arm64", "x64"].includes(architecture)) throw new Error("Unknown macOS target.");
      const name = architecture === "arm64" ? "Apple Silicon" : "Intel";
      return [item("-mac-" + architecture + ".dmg", name + " (DMG)"),
        item("-mac-" + architecture + ".zip", name + " (ZIP)")];
    });
    groups.push(group("macos", "macOS", support, links));
  }
  return groups.join("\n");
}

async function releaseHistory(docs) {
  const directory = path.join(docs, "releases");
  const versions = (await readdir(directory)).filter((name) => /^v\d+\.\d+\.\d+\.md$/.test(name))
    .sort((a, b) => b.localeCompare(a, "en", { numeric: true }));
  const rows = await Promise.all(versions.map(async (name) => {
    const tag = name.slice(0, -3);
    const notes = await readFile(path.join(directory, name), "utf8");
    const line = notes.split(/\r?\n/).find((line) => line.trim() && !/^[#>*-]/.test(line)) ?? "";
    const summary = line.replace(/\[([^\]]+)\]\([^)]+\)/g, "$1").replace(/[*`]/g, "").trim();
    let date = "";
    try {
      const record = JSON.parse(await readFile(path.join(directory, tag + "-publication.json"), "utf8"));
      const timestamp = record.published_at_utc?.slice(0, 10) ?? "";
      if (/^\d{4}-\d{2}-\d{2}$/.test(timestamp)) {
        const label = new Intl.DateTimeFormat("es-MX", { timeZone: "UTC", dateStyle: "medium" })
          .format(new Date(timestamp));
        date = `<time datetime="${timestamp}">${escape(label)}</time>`;
      }
    } catch (error) {
      if (error.code !== "ENOENT") throw error;
    }
    return `<li>
      <div class="timeline-meta"><strong>${tag}</strong>${date}</div>
      <div><p>${escape(summary)}</p><a class="plain-link" href="${repo}/blob/main/docs/releases/${name}">Notas de ${tag}</a></div>
    </li>`;
  }));
  return rows.join("\n");
}

export async function sitePages(docs, release, extensionCards, assetTokens) {
  const template = await readFile(path.join(docs, "index.html"), "utf8");
  const version = release.version;
  const releaseUrl = repo + "/releases/tag/v" + version;
  if (release.release_url !== releaseUrl) throw new Error("Invalid release URL.");
  const checksums = release.assets.find((asset) => asset.name === "SHA256SUMS-v" + version + ".txt");
  const checksumsUrl = repo + "/releases/download/v" + version + "/SHA256SUMS-v" + version + ".txt";
  if (checksums?.url !== checksumsUrl) throw new Error("Missing release checksums.");
  const signatureNotes = [];
  if (release.limits?.some((limit) => /unsigned windows/i.test(limit))) {
    signatureNotes.push("Los instaladores de Windows se publican sin firma digital.");
  }
  if (release.verification?.macos?.some((item) => !item.notarized)) {
    signatureNotes.push("Los paquetes de macOS tienen firma ad hoc y se distribuyen sin notarización.");
  }
  const tokens = {
    version, release_url: escape(releaseUrl), checksums_url: escape(checksumsUrl),
    signature_note: escape(signatureNotes.join(" ")), platform_downloads: downloadGroups(release),
    extension_cards: extensionCards, timeline: await releaseHistory(docs), ...assetTokens,
  };
  return Promise.all(routes.map(async (page) => {
    const root = page.route ? "../" : "./";
    const navigation = routes.map((other) =>
      `<a href="${root}${other.route}"${page === other ? ' aria-current="page"' : ""}>${other.name}</a>`
    ).join("\n        ");
    const fragment = await readFile(path.join(docs, "site", page.file + ".html"), "utf8");
    const html = render(template, {
      ...tokens, root, navigation, title: escape(page.title ?? page.name + " | CrabPDF"),
      description: escape(page.description), canonical: site + page.route, content: render(fragment, tokens),
    });
    return { path: page.route + "index.html", html };
  }));
}
