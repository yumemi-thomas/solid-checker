#!/usr/bin/env node
// ADR 0324: the digest of an installed `@solidjs/router` package's bytes, as
// the checker computes it (rust/crates/solid-facts-backend/src/installed_bytes.rs).
//
//   node scripts/router-digest.mjs <installed package directory>
//
// sha256 over, in order:
//   1. the line "solid-checker router bytes v1\n";
//   2. for each of name, version, main, module, exports, type of package.json,
//      the line `<key>=<value>\n`, where <value> is the JSON of the field with
//      object keys sorted at every depth and no whitespace, or `undefined`
//      when the field is absent;
//   3. for each regular file under <directory>/dist, recursively, sorted by
//      its path relative to dist (UTF-8 bytes, "/" separators): the bytes
//      `file\0<relative path>\0`, the content length as a little-endian u64,
//      then the content.
// A symlink anywhere under dist, or an unreadable entry, has no digest.
import { createHash } from "node:crypto";
import { lstatSync, readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";

const FIELDS = ["name", "version", "main", "module", "exports", "type"];

function canonical(value) {
  if (value === undefined) return "undefined";
  const sorted = (node) =>
    node !== null && typeof node === "object" && !Array.isArray(node)
      ? Object.fromEntries(Object.keys(node).sort().map((key) => [key, sorted(node[key])]))
      : Array.isArray(node)
        ? node.map(sorted)
        : node;
  return JSON.stringify(sorted(value));
}

function files(directory, prefix = "") {
  const found = [];
  for (const entry of readdirSync(directory)) {
    const path = join(directory, entry);
    const stat = lstatSync(path);
    if (stat.isSymbolicLink()) throw new Error(`symlink: ${path}`);
    if (stat.isDirectory()) found.push(...files(path, `${prefix}${entry}/`));
    else if (stat.isFile()) found.push([`${prefix}${entry}`, path]);
    else throw new Error(`not a regular file: ${path}`);
  }
  return found;
}

export function routerDigest(directory) {
  const manifest = JSON.parse(readFileSync(join(directory, "package.json"), "utf8"));
  const hash = createHash("sha256");
  hash.update("solid-checker router bytes v1\n");
  for (const key of FIELDS) hash.update(`${key}=${canonical(manifest[key])}\n`);
  const entries = files(join(directory, "dist")).sort(([a], [b]) =>
    Buffer.compare(Buffer.from(a), Buffer.from(b)),
  );
  for (const [relative, path] of entries) {
    const content = readFileSync(path);
    const length = Buffer.alloc(8);
    length.writeBigUInt64LE(BigInt(content.length));
    hash.update(Buffer.concat([Buffer.from(`file\0${relative}\0`), length]));
    hash.update(content);
  }
  return hash.digest("hex");
}

if (process.argv[1] === new URL(import.meta.url).pathname) {
  console.log(routerDigest(process.argv[2]));
}
