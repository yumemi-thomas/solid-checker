#!/usr/bin/env node

import { existsSync, readFileSync, readdirSync } from "node:fs";
import { join, relative, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

export const root = fileURLToPath(new URL("..", import.meta.url));
const dialectsRoot = join(root, "rust", "dialects");

function fail(message) {
  throw new Error(`dialect manifest: ${message}`);
}

function requiredString(value, field, source) {
  if (typeof value !== "string" || value.length === 0) {
    fail(`${source} requires non-empty ${field}`);
  }
}

/** Loads and validates every checked-in dialect assembly manifest. */
export function loadDialectManifests({ requireArtifacts = false } = {}) {
  const manifests = readdirSync(dialectsRoot, { withFileTypes: true })
    .filter(entry => entry.isDirectory() && entry.name.startsWith("solid-v"))
    .map(entry => {
      const source = join(dialectsRoot, entry.name, "dialect.json");
      if (!existsSync(source)) fail(`${entry.name} has no dialect.json`);
      const manifest = JSON.parse(readFileSync(source, "utf8"));
      if (manifest.schemaVersion !== 1) fail(`${source} has unsupported schemaVersion`);
      requiredString(manifest.id, "id", source);
      if (manifest.id !== entry.name) {
        fail(`${source} id ${manifest.id} must match directory ${entry.name}`);
      }
      requiredString(manifest.ruleManifest, "ruleManifest", source);
      if (requireArtifacts && !existsSync(join(root, manifest.ruleManifest))) {
        fail(`${source} references missing ${manifest.ruleManifest}`);
      }
      if (!Array.isArray(manifest.contracts) || manifest.contracts.length === 0) {
        fail(`${source} requires at least one contract`);
      }
      for (const contract of manifest.contracts) {
        for (const field of [
          "package",
          "packagePathEnv",
          "defaultPackagePath",
          "generatorTarget",
          "reviewContract",
          "exportsIndex",
          "bundledContract",
        ]) {
          requiredString(contract[field], `contracts[].${field}`, source);
        }
        if (!contract.generatorTarget.startsWith(`${manifest.id}/`)) {
          fail(`${source} generatorTarget ${contract.generatorTarget} must start with ${manifest.id}/`);
        }
        for (const path of [contract.reviewContract, contract.exportsIndex, contract.bundledContract]) {
          if (requireArtifacts && !existsSync(join(root, path))) {
            fail(`${source} references missing ${path}`);
          }
        }
        if (contract.composeScript && !existsSync(join(root, contract.composeScript))) {
          fail(`${source} references missing ${contract.composeScript}`);
        }
        for (const input of contract.composeInputs ?? []) {
          requiredString(input, "contracts[].composeInputs[]", source);
          if (requireArtifacts && !existsSync(join(root, input))) {
            fail(`${source} references missing ${input}`);
          }
        }
      }
      return { ...manifest, source: relative(root, source) };
    })
    .sort((left, right) => left.id.localeCompare(right.id));

  const ids = new Set();
  const targets = new Set();
  const ruleManifests = new Set();
  for (const manifest of manifests) {
    if (ids.has(manifest.id)) fail(`duplicate id ${manifest.id}`);
    ids.add(manifest.id);
    if (ruleManifests.has(manifest.ruleManifest)) {
      fail(`duplicate ruleManifest ${manifest.ruleManifest}`);
    }
    ruleManifests.add(manifest.ruleManifest);
    for (const contract of manifest.contracts) {
      if (targets.has(contract.generatorTarget)) {
        fail(`duplicate generatorTarget ${contract.generatorTarget}`);
      }
      targets.add(contract.generatorTarget);
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
  for (const manifest of loadDialectManifests()) {
    for (const contract of manifest.contracts) {
      const packagePath = process.env[contract.packagePathEnv] ?? contract.defaultPackagePath;
      const args = [
        "+1.97",
        "run",
        "--manifest-path",
        "rust/Cargo.toml",
        "-p",
        "solid-facts-backend",
        "--bin",
        "solid-contract-gen",
        "--",
        "--package",
        packagePath,
        "--dialect",
        contract.generatorTarget,
        "--out",
        contract.reviewContract,
        "--index-out",
        contract.exportsIndex,
      ];
      if (check) args.push("--check");
      run("cargo", args);
    }
  }
}

function checkComposedContracts() {
  for (const manifest of loadDialectManifests({ requireArtifacts: true })) {
    for (const contract of manifest.contracts) {
      if (contract.composeScript) run(process.execPath, [contract.composeScript, "--check"]);
    }
  }
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
