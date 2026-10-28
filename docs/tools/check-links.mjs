// Dependency-free check for portable, local documentation references.
import { execFileSync } from "node:child_process";
import { readFile, stat } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../", import.meta.url));
const names = [...new Set(execFileSync("git", ["ls-files", "--cached", "--others", "--exclude-standard", "-z"], { cwd: root, encoding: "utf8" }).split("\0").filter(Boolean))];
const files = names.filter((name) => name.endsWith(".md") && path.basename(name) !== "AGENTS.md");
const failures = [];
let checked = 0;
let scanned = 0;
for (const name of files) {
  let markdown;
  try { markdown = await readFile(path.join(root, name), "utf8"); }
  catch (error) { if (error.code === "ENOENT") continue; throw error; } // Renames in a dirty worktree.
  scanned += 1;
  markdown = markdown.replace(/```[\s\S]*?```/g, "").replace(/`[^`\n]*`/g, "");
  for (const match of markdown.matchAll(/!?\[[^\]]*\]\(([^\s)]+)(?:\s+"[^"]*")?\)/g)) {
    const href = match[1].replace(/^<|>$/g, "");
    if (/^[a-zA-Z][a-zA-Z0-9+.-]*:/.test(href) || href.startsWith("//") || href.startsWith("#")) continue;
    const local = decodeURIComponent(href.split(/[?#]/)[0]);
    if (!local) continue;
    const target = path.resolve(path.dirname(path.join(root, name)), local);
    const relative = path.relative(root, target).replaceAll(path.sep, "/");
    if (relative.startsWith("../") || path.isAbsolute(relative)) {
      failures.push(name + ": reference outside the repository: " + href); continue;
    }
    try {
      const info = await stat(target);
      if (info.isFile() && !names.includes(relative)) throw new Error("target is not versioned");
      if (info.isDirectory() && !names.some((entry) => entry.startsWith(relative.replace(/\/$/, "") + "/"))) throw new Error("directory has no versioned content");
      checked += 1;
    } catch (error) { failures.push(name + ": " + href + " (" + (error.code || error.message) + ")"); }
  }
}
if (failures.length) throw new Error("Broken or nonportable documentation references:\n" + failures.join("\n"));
console.log("Documentation: " + checked + " portable local references checked across " + scanned + " Markdown files.");
