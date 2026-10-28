import { readFile, access } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";

const output = fileURLToPath(new URL("../../dist/site/", import.meta.url));
const html = await readFile(path.join(output, "index.html"), "utf8");
if (html.includes("{{") || html.includes("}}")) throw new Error("Unresolved website placeholders.");
const ids = [...html.matchAll(/\bid="([^"]+)"/g)].map((match) => match[1]);
if (new Set(ids).size !== ids.length) throw new Error("Duplicate HTML IDs.");
for (const [, reference] of html.matchAll(/(?:href|src)="([^"]+)"/g)) {
  if (reference.startsWith("#")) {
    if (!ids.includes(reference.slice(1))) throw new Error("Broken fragment: " + reference);
  } else if (reference.startsWith("./") && reference !== "./") {
    const target = path.resolve(output, reference);
    if (!target.startsWith(output)) throw new Error("Reference outside the public site: " + reference);
    await access(target);
  }
}
for (const [, target] of html.matchAll(/aria-(?:controls|labelledby)="([^"]+)"/g)) {
  for (const id of target.split(" ")) if (!ids.includes(id)) throw new Error("Broken accessible reference: " + id);
}
for (const [tag] of html.matchAll(/<img\b[^>]+>/g)) {
  if (!/\balt="/.test(tag)) throw new Error("Image needs an alt attribute.");
  const source = tag.match(/\bsrc="([^"]+)"/)?.[1];
  if (source?.endsWith(".png")) {
    const png = await readFile(path.resolve(output, source));
    const width = Number(tag.match(/\bwidth="(\d+)"/)?.[1]);
    const height = Number(tag.match(/\bheight="(\d+)"/)?.[1]);
    if (png.readUInt32BE(16) !== width || png.readUInt32BE(20) !== height) throw new Error("Incorrect image dimensions: " + source);
  }
}
console.log("Built website: local assets, fragments, accessible references and image dimensions verified.");
