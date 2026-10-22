#!/usr/bin/env node

import { createHash } from "node:crypto";
import { lstatSync, readFileSync, readdirSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
export const IDENTITY_FORMAT = 1;

const roots = [
  "apps/solid-typefacts",
  "rust/crates/typefacts",
  "shims",
];
const files = [
  "go.mod",
  "go.sum",
  "schema/typefacts-v1.schema.json",
  "schema/typefacts-codec-limits.json",
  "scripts/build-typefacts.sh",
  "scripts/typefacts-source-identity.mjs",
];

// The Rust producer (apps/solid-typefacts-rs, migration slice S6): its own
// sources, lockfile and toolchain pin, the client crate it shares the wire
// types and codec with, and the schemas. `target/` is build output.
const rustRoots = [
  "apps/solid-typefacts-rs/src",
  "apps/solid-typefacts-rs/tests",
  "rust/crates/typefacts",
];
const rustFiles = [
  "apps/solid-typefacts-rs/Cargo.toml",
  "apps/solid-typefacts-rs/Cargo.lock",
  "apps/solid-typefacts-rs/rust-toolchain.toml",
  "schema/typefacts-v1.schema.json",
  "schema/typefacts-codec-limits.json",
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

export function sourceDigest(root = ROOT, producer = "go") {
  const rust = producer === "rust";
  const selected = (rust ? rustFiles : files).map(path => resolve(root, path));
  for (const path of rust ? rustRoots : roots) collect(resolve(root, path), selected);
  selected.sort((left, right) => Buffer.compare(
    Buffer.from(relative(root, left)),
    Buffer.from(relative(root, right)),
  ));

  const hash = createHash("sha256");
  hash.update(`${rust ? "solid-checker-typefacts-rs-source" : "solid-checker-typefacts-source"}\0${IDENTITY_FORMAT}\0`);
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

/** The Rust producer's identity: `rustc -vV` stands where `go version` does. */
export function rustIdentity(buildId = process.env.TYPEFACTS_BUILD_ID || "dev", root = ROOT) {
  const toolchain = process.env.TYPEFACTS_TOOLCHAIN_IDENTITY ||
    execFileSync("rustc", ["-vV"], {
      encoding: "utf8",
      env: { ...process.env, RUSTUP_TOOLCHAIN: process.env.RUSTUP_TOOLCHAIN || "1.99.1" },
    }).trim();
  return { format: IDENTITY_FORMAT, producer: "rust", sourceDigest: sourceDigest(root, "rust"), toolchain, buildId };
}

export function identity(buildId = process.env.TYPEFACTS_BUILD_ID || "dev", root = ROOT) {
  const toolchain = process.env.TYPEFACTS_TOOLCHAIN_IDENTITY ||
    execFileSync("go", ["version"], { encoding: "utf8" }).trim();
  return { format: IDENTITY_FORMAT, sourceDigest: sourceDigest(root), toolchain, buildId };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const buildIdIndex = process.argv.indexOf("--build-id");
  const buildId = buildIdIndex === -1 ? undefined : process.argv[buildIdIndex + 1];
  if (buildIdIndex !== -1 && !buildId) throw new Error("--build-id requires a value");
  const result = process.argv.includes("--rust") ? rustIdentity(buildId) : identity(buildId);
  process.stdout.write(process.argv.includes("--digest") ? result.sourceDigest : JSON.stringify(result));
}
