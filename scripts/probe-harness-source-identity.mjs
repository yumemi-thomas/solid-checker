#!/usr/bin/env node

// Source identity of the runtime-probe harness image, framed exactly like
// scripts/typefacts-source-identity.mjs.
//
// The harness is interpreted, not compiled, so its source *is* its executable
// image: this manifest is what the Rust probe adapter recomputes from the
// bytes on disk before every launch and compares with the digest compiled into
// the verifier (`SOLID_CHECKER_PROBE_HARNESS_SHA256`). The compiled-in digest
// is the root of trust; the stamp this script writes beside the CLI is a
// build-provenance cross-check, never a root of its own.
//
// The file list is closed and sorted. It covers every script that participates
// in a probe launch — the worker Rust spawns, the harness module the worker
// imports, the audit-path driver and CLI entry that share the protocol — plus
// the CLI manifest and both checked-in lockfiles, because either can determine
// what a `node` or `bun` run of these scripts resolves. Adding a file to the
// harness means adding it here in the same change; the digest moves, and every
// build that did not move with it refuses probe authority.

import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { dirname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
export const IDENTITY_FORMAT = 1;
export const STAMP_RELATIVE_PATH = "packages/cli/probe-harness.buildinfo";

export const HARNESS_FILES = [
  "packages/cli/package.json",
  "packages/cli/bun.lock",
  "packages/cli/package-lock.json",
  "packages/cli/scripts/contract-probe-driver.mjs",
  "packages/cli/scripts/contract-probe-harness.mjs",
  "packages/cli/scripts/contract-probe-worker.mjs",
  "packages/cli/scripts/probe-contract.mjs",
  "scripts/probe-harness-source-identity.mjs",
];

export function sourceDigest(root = ROOT) {
  const selected = HARNESS_FILES.map(path => resolve(root, path));
  selected.sort((left, right) =>
    Buffer.compare(Buffer.from(relative(root, left)), Buffer.from(relative(root, right)))
  );

  const hash = createHash("sha256");
  hash.update(`solid-checker-probe-harness-source\0${IDENTITY_FORMAT}\0`);
  for (const path of selected) {
    const name = relative(root, path).replaceAll("\\", "/");
    const contents = readFileSync(path);
    hash.update(name);
    hash.update("\0");
    hash.update(String(contents.length));
    hash.update("\0");
    hash.update(contents);
    hash.update("\0");
  }
  return hash.digest("hex");
}

export function identity(buildId = process.env.SOLID_CHECKER_BUILD_ID || "dev", root = ROOT) {
  const toolchain =
    process.env.PROBE_HARNESS_TOOLCHAIN_IDENTITY ||
    execFileSync(process.execPath, ["--version"], { encoding: "utf8" }).trim();
  return { format: IDENTITY_FORMAT, sourceDigest: sourceDigest(root), toolchain, buildId };
}

export function writeStamp(buildId, root = ROOT) {
  const stamp = identity(buildId, root);
  writeFileSync(resolve(root, STAMP_RELATIVE_PATH), `${JSON.stringify(stamp)}\n`);
  return stamp;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const buildIdIndex = process.argv.indexOf("--build-id");
  const buildId = buildIdIndex === -1 ? undefined : process.argv[buildIdIndex + 1];
  if (buildIdIndex !== -1 && !buildId) throw new Error("--build-id requires a value");
  if (process.argv.includes("--write-stamp")) {
    const stamp = writeStamp(buildId);
    process.stdout.write(process.argv.includes("--digest") ? stamp.sourceDigest : JSON.stringify(stamp));
  } else {
    const result = identity(buildId);
    process.stdout.write(
      process.argv.includes("--digest") ? result.sourceDigest : JSON.stringify(result)
    );
  }
}
