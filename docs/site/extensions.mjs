import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import { copyFile, mkdir, readFile, stat, writeFile } from "node:fs/promises";
import path from "node:path";

const platforms = new Map([
  ["x86_64-pc-windows-msvc", "Windows x64"],
  ["i686-pc-windows-msvc", "Windows x86"],
  ["x86_64-unknown-linux-gnu", "Linux x64"],
  ["aarch64-apple-darwin", "macOS Apple Silicon"],
  ["x86_64-apple-darwin", "macOS Intel"],
]);
const identifier = /^[a-z0-9_-](?:[a-z0-9._-]{0,62}[a-z0-9_-])?$/;
const version = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;
const maxDownload = 512 * 1024 * 1024;
const escape = (text) => text.replace(/[&<>"']/g, (char) => ({
  "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;",
})[char]);

async function digest(filename) {
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(filename)) hash.update(chunk);
  return hash.digest("hex");
}

function card(entry) {
  const { code, name, description, version, assets } = entry;
  const targets = assets.map(({ target }) => platforms.get(target)).join(" · ");
  return `<article class="extension-entry">
    <div>
      <h3 class="text-lg font-semibold tracking-tight">${escape(name)}</h3>
      <p class="mt-2 text-sm leading-6 text-muted">${escape(description)}</p>
      <p class="mt-3 text-xs leading-5 text-muted">Versión ${version} · ${escape(targets)}</p>
    </div>
    <div class="extension-code">
      <code aria-label="Código de extensión ${code}">${code}</code>
      <button type="button" data-extension-code="${code}" class="js-only button button-secondary justify-center py-2">Copiar código</button>
      <p data-extension-status role="status" class="text-xs text-muted"></p>
    </div>
  </article>`;
}

export async function buildExtensions(docs, repo, output) {
  const catalog = JSON.parse(await readFile(path.join(docs, "site/extensions.json"), "utf8"));
  if (catalog.schema !== 1 || !Array.isArray(catalog.extensions) || catalog.extensions.length > 128) {
    throw new Error("Invalid extension catalog schema.");
  }
  const files = ["extensions/catalog-v1.json"];
  const codes = new Set();
  const ids = new Set();
  const preview = process.env.CRABPDF_CATALOG_PREVIEW === "1" && process.env.CI !== "true";
  await mkdir(path.join(output, "extensions"), { recursive: true });
  for (const entry of catalog.extensions) {
    if (!/^[1-9]\d{5}$/.test(entry.code) || codes.has(entry.code) ||
      !identifier.test(entry.id) || ids.has(entry.id) || !version.test(entry.version) ||
      entry.api_version !== 1 || typeof entry.host_version !== "string" ||
      typeof entry.name !== "string" || entry.name.trim().length === 0 || Buffer.byteLength(entry.name) > 120 ||
      typeof entry.description !== "string" || Buffer.byteLength(entry.description) > 2000) {
      throw new Error("Invalid or duplicate extension entry: " + entry.code);
    }
    codes.add(entry.code);
    ids.add(entry.id);
    const done = new Set();
    const assets = [];
    for (const asset of entry.assets ?? []) {
      if (!platforms.has(asset.target) || done.has(asset.target) ||
        typeof asset.file !== "string" || !asset.file.endsWith(".zip") || !identifier.test(asset.file.slice(0, -4)) ||
        !Number.isSafeInteger(asset.bytes) || asset.bytes < 1 || asset.bytes > maxDownload ||
        !/^[a-f0-9]{64}$/.test(asset.sha256) || !identifier.test(asset.release_tag ?? "")) {
        throw new Error("Invalid GitHub release asset for " + entry.code);
      }
      done.add(asset.target);
      assets.push(asset);
    }
    for (const target of entry.example_targets ?? []) {
      if (entry.id !== "org.crabpdf.selection-info" || !platforms.has(target) || done.has(target)) {
        throw new Error("Invalid target for bundled SDK example: " + target);
      }
      done.add(target);
      const file = `selection-info-${target}.zip`;
      const source = path.join(repo, "artifacts/site-extensions", file);
      const bytes = (await stat(source).catch((error) => {
        if (!preview || error.code !== "ENOENT") throw error;
        return { size: 0 };
      })).size;
      if (preview && bytes === 0) continue;
      if (bytes < 1 || bytes > maxDownload) throw new Error("Invalid size: " + source);
      const relative = "extensions/packages/" + file;
      const destination = path.join(output, relative);
      await mkdir(path.dirname(destination), { recursive: true });
      await copyFile(source, destination);
      assets.push({ target, file, bytes, sha256: await digest(destination) });
      files.push(relative);
    }
    if (assets.length === 0 && !preview) throw new Error("Missing assets for " + entry.code);
    entry.assets = assets;
    delete entry.example_targets;
  }
  catalog.extensions = catalog.extensions.filter((entry) => entry.assets.length > 0);
  const json = JSON.stringify(catalog, null, 2) + "\n";
  if (Buffer.byteLength(json) > 256 * 1024) throw new Error("Extension catalog exceeds 256 KiB.");
  await writeFile(path.join(output, files[0]), json);
  return { files, cards: catalog.extensions.map(card).join("\n") || "<p>El catálogo todavía no tiene paquetes.</p>" };
}
