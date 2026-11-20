import { mkdir, copyFile, readFile, writeFile, readdir, stat } from "node:fs/promises";
import path from "node:path";
import { buildExtensions, escape } from "./extensions.mjs";
import { sitePages } from "./pages.mjs";
import { fileURLToPath } from "node:url";

const docs = fileURLToPath(new URL("../", import.meta.url));
const repo = path.resolve(docs, "..");
const output = path.join(repo, "dist/site");
const { version } = JSON.parse(await readFile(path.join(docs, "site/release.json"), "utf8"));
if (!/^\d+\.\d+\.\d+(?:-[a-zA-Z0-9.-]+)?$/.test(version)) throw new Error("Invalid public release version.");
const release = JSON.parse(await readFile(path.join(docs, "releases/v" + version + "-publication.json"), "utf8"));
if (release.version !== version) throw new Error("Release metadata does not match its publication record.");
const megabytes = (suffix) => {
  const asset = release.assets.find((asset) => asset.name === "crabpdf-" + version + suffix);
  if (!asset || !Number.isSafeInteger(asset.bytes) || asset.bytes <= 0) throw new Error("Missing release asset: " + suffix);
  return String(Math.round(asset.bytes / 1_000_000));
};
const extensions = await buildExtensions(docs, repo, output);
const tokens = {
  extension_cards: extensions.cards,
  version,
  windows_installer_mb: megabytes("-win-x64-setup.exe"),
  windows_zip_mb: megabytes("-win-x64.zip"),
  ubuntu_deb_mb: megabytes("-ubuntu24.04-amd64.deb"),
  linux_tar_mb: megabytes("-linux-x64.tar.gz"),
};
const source = await readFile(path.join(docs, "index.html"), "utf8");
const pages = await sitePages(source, docs, version, escape);
for (const page of pages) {
  const html = page.html.replace(/\{\{\s*([a-z_]+)\s*\}\}/g, (_, name) => {
    if (!Object.hasOwn(tokens, name)) throw new Error("Unknown website token: " + name);
    return tokens[name];
  });
  const destination = path.join(output, page.path);
  await mkdir(path.dirname(destination), { recursive: true });
  await writeFile(destination, html);
}

// One canonical source for the logo and screenshots; only public files are copied.
const assets = [
  [path.join(docs, "site/app.js"), "site/app.js"],
  [path.join(repo, "assets/crabpdf-logo.svg"), "site/assets/crabpdf-logo.svg"],
  [path.join(docs, "screenshots/dark.png"), "site/assets/reading.png"],
  [path.join(docs, "screenshots/highlight.png"), "site/assets/marks.png"],
  [path.join(docs, "screenshots/library-formats.png"), "site/assets/library.png"],
];
for (const [source, relative] of assets) {
  const destination = path.join(output, relative);
  await mkdir(path.dirname(destination), { recursive: true });
  await copyFile(source, destination);
}
const guides = new Map([
  ["USAGE.md", "documentacion/uso.md"],
  ["EXTENSIONS.md", "documentacion/extensiones.md"],
]);
const siteBase = "https://mhalberto.github.io/crabpdf/";
for (const [name, relative] of guides) {
  const markdown = await readFile(path.join(docs, name), "utf8");
  const destinations = new Map();
  for (const [, href] of markdown.matchAll(/\]\(([^\s)]+)\)/g)) {
    if (/^(?:[a-z]+:|#|\/)/i.test(href)) continue;
    const [pathname, fragment] = href.split("#", 2);
    const local = path.resolve(docs, pathname);
    const inRepo = path.relative(repo, local).split(path.sep).join("/");
    if (path.isAbsolute(inRepo) || inRepo.startsWith("../")) {
      throw new Error("Link outside the repository in " + name + ": " + href);
    }
    const info = await stat(local);
    const guide = guides.get(pathname);
    const repository = "https://github.com/MHAlberto/crabpdf/";
    const base = guide ? siteBase + guide
      : inRepo.startsWith("docs/screenshots/") ? "https://raw.githubusercontent.com/MHAlberto/crabpdf/main/" + inRepo
      : repository + (info.isDirectory() ? "tree/" : "blob/") + "main/" + inRepo;
    destinations.set(href, base + (fragment ? "#" + fragment : ""));
  }
  const destination = path.join(output, relative);
  await mkdir(path.dirname(destination), { recursive: true });
  await writeFile(destination, markdown.replace(/\]\(([^\s)]+)\)/g,
    (match, href) => destinations.has(href) ? "](" + destinations.get(href) + ")" : match));
}
// Tailwind writes its output here before this script runs.
const files = [...pages.map((page) => page.path), "site/assets/site.css", ...extensions.files, ...guides.values(), ...assets.map(([, relative]) => relative)];
async function walk(folder, prefix = "") {
  const result = [];
  for (const entry of await readdir(folder, { withFileTypes: true })) {
    if (entry.isSymbolicLink()) throw new Error("The site output cannot contain symlinks.");
    const relative = prefix + entry.name;
    if (entry.isDirectory()) result.push(...await walk(path.join(folder, entry.name), relative + "/"));
    else result.push(relative);
  }
  return result;
}
if (JSON.stringify((await walk(output)).sort()) !== JSON.stringify([...files].sort())) {
  throw new Error("Unexpected files in dist/site. Review the output before publishing.");
}
const bytes = (await Promise.all(files.map(async (file) => (await readFile(path.join(output, file))).length)))
  .reduce((sum, size) => sum + size, 0);
console.log("Website " + version + ": " + files.length + " public files, " + bytes + " bytes in dist/site.");
