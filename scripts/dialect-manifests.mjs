#!/usr/bin/env node

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
      if (manifest.schemaVersion !== 1) fail(`${source} has unsupported schemaVersion`);
      requiredString(manifest.id, "id", source);
      if (manifest.id !== entry.name) {
        fail(`${source} id ${manifest.id} must match directory ${entry.name}`);
      }
      requiredString(manifest.ruleManifest, "ruleManifest", source);
      if (requireArtifacts && !existsSync(join(projectRoot, manifest.ruleManifest))) {
        fail(`${source} references missing ${manifest.ruleManifest}`);
      }
      if (!Array.isArray(manifest.contracts) || manifest.contracts.length === 0) {
        fail(`${source} requires at least one contract`);
      }
      for (const contract of manifest.contracts) {
        // A contract is generated from an installed package unless it says
        // otherwise. `generated: false` declares a hand-authored bundled
        // overlay — reviewed against the package rather than derived from it,
        // so it has no generator target, no review contract, and no export
        // index. It is still declared, because the manifest is the inventory
        // of every package a dialect models and a package missing from it is
        // covered by no gate at all.
        const generated = contract.generated !== false;
        if (typeof contract.generated !== "undefined" && typeof contract.generated !== "boolean") {
          fail(`${source} contracts[].generated must be a boolean`);
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
        if (typeof contract.probeDefaultExport !== "undefined") {
          if (typeof contract.probeDefaultExport !== "boolean") {
            fail(`${source} contracts[].probeDefaultExport must be a boolean`);
          } else if (!contract.probeRuntime) {
            fail(`${source} contracts[].probeDefaultExport has no meaning without probeRuntime`);
          }
        }
        const generatorFields = [
          "packagePathEnv",
          "defaultPackagePath",
          "generatorTarget",
          "reviewContract",
          "exportsIndex",
        ];
        for (const field of generated
          ? ["package", ...generatorFields, "bundledContract"]
          : ["package", "bundledContract"]) {
          requiredString(contract[field], `contracts[].${field}`, source);
        }
        if (!generated) {
          // Refused rather than ignored: a half-filled entry means someone
          // meant to declare a generated contract and left fields out, which
          // must not pass as a deliberate hand-authored one.
          for (const field of generatorFields) {
            if (typeof contract[field] !== "undefined") {
              fail(`${source} contracts[].${field} is not allowed when generated is false`);
            }
          }
        }
        if (generated && !contract.generatorTarget.startsWith(`${manifest.id}/`)) {
          fail(`${source} generatorTarget ${contract.generatorTarget} must start with ${manifest.id}/`);
        }
        const artifacts = generated
          ? [contract.reviewContract, contract.exportsIndex, contract.bundledContract]
          : [contract.bundledContract];
        for (const path of artifacts) {
          if (requireArtifacts && !existsSync(join(projectRoot, path))) {
            fail(`${source} references missing ${path}`);
          }
        }
        if (contract.composeScript && !existsSync(join(projectRoot, contract.composeScript))) {
          fail(`${source} references missing ${contract.composeScript}`);
        }
        for (const input of contract.composeInputs ?? []) {
          requiredString(input, "contracts[].composeInputs[]", source);
          if (requireArtifacts && !existsSync(join(projectRoot, input))) {
            fail(`${source} references missing ${input}`);
          }
        }
      }
      return { ...manifest, source: relative(projectRoot, source) };
    })
    .sort((left, right) => left.id.localeCompare(right.id));

  const ids = new Set();
  const targets = new Set();
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
      if (contract.generated === false) continue;
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
      // Hand-authored overlays have nothing to regenerate from; their artifact
      // is reviewed, not derived. `make contract-conformance` is what checks
      // them.
      if (contract.generated === false) continue;
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
