#!/usr/bin/env node

import { createHash } from "node:crypto";
import { lstatSync, readFileSync, readdirSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
export const IDENTITY_FORMAT = 1;

// The Rust producer (apps/solid-typefacts, on tsrs): its own sources, manifest,
// lockfile (which pins every crate, tsrs included, to an exact revision) and
// toolchain pin, the client crate it shares the wire types and codec with, the
// workspace manifest that crate inherits from, and the schemas. Tests,
// testdata and `target/` are not inputs of the binary.
const roots = [
  "apps/solid-typefacts/src",
  "rust/crates/typefacts/src",
];
const files = [
  "apps/solid-typefacts/Cargo.toml",
  "apps/solid-typefacts/Cargo.lock",
  "apps/solid-typefacts/rust-toolchain.toml",
  "rust/crates/typefacts/Cargo.toml",
  "rust/Cargo.toml",
  "schema/typefacts-v1.schema.json",
  "schema/typefacts-codec-limits.json",
  "scripts/build-typefacts.sh",
  "scripts/typefacts-source-identity.mjs",
];

function collect(path, result) {
  const stat = lstatSync(path);
  if (stat.isDirectory()) {
    if (path.endsWith("/target")) return;
    for (const entry of readdirSync(path).sort()) collect(join(path, entry), result);
    return;
  }
  if (stat.isFile()) result.push(path);
}

export function sourceDigest(root = ROOT) {
  const selected = files.map(path => resolve(root, path));
  for (const path of roots) collect(resolve(root, path), selected);
  selected.sort((left, right) => Buffer.compare(
    Buffer.from(relative(root, left)),
    Buffer.from(relative(root, right)),
  ));

  const hash = createHash("sha256");
  hash.update(`solid-checker-typefacts-source\0${IDENTITY_FORMAT}\0`);
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

/** The tsrs revision the producer is pinned to: one rev across every tsrs crate. */
export function tsrsRevision(root = ROOT) {
  const manifest = readFileSync(resolve(root, "apps/solid-typefacts/Cargo.toml"), "utf8");
  const revisions = new Set([...manifest.matchAll(/tsrs_\w+\s*=\s*\{[^}]*\brev\s*=\s*"([0-9a-f]{40})"/g)].map(match => match[1]));
  if (revisions.size !== 1) throw new Error(`apps/solid-typefacts/Cargo.toml must pin every tsrs crate to one rev, found ${[...revisions].join(", ") || "none"}`);
  return [...revisions][0];
}

/** `rustc -vV` of the pinned toolchain; TYPEFACTS_TOOLCHAIN_IDENTITY overrides it (tests). */
function toolchainIdentity(root) {
  if (process.env.TYPEFACTS_TOOLCHAIN_IDENTITY) return process.env.TYPEFACTS_TOOLCHAIN_IDENTITY;
  const pin = /channel\s*=\s*"([^"]+)"/.exec(readFileSync(resolve(root, "apps/solid-typefacts/rust-toolchain.toml"), "utf8"))?.[1];
  return execFileSync("rustc", ["-vV"], {
    encoding: "utf8",
    env: { ...process.env, RUSTUP_TOOLCHAIN: process.env.RUSTUP_TOOLCHAIN || pin },
  }).trim();
}

/**
 * A non-release profile (`TYPEFACTS_PROFILE=bench-dev`, the thin-LTO inner
 * loop) is recorded; the release identity has no `profile` key, so it is the
 * same bytes it always was and the CI cache keys built from it do not move.
 */
export function identity(
  buildId = process.env.TYPEFACTS_BUILD_ID || "dev",
  root = ROOT,
  profile = process.env.TYPEFACTS_PROFILE || "release",
) {
  return {
    format: IDENTITY_FORMAT,
    sourceDigest: sourceDigest(root),
    tsrsRev: tsrsRevision(root),
    toolchain: toolchainIdentity(root),
    buildId,
    ...(profile === "release" ? {} : { profile }),
  };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const buildIdIndex = process.argv.indexOf("--build-id");
  const buildId = buildIdIndex === -1 ? undefined : process.argv[buildIdIndex + 1];
  if (buildIdIndex !== -1 && !buildId) throw new Error("--build-id requires a value");
  const result = identity(buildId);
  process.stdout.write(process.argv.includes("--digest") ? result.sourceDigest : JSON.stringify(result));
}
