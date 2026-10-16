#!/usr/bin/env bun

import { existsSync, readFileSync, readdirSync } from "node:fs";
import { join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const root = fileURLToPath(new URL("..", import.meta.url));

function fail(message) {
  throw new Error(`dialect manifest: ${message}`);
}

function requiredString(value, field, source) {
  if (typeof value !== "string" || value.length === 0) {
    fail(`${source} requires non-empty ${field}`);
  }
}

/** Loads and validates every checked-in dialect assembly manifest. */
export function loadDialectManifests({ requireArtifacts = false, projectRoot = root } = {}) {
  const dialectsRoot = join(projectRoot, "rust", "dialects");
  const manifests = readdirSync(dialectsRoot, { withFileTypes: true })
    .filter(entry => entry.isDirectory() && entry.name.startsWith("solid-v"))
    .map(entry => {
      const source = join(dialectsRoot, entry.name, "dialect.json");
      if (!existsSync(source)) fail(`${entry.name} has no dialect.json`);
      const manifest = JSON.parse(readFileSync(source, "utf8"));
      if (manifest.schemaVersion !== 2) fail(`${source} has unsupported schemaVersion`);
      requiredString(manifest.id, "id", source);
      if (manifest.id !== entry.name) {
        fail(`${source} id ${manifest.id} must match directory ${entry.name}`);
      }
      requiredString(manifest.ruleManifest, "ruleManifest", source);
      if (requireArtifacts && !existsSync(join(projectRoot, manifest.ruleManifest))) {
        fail(`${source} references missing ${manifest.ruleManifest}`);
      }
      for (const field of ["bundleIndex", "reviewBundleIndex", "contracts"]) {
        if (field in manifest) {
          fail(`${source} ${field} was removed with package contracts`);
        }
      }
      // The compiler each dialect is assembled against is per-dialect data,
      // and `check-compiler-facts-identity.mjs` used to carry solid-v2's
      // copy of it as literals. Declaring it here is what makes that gate
      // enumerate dialects rather than check one: a dialect that omits the
      // block fails validation instead of going silently unchecked.
      const compiler = manifest.compilerIdentity;
      if (typeof compiler !== "object" || compiler === null) {
        fail(`${source} requires compilerIdentity`);
      }
      const compilerFields = [
        "document",
        "adapter",
        "conformance",
        "report",
        "cargoPackage",
        "cargoSourcePrefix"
      ];
      for (const field of compilerFields) {
        requiredString(compiler[field], `compilerIdentity.${field}`, source);
      }
      for (const field of Object.keys(compiler)) {
        if (!compilerFields.includes(field)) {
          fail(`${source} compilerIdentity.${field} is not part of the compiler wiring`);
        }
      }
      if (requireArtifacts) {
        for (const field of ["document", "adapter", "conformance", "report"]) {
          if (!existsSync(join(projectRoot, compiler[field]))) {
            fail(`${source} references missing ${compiler[field]}`);
          }
        }
      }
      return { ...manifest, source: relative(projectRoot, source) };
    })
    .sort((left, right) => left.id.localeCompare(right.id));

  const ids = new Set();
  const ruleManifests = new Set();
  for (const manifest of manifests) {
    if (ids.has(manifest.id)) fail(`duplicate id ${manifest.id}`);
    ids.add(manifest.id);
    if (ruleManifests.has(manifest.ruleManifest)) {
      fail(`duplicate ruleManifest ${manifest.ruleManifest}`);
    }
    ruleManifests.add(manifest.ruleManifest);
  }
  return manifests;
}

const invokedDirectly =
  process.argv[1] && fileURLToPath(import.meta.url) === resolve(process.argv[1]);
if (invokedDirectly) {
  const command = process.argv[2] ?? "validate";
  if (command === "validate") {
    const manifests = loadDialectManifests({ requireArtifacts: true });
    console.log(`validated ${manifests.length} dialect assembly manifests`);
  } else {
    fail(`unknown command ${command}`);
  }
}
