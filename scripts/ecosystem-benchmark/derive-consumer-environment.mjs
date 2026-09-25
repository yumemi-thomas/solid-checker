#!/usr/bin/env bun
// Derives one consumer-environment entry from a consumer's installed pnpm
// lockfile, for review into scripts/ecosystem-benchmark/consumer-environments.json.
//
//   bun scripts/ecosystem-benchmark/derive-consumer-environment.mjs \
//     --lock <pnpm-lock.yaml> --id <id> --repository <url> --branch <name> \
//     --commit <sha> --importer packages/core [--importer packages/utils]
//
// Prints the entry and the problems `run.mjs --consumer-environment` would
// refuse it for. It writes nothing: an environment is reviewed data, and the
// reviewer pastes it in. Needs Bun (for `Bun.YAML`) and no network.

import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { deriveConsumerEnvironment, environmentProblems } from "./lib/consumer-environments.mjs";
import { loadAuditedArchives } from "./lib/dialect-authority.mjs";

const HERE = dirname(fileURLToPath(import.meta.url));

function fail(message) {
  console.error(`derive-consumer-environment: ${message}`);
  process.exit(2);
}

const options = { importers: [] };
const argv = process.argv.slice(2);
for (let index = 0; index < argv.length; index += 1) {
  const argument = argv[index];
  const value = argv[index + 1];
  if (value === undefined) fail(`${argument} requires a value`);
  index += 1;
  if (argument === "--lock") options.lock = value;
  else if (argument === "--id") options.id = value;
  else if (argument === "--repository") options.repository = value;
  else if (argument === "--branch") options.branch = value;
  else if (argument === "--commit") options.commit = value;
  else if (argument === "--importer") options.importers.push(value);
  else fail(`unknown argument ${argument}`);
}
for (const required of ["lock", "id", "repository", "branch", "commit"]) {
  if (!options[required]) fail(`--${required} is required`);
}
if (options.importers.length === 0) fail("at least one --importer is required");
if (typeof Bun === "undefined" || !Bun.YAML) fail("needs Bun, for Bun.YAML");

const lockText = readFileSync(resolve(options.lock), "utf8");
const manifest = JSON.parse(readFileSync(join(HERE, "manifest.json"), "utf8"));
const entry = deriveConsumerEnvironment({
  lock: Bun.YAML.parse(lockText),
  id: options.id,
  source: {
    repository: options.repository,
    branch: options.branch,
    commit: options.commit,
    lockfile: "pnpm-lock.yaml",
    lockfileDigest: `sha256:${createHash("sha256").update(lockText).digest("hex")}`
  },
  importers: options.importers,
  manifest
});
console.log(JSON.stringify(entry, null, 2));
const problems = environmentProblems(entry, { manifest, auditedArchives: loadAuditedArchives() });
for (const problem of problems) console.error(`refused: ${problem}`);
process.exit(problems.length ? 1 : 0);
