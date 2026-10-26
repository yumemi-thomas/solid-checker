#!/usr/bin/env bun
// Checks the reproduction set's recorded package bytes against the npm registry.
//
// scripts/repro-budget.mjs (offline, run by `make verify`) holds
// fixtures/uncertifiable-repro/node_modules to the sha256 values in
// packages.json, but never checks the record itself. This script does: for each
// package it fetches https://registry.npmjs.org/<name>/<version>, compares the
// registry's dist.integrity to the recorded integrity, downloads dist.tarball
// and checks its sha512 against the same integrity, then unpacks the tarball in
// memory and compares every recorded file's sha256 to the tarball's bytes and to
// the file in node_modules. A recorded file the tarball lacks is a mismatch.
//
// Opt-in: it needs network and is not a dependency of `make verify`.
//
//   bun scripts/repro-registry-check.mjs                    check, exit 1 on any mismatch
//   bun scripts/repro-registry-check.mjs --packages <path>  another packages.json
//
// Exit 0 when every package matches, 1 on a mismatch, 2 on a network failure
// (the URL is named). Mismatches are never repaired here: the record is edited
// by hand, deliberately.
import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";
import { gunzipSync } from "node:zlib";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const REGISTRY = "https://registry.npmjs.org";

class NetworkError extends Error {
  constructor(url, detail) {
    super(`${url}: ${detail}`);
    this.url = url;
  }
}

const sha = (algorithm, bytes, encoding) => createHash(algorithm).update(bytes).digest(encoding);

/** The registry URL of one version; a scoped name keeps its slash encoded, as npm sends it. */
export function versionUrl(name, version) {
  return `${REGISTRY}/${name.replace("/", "%2f")}/${encodeURIComponent(version)}`;
}

async function fetchBytes(url) {
  let response;
  try {
    response = await fetch(url);
  } catch (error) {
    throw new NetworkError(url, error.cause?.message ?? error.message);
  }
  if (response.status === 404) return null;
  if (!response.ok) throw new NetworkError(url, `HTTP ${response.status}`);
  try {
    return Buffer.from(await response.arrayBuffer());
  } catch (error) {
    throw new NetworkError(url, error.message);
  }
}

const field = (block, from, length) => {
  const end = block.indexOf(0, from);
  return block.toString("utf8", from, end === -1 || end > from + length ? from + length : end);
};

/** The regular files of a gzipped tar as a Map of path (leading `package/` removed) to bytes. */
export function untar(gzipped) {
  const tar = gunzipSync(gzipped);
  const files = new Map();
  let longName = null;
  let paxPath = null;
  for (let at = 0; at + 512 <= tar.length; ) {
    const header = tar.subarray(at, at + 512);
    if (header.every((byte) => byte === 0)) break;
    const size = parseInt(field(header, 124, 12).trim() || "0", 8);
    const type = String.fromCharCode(header[156] || 48);
    const body = tar.subarray(at + 512, at + 512 + size);
    at += 512 + Math.ceil(size / 512) * 512;
    if (type === "L") {
      longName = body.toString("utf8").replace(/\0.*$/s, "");
    } else if (type === "x") {
      const match = /(?:^|\n)\d+ path=([^\n]*)\n/.exec(body.toString("utf8"));
      if (match) paxPath = match[1];
    } else if (type === "0") {
      const prefix = field(header, 345, 155);
      let name = longName ?? paxPath ?? (prefix ? `${prefix}/${field(header, 0, 100)}` : field(header, 0, 100));
      longName = paxPath = null;
      name = name.replace(/^[^/]+\//, "");
      files.set(name, Buffer.from(body));
    } else {
      longName = paxPath = null;
    }
  }
  return files;
}

/** Every difference between one recorded package and the registry, as printable lines. */
async function checkPackage(pkg, modulesRoot) {
  const found = [];
  const url = versionUrl(pkg.name, pkg.version);
  const metadata = await fetchBytes(url);
  if (!metadata) return [`${url}: not in the registry`];
  const dist = JSON.parse(metadata.toString("utf8")).dist ?? {};
  if (dist.integrity !== pkg.integrity) found.push(`integrity: recorded ${pkg.integrity}, registry ${dist.integrity}`);
  if (!dist.tarball) return [...found, "registry lists no dist.tarball"];

  const tarball = await fetchBytes(dist.tarball);
  if (!tarball) return [...found, `${dist.tarball}: not in the registry`];
  const actual = `sha512-${sha("sha512", tarball, "base64")}`;
  if (actual !== pkg.integrity) found.push(`tarball sha512: recorded integrity ${pkg.integrity}, downloaded ${actual}`);

  let files;
  try {
    files = untar(tarball);
  } catch (error) {
    return [...found, `${dist.tarball}: cannot unpack (${error.message})`];
  }
  for (const [file, recorded] of Object.entries(pkg.files ?? {})) {
    const published = files.get(file);
    if (!published) {
      found.push(`${file}: recorded but not in the tarball`);
      continue;
    }
    const inTarball = sha("sha256", published, "hex");
    if (inTarball !== recorded) found.push(`${file}: recorded sha256 ${recorded}, tarball ${inTarball}`);
    const local = join(modulesRoot, pkg.name, file);
    if (!existsSync(local)) found.push(`${file}: absent from node_modules`);
    else {
      const onDisk = sha("sha256", readFileSync(local), "hex");
      if (onDisk !== inTarball) found.push(`${file}: node_modules sha256 ${onDisk}, tarball ${inTarball}`);
    }
  }
  return found;
}

async function main(argv) {
  const at = argv.indexOf("--packages");
  const path = at === -1 ? join(ROOT, "fixtures/uncertifiable-repro/packages.json") : argv[at + 1];
  if (!path) {
    console.error("repro-registry-check: --packages needs a path");
    return 2;
  }
  const modulesRoot = join(dirname(path), "node_modules");
  const packages = JSON.parse(readFileSync(path, "utf8")).packages ?? [];
  let mismatched = false;
  let unreachable = false;
  for (const pkg of packages) {
    const label = `${pkg.name}@${pkg.version}`;
    try {
      const found = await checkPackage(pkg, modulesRoot);
      if (found.length === 0) console.log(`OK ${label} (${Object.keys(pkg.files ?? {}).length} files)`);
      else {
        mismatched = true;
        console.log(`MISMATCH ${label}`);
        for (const line of found) console.log(`  ${line}`);
      }
    } catch (error) {
      if (!(error instanceof NetworkError)) throw error;
      unreachable = true;
      console.error(`NETWORK ${label}: ${error.message}`);
    }
  }
  return unreachable ? 2 : mismatched ? 1 : 0;
}

if (process.argv[1] && process.argv[1] === fileURLToPath(import.meta.url)) {
  process.exit(await main(process.argv.slice(2)));
}
