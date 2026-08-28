#!/usr/bin/env bun

import { existsSync, readFileSync, readdirSync } from "node:fs";
import { join, relative, resolve } from "node:path";
import { spawnSync } from "node:child_process";
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
      requiredString(manifest.bundleIndex, "bundleIndex", source);
      requiredString(manifest.reviewBundleIndex, "reviewBundleIndex", source);
      if (requireArtifacts && !existsSync(join(projectRoot, manifest.ruleManifest))) {
        fail(`${source} references missing ${manifest.ruleManifest}`);
      }
      for (const field of ["bundleIndex", "reviewBundleIndex"]) {
        if (requireArtifacts && !existsSync(join(projectRoot, manifest[field]))) {
          fail(`${source} references missing ${manifest[field]}`);
        }
      }
      if (!Array.isArray(manifest.contracts) || manifest.contracts.length === 0) {
        fail(`${source} requires at least one contract`);
      }
      for (const contract of manifest.contracts) {
        requiredString(contract.package, "contracts[].package", source);
        const allowed = new Set(["package", "probeRuntime", "probeModes"]);
        for (const field of Object.keys(contract)) {
          if (!allowed.has(field)) {
            fail(`${source} contracts[].${field} is not part of the normalized bundle inventory`);
          }
        }
        if (
          typeof contract.probeRuntime !== "undefined" &&
          typeof contract.probeRuntime !== "boolean"
        ) {
          fail(`${source} contracts[].probeRuntime must be a boolean`);
        }
        // A contract may state its claims for fewer than all four condition
        // modes when a build under some condition is a different artifact.
        if (typeof contract.probeModes !== "undefined") {
          const modes = contract.probeModes;
          const known = ["client", "server", "development", "production"];
          if (!Array.isArray(modes) || modes.length === 0) {
            fail(`${source} contracts[].probeModes must be a non-empty array`);
          } else if (modes.some(mode => !known.includes(mode))) {
            fail(`${source} contracts[].probeModes must be drawn from ${known.join(", ")}`);
          } else if (!contract.probeRuntime) {
            fail(`${source} contracts[].probeModes has no meaning without probeRuntime`);
          }
        }
      }
      return { ...manifest, source: relative(projectRoot, source) };
    })
    .sort((left, right) => left.id.localeCompare(right.id));

  const ids = new Set();
  const packages = new Set();
  const ruleManifests = new Set();
  for (const manifest of manifests) {
    if (ids.has(manifest.id)) fail(`duplicate id ${manifest.id}`);
    ids.add(manifest.id);
    if (ruleManifests.has(manifest.ruleManifest)) {
      fail(`duplicate ruleManifest ${manifest.ruleManifest}`);
    }
    ruleManifests.add(manifest.ruleManifest);
    for (const contract of manifest.contracts) {
      if (packages.has(`${manifest.id}/${contract.package}`)) {
        fail(`${manifest.id} declares ${contract.package} twice`);
      }
      packages.add(`${manifest.id}/${contract.package}`);
    }
  }
  return manifests;
}

function run(command, args) {
  const result = spawnSync(command, args, { cwd: root, stdio: "inherit", env: process.env });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}

function generateContracts(check) {
  loadDialectManifests({ requireArtifacts: check });
  const args = [
    "+1.97",
    "run",
    "--manifest-path",
    "rust/Cargo.toml",
    "-p",
    "solid-facts-backend",
    "--bin",
    "solid-contract-bundles",
    "--",
    "--root",
    root,
  ];
  if (check) args.push("--check");
  run("cargo", args);
}

function checkComposedContracts() {
  generateContracts(true);
}

const invokedDirectly =
  process.argv[1] && fileURLToPath(import.meta.url) === resolve(process.argv[1]);
if (invokedDirectly) {
  const command = process.argv[2] ?? "validate";
  if (command === "generate-contracts") generateContracts(false);
  else if (command === "check-contracts") generateContracts(true);
  else if (command === "check-composed-contracts") checkComposedContracts();
  else if (command === "validate") {
    const manifests = loadDialectManifests({ requireArtifacts: true });
    console.log(`validated ${manifests.length} dialect assembly manifests`);
  } else {
    fail(`unknown command ${command}`);
  }
}
