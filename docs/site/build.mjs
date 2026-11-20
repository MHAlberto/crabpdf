import { mkdir, copyFile, readFile, writeFile, readdir, stat } from "node:fs/promises";
import path from "node:path";
import { createHash } from "node:crypto";
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
const extensions = await buildExtensions(docs, repo, output);
const reader = "screenshots/0.6.0-reading.png";
const search = "screenshots/0.6.0-search.png";
const manager = "screenshots/0.7.0-extensions.png";
const imageSize = {};
for (const [key, filename] of [["reader", reader], ["search", search], ["manager", manager]]) {
  const png = await readFile(path.join(docs, filename));
  if (png.length < 24 || png.toString("ascii", 12, 16) !== "IHDR") {
    throw new Error("Invalid screenshot: " + filename);
  }
  imageSize[key + "_width"] = png.readUInt32BE(16);
  imageSize[key + "_height"] = png.readUInt32BE(20);
}
const revisions = {};
for (const [key, filename] of [["css", "site/styles/site.css"], ["js", "site/app.js"]]) {
  const bytes = await readFile(path.join(docs, filename));
  revisions[key + "_revision"] = createHash("sha256").update(bytes).digest("hex").slice(0, 12);
}
const pages = await sitePages(docs, release, extensions.cards, { ...imageSize, ...revisions });
for (const page of pages) {
  const destination = path.join(output, page.path);
  await mkdir(path.dirname(destination), { recursive: true });
  await writeFile(destination, page.html);
}

// One canonical source for the logo and screenshots; only public files are copied.
const assets = [
  [path.join(docs, "site/app.js"), "site/app.js"],
  [path.join(repo, "assets/crabpdf-logo.svg"), "site/assets/crabpdf-logo.svg"],
  [path.join(docs, "site/styles/site.css"), "site/assets/site.css"],
  [path.join(docs, reader), "site/assets/reading.png"],
  [path.join(docs, search), "site/assets/search.png"],
  [path.join(docs, manager), "site/assets/extension-manager.png"],
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
const files = [...pages.map((page) => page.path), ...extensions.files, ...guides.values(), ...assets.map(([, relative]) => relative)];
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
const bytes = (await Promise.all(files.map(async (file) => (await stat(path.join(output, file))).size)))
  .reduce((sum, size) => sum + size, 0);
console.log("Website " + version + ": " + files.length + " public files, " + bytes + " bytes in dist/site.");
