// The vendored @tybys/wasm-util copy is the pinned upstream plus owned.patch
// and nothing else, carries its notices, and covers every path import the
// shipped reactors declare.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { cpSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

const packageRoot = fileURLToPath(new URL("..", import.meta.url));
const vendor = join(packageRoot, "vendor/wasm-util");
const upstream = join(packageRoot, "node_modules/@tybys/wasm-util");
const record = JSON.parse(readFileSync(join(vendor, "UPSTREAM.json"), "utf8"));
const sha256 = path => createHash("sha256").update(readFileSync(path)).digest("hex");
const files = directory => readdirSync(directory, { recursive: true })
  .filter(name => statSync(join(directory, name)).isFile())
  .map(name => relative(directory, join(directory, name)))
  .sort();

test("the upstream bytes are the installed pinned package's", () => {
  assert.equal(JSON.parse(readFileSync(join(upstream, "package.json"), "utf8")).version, record.version);
  for (const [file, digest] of Object.entries(record.upstreamFiles)) {
    assert.equal(sha256(join(upstream, file)), digest, file);
  }
});

test("the copy is exactly the upstream files plus owned.patch", () => {
  const owned = Object.keys(record.ownedFiles).sort();
  assert.deepEqual(files(vendor), [...owned, "LICENSE", "NOTICE", "UPSTREAM.json", "owned.patch"].sort());
  for (const [file, digest] of Object.entries(record.ownedFiles)) assert.equal(sha256(join(vendor, file)), digest, file);
  for (const file of Object.keys(record.upstreamFiles)) {
    const changed = record.changedFiles.includes(file);
    assert.equal(sha256(join(vendor, file)) === record.upstreamFiles[file], !changed, file);
  }
  assert.deepEqual(owned.filter(file => !(file in record.upstreamFiles)), record.addedFiles);

  const scratch = mkdtempSync(join(tmpdir(), "vendor-wasm-util-"));
  try {
    for (const file of Object.keys(record.upstreamFiles)) cpSync(join(upstream, file), join(scratch, file), { recursive: true });
    const applied = spawnSync("patch", ["-p1", "--quiet", "--input", join(vendor, "owned.patch")], { cwd: scratch, encoding: "utf8" });
    assert.equal(applied.status, 0, applied.stderr || applied.stdout);
    for (const file of owned) assert.equal(sha256(join(scratch, file)), record.ownedFiles[file], `reconstructed ${file}`);
  } finally {
    rmSync(scratch, { recursive: true, force: true });
  }
});

test("the copy carries its license and the Node.js path notice", () => {
  assert.equal(sha256(join(vendor, "LICENSE")), record.licenses.LICENSE.sha256);
  assert.match(readFileSync(join(vendor, "LICENSE"), "utf8"), /^MIT License\n\nCopyright \(c\) 2022-present Toyobayashi\n/);
  const notice = readFileSync(join(vendor, "NOTICE"), "utf8");
  assert.match(notice, /Copyright Joyent, Inc\. and other Node contributors\./);
  assert.match(notice, /Permission is hereby granted, free of charge/);
});

test("every path import the shipped reactors declare is one the copy repairs", () => {
  const repaired = ["path_create_directory", "path_filestat_get", "path_open", "path_readlink", "path_remove_directory", "path_unlink_file"];
  for (const artifact of ["owned/solid-checker.wasm", "owned/solid-typefacts.wasm"]) {
    const imports = WebAssembly.Module.imports(new WebAssembly.Module(readFileSync(join(packageRoot, artifact))))
      .filter(entry => entry.module === "wasi_snapshot_preview1" && entry.name.startsWith("path_"))
      .map(entry => entry.name);
    for (const name of imports) assert.ok(repaired.includes(name), `${artifact} imports ${name}, which the copy does not repair`);
  }
});

test("only the owned worker routes to the copy", () => {
  const routed = readdirSync(packageRoot)
    .filter(name => name.endsWith(".mjs") || name.endsWith(".cjs") || name.endsWith(".js"))
    .filter(name => readFileSync(join(packageRoot, name), "utf8").includes("vendor/wasm-util"));
  assert.deepEqual(routed, ["owned-worker-core.mjs"]);
});
