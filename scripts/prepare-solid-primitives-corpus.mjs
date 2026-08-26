import fs from "node:fs";
import path from "node:path";

const root = process.argv[2];
if (!root) throw new Error("usage: bun prepare-solid-primitives-corpus.mjs <solid-primitives-root>");

const packages = path.join(root, "packages");
for (const entry of fs.readdirSync(packages, { withFileTypes: true })) {
  if (!entry.isDirectory()) continue;
  const manifestPath = path.join(packages, entry.name, "package.json");
  if (!fs.existsSync(manifestPath)) continue;

  const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
  manifest.files ??= [];
  if (!manifest.files.includes("solid-reactivity.json")) {
    manifest.files.push("solid-reactivity.json");
    fs.writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
  }
}
