import { readFile, access } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";

const docs = fileURLToPath(new URL("../", import.meta.url));
const html = await readFile(path.join(docs, "index.html"), "utf8");
const ids = [...html.matchAll(/\bid="([^"]+)"/g)].map((match) => match[1]);
if (new Set(ids).size !== ids.length) throw new Error("Duplicate HTML IDs.");
for (const [, reference] of html.matchAll(/(?:href|src)="([^"]+)"/g)) {
  if (reference.startsWith("#")) {
    if (!ids.includes(reference.slice(1))) throw new Error("Broken fragment: " + reference);
  } else if (reference.startsWith("./") && reference !== "./") {
    const target = path.resolve(docs, reference);
    if (!target.startsWith(docs)) throw new Error("Reference outside docs: " + reference);
    await access(target);
  }
}
for (const [, target] of html.matchAll(/aria-(?:controls|labelledby)="([^"]+)"/g)) {
  for (const id of target.split(" ")) if (!ids.includes(id)) throw new Error("Broken accessible reference: " + id);
}
for (const match of html.matchAll(/<img\b[^>]+>/g)) {
  if (!/\balt="/.test(match[0])) throw new Error("Image needs an alt attribute.");
}
for (const file of ["reading.png", "marks.png", "library.png"]) {
  const png = await readFile(path.join(docs, "site/assets", file));
  if (png.readUInt32BE(16) !== 1080 || png.readUInt32BE(20) !== 780) throw new Error("Update screenshot dimensions: " + file);
}
console.log("Local assets, fragments, accessible references and screenshot dimensions verified.");
