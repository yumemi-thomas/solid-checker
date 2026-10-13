#!/usr/bin/env node

// Bundle identity of the pinned headless-shell browser (ADR 0033).
//
// `PROBE_BROWSER` names the *real path* of the executable; the pin is the
// path-ordered tree digest of the directory holding it, because the executable
// loads its siblings at start-up (ICU data, the V8 context snapshot, resource
// packs, GPU libraries) and each shapes the realm a recipe runs in.
//
// The framing is exactly `hash_tree` in
// rust/crates/solid-facts-backend/src/contract_certification/probe_harness.rs,
// which the verifier recomputes before every launch and re-asserts in every
// census: the domain string, then per regular file in byte order of its
// relative path — the path's byte length as a big-endian u64, the path, the
// file length as a big-endian u64, and the file's `sha256:<hex>` digest string.
// A symlink or any non-regular entry refuses, as it does in Rust.
// `probe_harness::tests::the_probe_browser_bundle_identity_script_matches_the_native_tree_digest`
// pins the two implementations against each other.

import { createHash } from "node:crypto";
import { lstatSync, readdirSync, readFileSync, realpathSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

function u64(value) {
  const bytes = Buffer.alloc(8);
  bytes.writeBigUInt64BE(BigInt(value));
  return bytes;
}

function collect(root, directory, entries) {
  for (const name of readdirSync(directory)) {
    const path = join(directory, name);
    const stat = lstatSync(path);
    if (stat.isDirectory()) {
      collect(root, path, entries);
    } else if (stat.isFile()) {
      const contents = readFileSync(path);
      entries.push({
        relative: relative(root, path).replaceAll("\\", "/"),
        digest: `sha256:${createHash("sha256").update(contents).digest("hex")}`,
        length: contents.length
      });
    } else {
      throw new Error(`browser bundle entry ${path} is not a regular file or directory`);
    }
  }
}

export function bundleDigest(executable) {
  const stat = lstatSync(executable);
  if (!stat.isFile()) {
    throw new Error(`the pinned browser executable must be a regular file named by its real path: ${executable}`);
  }
  const root = dirname(executable);
  const entries = [];
  collect(root, root, entries);
  entries.sort((left, right) => Buffer.compare(Buffer.from(left.relative), Buffer.from(right.relative)));
  const hash = createHash("sha256");
  hash.update("solid-checker:probe-private-tree:v1\0");
  for (const entry of entries) {
    const path = Buffer.from(entry.relative);
    hash.update(u64(path.length));
    hash.update(path);
    hash.update(u64(entry.length));
    hash.update(entry.digest);
  }
  return hash.digest("hex");
}

if (process.argv[1] && realpathSync(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const executable = process.argv[2];
  if (!executable) {
    process.stderr.write("usage: probe-browser-identity.mjs <real path of the headless-shell executable>\n");
    process.exit(2);
  }
  process.stdout.write(bundleDigest(executable));
}
