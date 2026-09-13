#!/usr/bin/env node
// Pin the exact published `solid-js@1.9.14` archive the Solid 1.x dialect's
// negative authority was audited against, the way `audit-solid-rc3.mjs` pins
// the 2.0.0-rc.3 archives: fetch the registry document, fetch the tarball it
// names, refuse on any disagreement between the two (SRI, SHA-1, file count,
// unpacked size), and write the per-file manifest the citation tests read.
//
//   node scripts/audit-solid-1x.mjs [--output benchmarks/package-contract-v2/phase0/solid-1x]
//
// Outputs, under `<output>/solid-js/`:
//   registry-metadata.json  the registry's version document, verbatim
//   package.json            the archive's own manifest bytes, verbatim
//   exports.json            its `exports` map, for the condition table
//   files.json              [{ path, sha256, bytes }] for every archive file
//
// The rows in `rust/crates/solid-dialect/src/solid_1x.rs` cite `files.json`
// digests and byte ranges; `audited-slices/solid-v1/` carries the cited bytes.
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, relative, resolve } from "node:path";

const NAME = "solid-js";
const VERSION = "1.9.14";
const METADATA_URL = `https://registry.npmjs.org/${NAME}/${VERSION}`;

function sha(algorithm, bytes, encoding = "hex") {
  return createHash(algorithm).update(bytes).digest(encoding);
}

function walkFiles(root) {
  const out = [];
  for (const entry of readdirSync(root, { withFileTypes: true })) {
    const path = join(root, entry.name);
    if (entry.isDirectory()) out.push(...walkFiles(path));
    else if (entry.isFile()) out.push(path);
  }
  return out.sort();
}

function assertSafeArchiveEntries(entries) {
  for (const entry of entries) {
    if (!entry.startsWith("package/") || entry.includes("..") || entry.startsWith("/")) {
      throw new Error(`refusing archive entry ${JSON.stringify(entry)}`);
    }
  }
}

async function main() {
  const argv = process.argv.slice(2);
  const outputIndex = argv.indexOf("--output");
  const outputDir = resolve(
    outputIndex >= 0 ? argv[outputIndex + 1] : "benchmarks/package-contract-v2/phase0/solid-1x"
  );

  const metadataResponse = await fetch(METADATA_URL, { headers: { accept: "application/json" } });
  if (!metadataResponse.ok) throw new Error(`${METADATA_URL} returned ${metadataResponse.status}`);
  const metadataText = await metadataResponse.text();
  const metadata = JSON.parse(metadataText);
  if (metadata.name !== NAME || metadata.version !== VERSION) {
    throw new Error(`registry identity mismatch for ${NAME}@${VERSION}`);
  }
  const tarballResponse = await fetch(metadata.dist.tarball);
  if (!tarballResponse.ok) throw new Error(`${metadata.dist.tarball} returned ${tarballResponse.status}`);
  const tarball = Buffer.from(await tarballResponse.arrayBuffer());
  if (`sha512-${sha("sha512", tarball, "base64")}` !== metadata.dist.integrity) {
    throw new Error(`${NAME} SRI mismatch`);
  }
  if (sha("sha1", tarball) !== metadata.dist.shasum) throw new Error(`${NAME} SHA-1 mismatch`);

  const scratch = mkdtempSync(join(tmpdir(), "solid-checker-audit-solid-1x-"));
  try {
    const tarPath = join(scratch, "solid-js.tgz");
    writeFileSync(tarPath, tarball);
    const entries = execFileSync("tar", ["-tzf", tarPath], { encoding: "utf8" }).split("\n").filter(Boolean);
    assertSafeArchiveEntries(entries);
    execFileSync("tar", ["-xzf", tarPath, "-C", scratch]);
    const packageRoot = join(scratch, "package");
    const files = walkFiles(packageRoot).map(path => {
      const bytes = readFileSync(path);
      return { path: relative(packageRoot, path), sha256: sha("sha256", bytes), bytes: bytes.length };
    });
    const unpackedBytes = files.reduce((sum, file) => sum + file.bytes, 0);
    if (metadata.dist.fileCount !== undefined && files.length !== metadata.dist.fileCount) {
      throw new Error(`${NAME} file-count mismatch: ${files.length} != ${metadata.dist.fileCount}`);
    }
    if (metadata.dist.unpackedSize !== undefined && unpackedBytes !== metadata.dist.unpackedSize) {
      throw new Error(`${NAME} unpacked-size mismatch: ${unpackedBytes} != ${metadata.dist.unpackedSize}`);
    }
    const manifestBytes = readFileSync(join(packageRoot, "package.json"));
    const manifest = JSON.parse(manifestBytes);
    for (const [key, value] of Object.entries(manifest.exports)) {
      if (key.includes("*")) continue;
      const targets = [];
      const collect = node => {
        if (typeof node === "string") targets.push(node);
        else if (node && typeof node === "object") Object.values(node).forEach(collect);
      };
      collect(value);
      for (const target of targets) {
        const local = join(packageRoot, target.replace(/^\.\//, ""));
        if (!(statSync(local, { throwIfNoEntry: false })?.isFile() ?? false)) {
          throw new Error(`${NAME} export target is missing: ${target}`);
        }
      }
    }

    const packageOutput = join(outputDir, "solid-js");
    mkdirSync(packageOutput, { recursive: true });
    writeFileSync(join(packageOutput, "registry-metadata.json"), `${JSON.stringify(metadata, null, 2)}\n`);
    writeFileSync(join(packageOutput, "package.json"), manifestBytes);
    writeFileSync(join(packageOutput, "exports.json"), `${JSON.stringify(manifest.exports, null, 2)}\n`);
    writeFileSync(join(packageOutput, "files.json"), `${JSON.stringify(files, null, 2)}\n`);
    console.log(
      `${NAME}@${VERSION}: ${files.length} files, ${unpackedBytes} bytes, integrity ${metadata.dist.integrity}, ` +
        `package.json sha256 ${sha("sha256", manifestBytes)} -> ${packageOutput}`
    );
  } finally {
    rmSync(scratch, { recursive: true, force: true });
  }
}

main().catch(error => {
  console.error(`audit-solid-1x: ${error.message}`);
  process.exit(1);
});
