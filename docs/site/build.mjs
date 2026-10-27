import { mkdir, copyFile, readFile, readdir } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const docs = fileURLToPath(new URL("../", import.meta.url));
const repo = path.resolve(docs, "..");
const output = path.join(repo, "dist", "site");
const files = [
  "index.html", "site/app.js", "site/assets/site.css",
  "site/assets/crabpdf-logo.svg", "site/assets/reading.png",
  "site/assets/marks.png", "site/assets/library.png",
];
// Copy only the public website. Architecture records, npm dependencies and
// build scripts in docs are not part of the Pages artifact.
for (const file of files) {
  const destination = path.join(output, file);
  await mkdir(path.dirname(destination), { recursive: true });
  await copyFile(path.join(docs, file), destination);
}
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
const actual = (await walk(output)).sort();
if (JSON.stringify(actual) !== JSON.stringify([...files].sort())) {
  throw new Error("Unexpected files in dist/site. Review the build directory before publishing.");
}
const bytes = (await Promise.all(files.map(async (file) => (await readFile(path.join(output, file))).length)))
  .reduce((total, size) => total + size, 0);
console.log("Website built in dist/site: " + files.length + " public files, " + bytes + " bytes.");
