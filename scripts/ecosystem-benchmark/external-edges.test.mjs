import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "vitest";

import {
  collectExternalEdges,
  extractExternalModuleSpecifiers,
  splitPackageSpecifier
} from "./lib/external-edges.mjs";

test("external edge extraction retains every exact package subpath", () => {
  const multiEdge = [
    'cannot statically expand external export-all "@tanstack/form-core" from /tmp/form.js',
    'cannot statically expand external export-all "@tanstack/solid-store" from /tmp/form.js',
    'cannot statically expand external export-all "@tanstack/store" from /tmp/store.js',
    'cannot statically expand external export-all "@tanstack/pacer/rate-limiter" from /tmp/pacer.js',
    'cannot statically expand external export-all "@tanstack/table-core/static-functions" from /tmp/table.js',
    'cannot statically expand external export-all "@corvu/accordion" from /tmp/corvu.js'
  ].join("\n");
  assert.deepEqual(extractExternalModuleSpecifiers(multiEdge), [
    "@corvu/accordion",
    "@tanstack/form-core",
    "@tanstack/pacer/rate-limiter",
    "@tanstack/solid-store",
    "@tanstack/store",
    "@tanstack/table-core/static-functions"
  ]);
  assert.deepEqual(splitPackageSpecifier("@tanstack/table-core/static-functions"), {
    specifier: "@tanstack/table-core/static-functions",
    package: "@tanstack/table-core",
    entrypoint: "./static-functions"
  });
});

test("external edge extraction retains an exact accepted-binding frontier", () => {
  assert.deepEqual(
    extractExternalModuleSpecifiers(
      "accepted dependency @corvu/disclosure has no exact runtime binding for export useContext"
    ),
    ["@corvu/disclosure"]
  );
});

test("external edges bind the nearest installed exact dependency version", () => {
  const projectDir = mkdtempSync(join(tmpdir(), "solid-checker-edge-version-"));
  const packageRoot = join(projectDir, "node_modules", "root");
  const nested = join(packageRoot, "node_modules", "@tanstack", "form-core");
  mkdirSync(nested, { recursive: true });
  writeFileSync(
    join(nested, "package.json"),
    JSON.stringify({ name: "@tanstack/form-core", version: "2.0.0" })
  );
  try {
    assert.deepEqual(
      collectExternalEdges({
        texts: ['cannot statically expand external export-all "@tanstack/form-core" from /tmp/form.js'],
        projectDir,
        packageRoot
      }),
      [{
        specifier: "@tanstack/form-core",
        package: "@tanstack/form-core",
        entrypoint: ".",
        resolvedVersion: "2.0.0"
      }]
    );
  } finally {
    rmSync(projectDir, { recursive: true, force: true });
  }
});
