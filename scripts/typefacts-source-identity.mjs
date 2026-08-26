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

function collect(path, result) {
  const stat = lstatSync(path);
  if (stat.isDirectory()) {
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

export function identity(buildId = process.env.TYPEFACTS_BUILD_ID || "dev", root = ROOT) {
  const toolchain = process.env.TYPEFACTS_TOOLCHAIN_IDENTITY ||
    execFileSync("go", ["version"], { encoding: "utf8" }).trim();
  return { format: IDENTITY_FORMAT, sourceDigest: sourceDigest(root), toolchain, buildId };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const buildIdIndex = process.argv.indexOf("--build-id");
  const buildId = buildIdIndex === -1 ? undefined : process.argv[buildIdIndex + 1];
  if (buildIdIndex !== -1 && !buildId) throw new Error("--build-id requires a value");
  const result = identity(buildId);
  process.stdout.write(process.argv.includes("--digest") ? result.sourceDigest : JSON.stringify(result));
}
