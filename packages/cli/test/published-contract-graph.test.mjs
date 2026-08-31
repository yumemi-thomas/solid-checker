import assert from "node:assert/strict";
import { test } from "vitest";

import {
  bunLockLocatorForInstalledPackage,
  createBunLockSelectionIndex,
  PublishedGraphAcquisitionRefusal,
  discoverInstalledPublishedGraph,
  exactBunLockSelection,
  publishedGraphRequestKey
} from "../scripts/published-contract-graph.mjs";

const lock = `{
  "packages": {
    "root@1.0.0": ["root@1.0.0", "", {}, "sha512-root"],
    "leaf@2.0.0": ["leaf@2.0.0", "", {}, "sha512-leaf"],
  },
}`;

test("exact Bun selection binds the record locator and rejects absence", () => {
  assert.deepEqual(exactBunLockSelection(lock, "leaf", "2.0.0"), {
    locator: "leaf@2.0.0",
    integrity: "sha512-leaf"
  });
  assert.throws(
    () => exactBunLockSelection(lock, "missing", "1.0.0"),
    error => error instanceof PublishedGraphAcquisitionRefusal && error.kind === "missing-lock-selection"
  );
});

test("one parsed Bun lock index preserves exact locator and integrity selection", () => {
  const indexedLock = createBunLockSelectionIndex(`{
    "packages": {
      "parent/leaf": ["leaf@2.0.0", "", {}, "sha512-nested"],
      "other/leaf": ["leaf@2.0.0", "", {}, "sha512-other"],
      "broken@3.0.0": ["broken@3.0.0", "", {}],
    },
  }`);
  assert.deepEqual(exactBunLockSelection(indexedLock, "leaf", "2.0.0", "parent/leaf"), {
    locator: "parent/leaf",
    integrity: "sha512-nested"
  });
  assert.throws(
    () => exactBunLockSelection(indexedLock, "leaf", "2.0.0"),
    error =>
      error instanceof PublishedGraphAcquisitionRefusal &&
      error.kind === "ambiguous-lock-selection"
  );
  assert.throws(
    () => exactBunLockSelection(indexedLock, "broken", "3.0.0"),
    error =>
      error instanceof PublishedGraphAcquisitionRefusal &&
      error.kind === "missing-lock-integrity"
  );
});

test("raw and indexed Bun selection agree for top-level and nested copies", () => {
  const sameVersionLock = `{
    "packages": {
      "leaf": ["leaf@2.0.0", "", {}, "sha512-top-level"],
      "parent/leaf": ["leaf@2.0.0", "", {}, "sha512-nested"],
    },
  }`;
  const indexedLock = createBunLockSelectionIndex(sameVersionLock);
  for (const [locator, expected] of [
    ["leaf", { locator: "leaf", integrity: "sha512-top-level" }],
    ["parent/leaf", { locator: "parent/leaf", integrity: "sha512-nested" }]
  ]) {
    assert.deepEqual(
      exactBunLockSelection(sameVersionLock, "leaf", "2.0.0", locator),
      expected
    );
    assert.deepEqual(
      exactBunLockSelection(indexedLock, "leaf", "2.0.0", locator),
      expected
    );
  }
  for (const input of [sameVersionLock, indexedLock]) {
    assert.throws(
      () => exactBunLockSelection(input, "leaf", "2.0.0"),
      error =>
        error instanceof PublishedGraphAcquisitionRefusal &&
        error.kind === "ambiguous-lock-selection"
    );
  }
});

test("exact Bun selection preserves integrity and cardinality refusal precedence", () => {
  const missingIntegrityLock = createBunLockSelectionIndex(`{
    "packages": {
      "parent/leaf": ["leaf@2.0.0", "", {}],
      "leaf": ["leaf@2.0.0", "", {}, "sha512-top-level"],
    },
  }`);
  assert.deepEqual(
    exactBunLockSelection(missingIntegrityLock, "leaf", "2.0.0", "leaf"),
    { locator: "leaf", integrity: "sha512-top-level" }
  );
  assert.throws(
    () => exactBunLockSelection(missingIntegrityLock, "leaf", "2.0.0", "parent/leaf"),
    error =>
      error instanceof PublishedGraphAcquisitionRefusal &&
      error.kind === "missing-lock-integrity"
  );
  assert.throws(
    () => exactBunLockSelection(missingIntegrityLock, "leaf", "2.0.0"),
    error =>
      error instanceof PublishedGraphAcquisitionRefusal &&
      error.kind === "missing-lock-integrity"
  );

  const ambiguousLock = createBunLockSelectionIndex(`{
    "packages": {
      "leaf": ["leaf@2.0.0", "", {}, "sha512-top-level"],
      "parent/leaf": ["leaf@2.0.0", "", {}, "sha512-nested"],
    },
  }`);
  assert.throws(
    () => exactBunLockSelection(ambiguousLock, "leaf", "2.0.0"),
    error =>
      error instanceof PublishedGraphAcquisitionRefusal &&
      error.kind === "ambiguous-lock-selection"
  );
  assert.throws(
    () => exactBunLockSelection(ambiguousLock, "missing", "1.0.0"),
    error =>
      error instanceof PublishedGraphAcquisitionRefusal &&
      error.kind === "missing-lock-selection"
  );
});

