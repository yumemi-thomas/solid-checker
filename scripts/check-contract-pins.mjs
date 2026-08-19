#!/usr/bin/env node
// Verifies that every bundled contract names a package release that exists in
// the registry, with the exact tarball the contract was audited against.
//
// `scripts/check-bundled-contracts.mjs` already proves this for contracts it
// probes: it installs them and reads npm's hidden lockfile. That leaves the
// contracts it does not probe -- a hand-authored overlay, or a dialect whose
// runtime is not probed at all -- pinned by a version string nothing checks.
// A version string alone is not a pin: republished or mutated contents keep
// the same version, and the contract would still claim to describe them.
//
// So an absent integrity is a failure here, not a skip. A pin that cannot be
// falsified is not a pin.
import { readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { loadDialectManifests, root } from "./dialect-manifests.mjs";

let failures = 0;
const fail = message => {
  failures++;
  console.error(`FAIL ${message}`);
};

/** The registry's integrity for one exact release, or an explained failure. */
function registryIntegrity(name, version) {
  const result = spawnSync("npm", ["view", `${name}@${version}`, "dist.integrity", "--json"], {
    encoding: "utf8",
  });
  if (result.error) {
    return { error: `cannot run npm view: ${result.error.message}` };
  }
  if (result.status !== 0) {
    const detail = result.stderr.trim().split("\n").at(-1) ?? `exit ${result.status}`;
    return { error: `registry lookup failed: ${detail}` };
  }
  const output = result.stdout.trim();
  if (output === "" || output === "undefined") {
    return { error: "the registry reports no release at that exact version" };
  }
  let parsed;
  try {
    parsed = JSON.parse(output);
  } catch (error) {
    return { error: `unreadable npm view output: ${error.message}` };
  }
  // A range or tag would yield an array; an exact version must not.
  if (typeof parsed !== "string") {
    return { error: `expected one integrity, got ${JSON.stringify(parsed)}` };
  }
  return { integrity: parsed };
}

/**
 * Checks one contract's pin, returning `undefined` when it holds and the
 * failure sentence when it does not. `lookup` is the registry query, injected
 * so the rules can be tested without a network.
 */
export function verifyPin({ label, file, expectedName, document }, lookup = registryIntegrity) {
  const pin = document?.package ?? {};
  if (pin.name !== expectedName) {
    return `${label}: ${file} describes ${JSON.stringify(pin.name)}, not ${expectedName}`;
  }
  if (typeof pin.version !== "string" || pin.version === "") {
    return `${label}: ${file} records no package version`;
  }
  if (typeof pin.integrity !== "string" || pin.integrity === "") {
    return (
      `${label}: ${file} pins ${pin.name}@${pin.version} by version alone. Record the release's ` +
      `integrity (npm view ${pin.name}@${pin.version} dist.integrity) so the pin can be falsified.`
    );
  }
  const observed = lookup(pin.name, pin.version);
  if (observed.error) return `${label}: ${pin.name}@${pin.version} ${observed.error}`;
  if (observed.integrity !== pin.integrity) {
    return (
      `${label}: ${pin.name}@${pin.version} is ${observed.integrity} in the registry, but the ` +
      `contract was audited against ${pin.integrity}. The artifact this contract describes is not ` +
      `the one the registry now serves.`
    );
  }
  return undefined;
}

function main() {
  const contracts = loadDialectManifests({ requireArtifacts: true }).flatMap(manifest =>
    manifest.contracts.map(contract => ({ dialect: manifest.id, ...contract })),
  );
  for (const contract of contracts) {
    const label = `${contract.dialect}/${contract.package}`;
    const failure = verifyPin({
      label,
      file: contract.bundledContract,
      expectedName: contract.package,
      document: JSON.parse(readFileSync(join(root, contract.bundledContract), "utf8")),
    });
    if (failure) {
      fail(failure);
      continue;
    }
    console.log(`ok   ${label}: matches its audited tarball`);
  }
  if (failures > 0) {
    console.error(`${failures} bundled contract pin(s) could not be verified`);
    process.exit(1);
  }
  console.log(`verified ${contracts.length} bundled contract pins against the registry`);
}

if (fileURLToPath(import.meta.url) === resolve(process.argv[1] ?? "")) main();
