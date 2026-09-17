import assert from "node:assert/strict";
import { test } from "vitest";

import {
  bunLockLocatorForInstalledPackage,
  createBunLockSelectionIndex,
  createPnpmLockSelectionIndex,
  PublishedGraphAcquisitionRefusal,
  discoverInstalledPublishedGraph,
  exactBunLockSelection,
  exactLockSelection,
  lockLocatorForInstalledPackage,
  parsePnpmLockPackages,
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


// The pnpm reader is the acquisition half of a pair: Rust re-reads the same
// bytes in `dependencies.rs` before any receipt, and these cases mirror the
// `pnpm_selection_*` tests there. A subset either side reads and the other
// refuses is the failure mode worth pinning, so the two rosters match.

const PNPM_INTEGRITY =
  "sha512-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA==";

const pnpmEntry = `  '@corvu/utils@0.3.2':\n    resolution: {integrity: ${PNPM_INTEGRITY}}\n`;

const pnpmLock = body =>
  `lockfileVersion: '9.0'\n\nsettings:\n  autoInstallPeers: true\n\npackages:\n\n${body}`;

const refusal = kind => error =>
  error instanceof PublishedGraphAcquisitionRefusal && error.kind === kind;

test("exact pnpm selection reads the packages key as the locator", () => {
  const index = createPnpmLockSelectionIndex(
    pnpmLock(`${pnpmEntry}    engines: {node: '>=10'}\n`)
  );
  assert.deepEqual(
    exactLockSelection({
      index,
      packageManager: "pnpm",
      lockfilePath: "/w/pnpm-lock.yaml",
      packageRoot: "/w/node_modules/.pnpm/@corvu+utils@0.3.2_solid-js@1.9.14/node_modules/@corvu/utils",
      packageName: "@corvu/utils",
      packageVersion: "0.3.2"
    }),
    { locator: "@corvu/utils@0.3.2", integrity: PNPM_INTEGRITY }
  );
  assert.throws(
    () =>
      exactLockSelection({
        index,
        packageManager: "pnpm",
        lockfilePath: "/w/pnpm-lock.yaml",
        packageRoot: "/w/node_modules/missing",
        packageName: "missing",
        packageVersion: "1.0.0"
      }),
    refusal("missing-lock-selection")
  );
});

test("the pnpm locator is the lock key, not the install path", () => {
  // pnpm stores a package under `.pnpm/<name>@<version>_<peers>/node_modules/`,
  // which names no key the lockfile wrote. Deriving a locator from that path --
  // as Bun's tree requires -- would invent a string nothing can select.
  assert.equal(
    lockLocatorForInstalledPackage({
      lockfilePath: "/w/pnpm-lock.yaml",
      packageManager: "pnpm",
      packageRoot: "/w/node_modules/.pnpm/@corvu+utils@0.3.2_solid-js@1.9.14/node_modules/@corvu/utils",
      packageName: "@corvu/utils",
      packageVersion: "0.3.2"
    }),
    "@corvu/utils@0.3.2"
  );
  assert.throws(
    () =>
      lockLocatorForInstalledPackage({
        lockfilePath: "/w/yarn.lock",
        packageManager: "yarn",
        packageRoot: "/w/node_modules/@corvu/utils",
        packageName: "@corvu/utils",
        packageVersion: "0.3.2"
      }),
    refusal("unsupported-package-manager")
  );
});

test("the pnpm reader reads a formatter-rewritten lockfile", () => {
  // A real lockfile in the consumer corpus had been through Prettier: keys
  // double-quoted, `resolution` wrapped across lines with a trailing comma. A
  // reader that assumed pnpm's own layout answers "no packages" for it.
  const selections = parsePnpmLockPackages(
    `lockfileVersion: "9.0"\n\npackages:\n  "@corvu/utils@0.3.2":\n    resolution:\n      {\n        integrity: ${PNPM_INTEGRITY},\n      }\n    engines: { node: ">=10" }\n`
  );
  assert.deepEqual([...selections], [["@corvu/utils@0.3.2", PNPM_INTEGRITY]]);
});

test("the pnpm reader reads a workspace specifier as a scalar", () => {
  // `workspace:*` is a plain scalar, not an alias: in YAML `*` opens a node only
  // at a token boundary, and a bare `:` is not one. Every lockfile in the demand
  // corpus carries this line, so treating it as an alias refuses all of them --
  // which is exactly what both readers did until this case was written.
  const selections = parsePnpmLockPackages(
    `lockfileVersion: '9.0'\n\nimporters:\n\n  .:\n    dependencies:\n      '@corvu/utils':\n        specifier: workspace:*\n        version: link:packages/utils\n\npackages:\n\n${pnpmEntry}`
  );
  assert.deepEqual([...selections], [["@corvu/utils@0.3.2", PNPM_INTEGRITY]]);
});

test("the pnpm reader refuses a lockfile major before 9", () => {
  // Major 6 wrote peer suffixes into `packages:` keys, so one name@version
  // could appear under several keys with no installed path to separate them.
  assert.throws(
    () =>
      parsePnpmLockPackages(
        `lockfileVersion: '6.0'\n\npackages:\n\n  /@corvu/utils@0.3.2:\n    resolution: {integrity: ${PNPM_INTEGRITY}}\n`
      ),
    refusal("unsupported-lock-version")
  );
});

test("the pnpm reader refuses a repeated packages key", () => {
  assert.throws(
    () =>
      parsePnpmLockPackages(
        pnpmLock(`${pnpmEntry}  '@corvu/utils@0.3.2':\n    resolution: {integrity: sha512-BBBB==}\n`)
      ),
    refusal("ambiguous-lock-selection")
  );
});

test("the pnpm reader refuses YAML beyond the subset it reads", () => {
  // Each of these can move a value from one entry to another, or redefine
  // `packages:` wholesale; skipping what it did not understand would answer
  // confidently from the wrong bytes.
  for (const [name, source] of [
    ["anchor", pnpmLock(`  base: &shared\n${pnpmEntry}`)],
    ["alias", pnpmLock(`${pnpmEntry}    extra: *shared\n`)],
    ["merge key", pnpmLock(`${pnpmEntry}    <<: *shared\n`)],
    ["second document", `${pnpmLock(pnpmEntry)}---\npackages:\n${pnpmEntry}`],
    ["tab", pnpmLock(pnpmEntry).replace("  '@corvu", "\t'@corvu")]
  ]) {
    assert.throws(
      () => parsePnpmLockPackages(source),
      refusal("unsupported-lock-syntax"),
      `${name} was read instead of refused`
    );
  }
});

test("the pnpm reader selects no package without a registry integrity", () => {
  // A tarball, git or link dependency cannot be authenticated against a
  // registry, so leaving it unselected refuses the graph rather than naming it.
  assert.throws(
    () =>
      parsePnpmLockPackages(
        pnpmLock("  '@corvu/utils@0.3.2':\n    resolution: {tarball: https://example.invalid/utils.tgz}\n")
      ),
    refusal("missing-lock-selection")
  );
});