test("installed Bun locator distinguishes nested copies at the same version", () => {
  assert.equal(
    bunLockLocatorForInstalledPackage(
      "/project/bun.lock",
      "/project/node_modules/@corvu/popover/node_modules/@corvu/utils"
    ),
    "@corvu/popover/@corvu/utils"
  );
});

test("run-wide graph reuse requires the complete canonical acquisition identity", () => {
  const request = {
    importer: "/project/src/index.ts",
    specifier: "leaf/subpath",
    packageRoot: "/project/node_modules/leaf",
    conditions: ["solid", "import", "solid"],
    integrity: "sha512-leaf"
  };
  assert.equal(
    publishedGraphRequestKey(request),
    publishedGraphRequestKey({ ...request, conditions: ["import", "solid"] })
  );
  for (const changed of [
    { importer: "/project/src/other.ts" },
    { specifier: "leaf/other" },
    { packageRoot: "/project/node_modules/other-leaf" },
    { conditions: ["browser"] },
    { integrity: "sha512-substituted" }
  ]) {
    assert.notEqual(
      publishedGraphRequestKey(request),
      publishedGraphRequestKey({ ...request, ...changed })
    );
  }
});

test("installed acquisition is dependency-first and exact-importer scoped", () => {
  const manifests = {
    "/project/node_modules/root": { name: "root", version: "1.0.0" },
    "/project/node_modules/leaf": { name: "leaf", version: "2.0.0" }
  };
  const graph = discoverInstalledPublishedGraph(
    {
      bunLockPath: "/project/bun.lock",
      root: {
        importer: "/project/root-entry.mjs",
        specifier: "root",
        packageRoot: "/project/node_modules/root",
        conditions: [],
        integrity: "sha512-root"
      }
    },
    {
      readLock: () => lock,
      readManifest: packageRoot => manifests[packageRoot],
      locatePackage: () => "/project/node_modules/leaf",
      resolveClosure: request => ({
        packageRoot: request.packageRoot,
        packageName: manifests[request.packageRoot].name,
        packageVersion: manifests[request.packageRoot].version,
        requestedEntrypoint: ".",
        closure: {
          hazards:
            request.specifier === "root"
              ? [{
                  kind: "unaccepted-external-dependency",
                  source: "./dist/index.js:leaf"
                }]
              : []
        }
      })
    }
  );
  assert.deepEqual(graph.nodes.map(node => node.packageName), ["leaf", "root"]);
  assert.equal(graph.nodes[0].importer, "/project/node_modules/root/dist/index.js");
  assert.deepEqual(graph.nodes[1].dependencies, [
    { specifier: "leaf", node: graph.nodes[0].key }
  ]);
});

test("installed acquisition refuses cycles and builtins", () => {
  const manifests = {
    "/project/node_modules/root": { name: "root", version: "1.0.0" },
    "/project/node_modules/leaf": { name: "leaf", version: "2.0.0" }
  };
  const common = {
    readLock: () => lock,
    readManifest: packageRoot => manifests[packageRoot],
    locatePackage: (_importer, name) =>
      `/project/node_modules/${name === "root" ? "root" : "leaf"}`
  };
  assert.throws(
    () => discoverInstalledPublishedGraph(
      {
        bunLockPath: "/project/bun.lock",
        root: {
          importer: "/entry.mjs",
          specifier: "root",
          packageRoot: "/project/node_modules/root",
          conditions: [],
          integrity: "sha512-root"
        }
      },
      {
        ...common,
        resolveClosure: request => ({
          packageRoot: request.packageRoot,
          packageName: manifests[request.packageRoot].name,
          packageVersion: manifests[request.packageRoot].version,
          requestedEntrypoint: ".",
          closure: {
            hazards: [{
              kind: "unaccepted-external-dependency",
              source: `./index.js:${request.specifier === "root" ? "leaf" : "root"}`
            }]
          }
        })
      }
    ),
    error => error instanceof PublishedGraphAcquisitionRefusal && error.kind === "cycle"
  );
  assert.throws(
    () => discoverInstalledPublishedGraph(
      {
        bunLockPath: "/project/bun.lock",
        root: {
          importer: "/entry.mjs",
          specifier: "root",
          packageRoot: "/project/node_modules/root",
          conditions: [],
          integrity: "sha512-root"
        }
      },
      {
        ...common,
        resolveClosure: () => ({
          packageRoot: "/project/node_modules/root",
          packageName: "root",
          packageVersion: "1.0.0",
          requestedEntrypoint: ".",
          closure: {
            hazards: [{ kind: "unaccepted-external-dependency", source: "./index.js:node:fs" }]
          }
        })
      }
    ),
    error =>
      error instanceof PublishedGraphAcquisitionRefusal &&
      error.kind === "unsupported-external-specifier"
  );
});
