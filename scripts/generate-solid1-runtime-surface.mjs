#!/usr/bin/env node
// Generates pkg/contracts/bundled/solid-v1/solid-js-runtime-surface.json: which
// exports solid-js@1.9.14 actually has, under which entrypoints, read from the
// installed package rather than from a declaration inventory.
//
// This replaced a census copied from the 1.x branch, which decided the same
// question and was wrong about it in both directions: it listed 20 names no
// build exports (`readSignal`, `registerGraph` and friends are properties of
// the DEV object, never exports) and omitted two that every build does
// (`innerHTML`, `ssrStyleProperty` on ./web). A declaration inventory answers
// "what does the package declare"; a contract states what the package *does*,
// so its export set has to come from the artifact it claims to describe.
//
// The surface is the union across the four condition modes the conformance
// suite probes, because 1.x resolves a different build per condition and the
// contract covers all of them.
//
//   node scripts/generate-solid1-runtime-surface.mjs [--check]
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { loadDialectManifests, root } from "./dialect-manifests.mjs";

const check = process.argv.includes("--check");
const PACKAGE = "solid-js";
const SURFACE = "pkg/contracts/bundled/solid-v1/solid-js-runtime-surface.json";
const conditionModes = [["browser"], ["node"], ["browser", "development"], ["browser", "production"]];

function fail(message) {
  console.error(`generate-solid1-runtime-surface: ${message}`);
  process.exit(1);
}

const dialect = loadDialectManifests().find(manifest => manifest.id === "solid-v1");
const contract = dialect?.contracts.find(item => item.package === PACKAGE);
if (!contract) fail("solid-v1 declares no solid-js contract");

// The version comes from the contract this surface feeds, so the two cannot
// describe different releases.
const version = JSON.parse(readFileSync(join(root, contract.bundledContract), "utf8")).package
  ?.version;
if (!version) fail(`${contract.bundledContract} records no package version`);

const install = join(tmpdir(), `solid-checker-runtime-surface-${PACKAGE}@${version}`);
mkdirSync(install, { recursive: true });
const directory = join(install, "node_modules", PACKAGE);
const installedVersion = () =>
  existsSync(join(directory, "package.json"))
    ? JSON.parse(readFileSync(join(directory, "package.json"), "utf8")).version
    : null;
if (installedVersion() !== version) {
  const result = spawnSync(
    "npm",
    ["install", "--prefix", install, "--no-audit", "--no-fund", "--no-save", `${PACKAGE}@${version}`],
    { stdio: "inherit" },
  );
  if (result.status !== 0) process.exit(result.status ?? 1);
}

const printer = join(root, "scripts/lib/print-package-surface.mjs");
const entrypoints = new Map();
for (const conditions of conditionModes) {
  const execution = spawnSync(
    "node",
    [...conditions.flatMap(condition => ["--conditions", condition]), printer, PACKAGE, directory],
    { encoding: "utf8" },
  );
  if (execution.status !== 0) {
    process.stderr.write(execution.stderr);
    fail(`reading the surface under ${conditions.join("+")} failed`);
  }
  for (const [entrypoint, surface] of Object.entries(JSON.parse(execution.stdout).entrypoints)) {
    const merged = entrypoints.get(entrypoint) ?? { conditions: new Set(), exports: new Map() };
    for (const condition of surface.conditions) merged.conditions.add(condition);
    for (const [name, kind] of Object.entries(surface.exports)) {
      const seen = merged.exports.get(name);
      if (seen && seen !== kind) {
        fail(`${entrypoint}:${name} is ${seen} under one condition and ${kind} under another`);
      }
      merged.exports.set(name, kind);
    }
    entrypoints.set(entrypoint, merged);
  }
}

const document = {
  package: { name: PACKAGE, version },
  entrypoints: Object.fromEntries(
    [...entrypoints.entries()]
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([entrypoint, surface]) => [
        entrypoint,
        {
          conditions: [...surface.conditions].sort(),
          exports: Object.fromEntries(
            [...surface.exports.entries()].sort(([left], [right]) => left.localeCompare(right)),
          ),
        },
      ]),
  ),
};

const path = join(root, SURFACE);
const rendered = `${JSON.stringify(document, null, 2)}\n`;
if (check) {
  const current = existsSync(path) ? readFileSync(path, "utf8") : "";
  if (current !== rendered) {
    fail(`${SURFACE} is stale -- rerun without --check`);
  }
  const counts = Object.entries(document.entrypoints)
    .map(([entrypoint, surface]) => `${entrypoint}: ${Object.keys(surface.exports).length}`)
    .join(", ");
  console.log(`ok   ${SURFACE} matches ${PACKAGE}@${version} (${counts})`);
} else {
  writeFileSync(path, rendered);
  console.log(`wrote ${SURFACE} for ${PACKAGE}@${version}`);
  for (const [entrypoint, surface] of Object.entries(document.entrypoints)) {
    console.log(`  ${entrypoint}: ${Object.keys(surface.exports).length} exports`);
  }
}
