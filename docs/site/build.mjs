import { mkdir, copyFile, readFile, writeFile, readdir } from "node:fs/promises";
import path from "node:path";
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
const tokens = {
  version,
  windows_installer_mb: megabytes("-win-x64-setup.exe"),
  windows_zip_mb: megabytes("-win-x64.zip"),
  ubuntu_deb_mb: megabytes("-ubuntu24.04-amd64.deb"),
  linux_tar_mb: megabytes("-linux-x64.tar.gz"),
};
const source = await readFile(path.join(docs, "index.html"), "utf8");
const html = source.replace(/\{\{\s*([a-z_]+)\s*\}\}/g, (_, name) => {
  if (!Object.hasOwn(tokens, name)) throw new Error("Unknown website token: " + name);
  return tokens[name];
});
await mkdir(output, { recursive: true });
await writeFile(path.join(output, "index.html"), html);

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
// Tailwind writes its output here before this script runs.
const files = ["index.html", "site/assets/site.css", ...assets.map(([, relative]) => relative)];
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
