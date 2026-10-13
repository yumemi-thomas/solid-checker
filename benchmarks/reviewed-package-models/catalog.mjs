import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, readdirSync, realpathSync } from "node:fs";
import { dirname, join, resolve } from "node:path";

export const read = path => JSON.parse(readFileSync(path, "utf8"));
export const hash = value => `sha256:${createHash("sha256").update(value).digest("hex")}`;
export function packageRoot(from, name) {
  for (let dir = resolve(from); ; dir = dirname(dir)) {
    const candidate = join(dir, "node_modules", name);
    if (existsSync(join(candidate, "package.json"))) return realpathSync(candidate);
    if (dirname(dir) === dir) throw new Error(`Installed package missing: ${name}`);
  }
}
export function packageDigest(root) {
  const files = [];
  function walk(dir, prefix = "") {
    for (const entry of readdirSync(dir, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
      if (entry.name === "node_modules" || entry.name === ".git") continue;
      const path = join(dir, entry.name), relative = `${prefix}${entry.name}`;
      if (entry.isDirectory()) walk(path, `${relative}/`);
      else {
        assert(entry.isFile(), `Unsupported package input: ${path}`);
        files.push([relative, hash(readFileSync(path))]);
      }
    }
  }
  walk(root);
  return hash(JSON.stringify(files));
}
export function closurePins(root) {
  const pins = new Map();
  function visit(dir) {
    dir = realpathSync(dir);
    if (pins.has(dir)) return;
    const manifest = read(join(dir, "package.json"));
    pins.set(dir, { package: manifest.name, version: manifest.version, digest: packageDigest(dir) });
    const dependencies = { ...manifest.dependencies, ...manifest.peerDependencies };
    for (const name of Object.keys(dependencies).sort()) {
      try { visit(packageRoot(dir, name)); }
      catch (error) {
        if (manifest.peerDependenciesMeta?.[name]?.optional || manifest.optionalDependencies?.[name]) continue;
        throw error;
      }
    }
  }
  visit(root);
  return [...pins.values()].sort((a, b) => JSON.stringify(a).localeCompare(JSON.stringify(b)));
}
export function nativeRuntimeRoots(project) {
  const solid = packageRoot(project, "solid-js");
  // pnpm need not expose transitive runtime packages at the consumer root.
  // Follow the dependency graph from the actually resolved Solid entry.
  return [solid, packageRoot(solid, "@solidjs/signals"), packageRoot(solid, "@solidjs/web")];
}
export function authenticateModel(model, project) {
  const root = packageRoot(project, model.package);
  const manifest = read(join(root, "package.json"));
  assert.equal(manifest.version, model.version, `Model version mismatch: ${model.package}`);
  assert.deepEqual(closurePins(root), model.pins, `Model input bytes changed: ${model.package}`);
  for (const runtime of nativeRuntimeRoots(project)) {
    const installed = read(join(runtime, "package.json"));
    assert.equal(installed.version, "2.0.0-rc.9", `Runtime mismatch: ${installed.name}`);
  }
  return root;
}
