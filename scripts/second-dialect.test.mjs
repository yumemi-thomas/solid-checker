// Would a second dialect actually arrive where it is supposed to?
//
// The scripts under `scripts/` claim to enumerate `rust/dialects/*/dialect.json`
// rather than naming `solid-v2`. While exactly one dialect ships, that claim is
// indistinguishable from a literal: every one of them would pass today if it
// had `solid-v2` written in it, which is how eight of them came to. So this
// assembles a *synthetic* `solid-v3` in a throwaway tree and demands that each
// enumerator picks it up with no edit anywhere.
//
// It proves the JavaScript half only. Registering a dialect in
// `solid_facts_backend::dialect::ALL`, adding a `Version` variant and
// implementing the vocabulary are compile-time integration decisions, and
// `docs/adding-a-dialect.md` says deliberately that their omissions should fail
// compilation rather than a gate. What this covers is the other half -- the
// places where an omission would fail *nothing* and the new dialect would
// simply be unchecked.
import assert from "node:assert/strict";
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import { loadDialectManifests } from "./dialect-manifests.mjs";
import { carriedSolidMajors, dialectStubProblems } from "./lib/dialect-stubs.mjs";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");

/**
 * A tree with this repository's `rust/dialects/` plus a synthetic `solid-v3`,
 * its rule manifest, and the bundle-index files its manifest must name.
 *
 * Only what the enumerators read is copied: these tests are about discovery,
 * not about whether a dialect's contracts are real.
 */
const withSecondDialect = (body) => {
  const root = mkdtempSync(join(tmpdir(), "solid-second-dialect-"));
  try {
    cpSync(join(ROOT, "rust/dialects"), join(root, "rust/dialects"), { recursive: true });
    cpSync(join(ROOT, "packages/cli/lib"), join(root, "packages/cli/lib"), { recursive: true });
    const manifest = {
      schemaVersion: 2,
      id: "solid-v3",
      ruleManifest: "packages/cli/lib/rules-solid-v3.json",
      bundleIndex: "pkg/contracts/bundled/solid-v3/bundle-index.json",
      reviewBundleIndex: "rust/crates/solid-dialect/contracts/solid-v3/bundle-index.json",
      contracts: [{ package: "solid-js" }],
    };
    mkdirSync(join(root, "rust/dialects/solid-v3"), { recursive: true });
    writeFileSync(
      join(root, "rust/dialects/solid-v3/dialect.json"),
      `${JSON.stringify(manifest, null, 2)}\n`
    );
    writeFileSync(
      join(root, "packages/cli/lib/rules-solid-v3.json"),
      `${JSON.stringify(
        {
          schemaVersion: 1,
          dialect: "solid-v3",
          config: "v3",
          namespace: "",
          docsBaseUrl: "https://example.invalid/docs/rules",
          rules: [
            {
              code: "SC1001",
              name: "strict-read-untracked",
              severity: "warning",
              uncertifiable: false,
              defaultEnabled: true,
              presets: [],
            },
          ],
        },
        null,
        2
      )}\n`
    );
    return body(root);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
};

test("a second assembly manifest is enumerated without editing the loader", () => {
  withSecondDialect((root) => {
    const ids = loadDialectManifests({ projectRoot: root }).map((manifest) => manifest.id);
    assert.deepEqual(ids, ["solid-v2", "solid-v3"]);
  });
});

// `scripts/lib/tsc-oracle-case.mjs` derives `DIALECTS` and `catalogEntries`
// from the manifests; `ownership-gate.mjs` and `tsc-oracle-gate.mjs` derive
// their accepted case dialects and their cache tree lists from `DIALECTS`. So
// this one derivation is what carries a new dialect into three gates.
test("the manifest id is what the oracle and ownership gates accept", () => {
  withSecondDialect((root) => {
    const manifests = loadDialectManifests({ projectRoot: root });
    const shorts = manifests.map((manifest) => manifest.id.slice("solid-".length));
    assert.deepEqual(shorts, ["v2", "v3"]);
    assert.deepEqual(
      shorts.map((short) => `solid-${short}`),
      manifests.map((manifest) => manifest.id),
      "the short form round-trips, which is what the ownership gate relies on"
    );
  });
});

test("a new dialect's rules join the catalog every oracle case is checked against", () => {
  withSecondDialect((root) => {
    const entries = loadDialectManifests({ projectRoot: root }).flatMap(
      (manifest) => JSON.parse(readFileSync(join(root, manifest.ruleManifest), "utf8")).rules
    );
    assert.ok(entries.length > 1, "both catalogs contribute");
  });
});

// ESLint catalog discovery is a filename pattern in `packages/cli/eslint.cjs`
// (`rules-solid-vN.json` under `lib/`), not a registry. Asserting the pattern
// here keeps step 8 of the checklist from being the one that silently does
// nothing.
test("the shipped ESLint adapter's discovery pattern matches a new rule manifest", () => {
  const pattern = /^rules-solid-v\d+\.json$/;
  const adapter = readFileSync(join(ROOT, "packages/cli/eslint.cjs"), "utf8");
  assert.ok(
    adapter.includes(pattern.source),
    "eslint.cjs still discovers catalogs by this pattern; update this test with it"
  );
  assert.ok(pattern.test("rules-solid-v3.json"));
});

test("a stub naming the new dialect's major stops being refused", () => {
  withSecondDialect((root) => {
    const majors = carriedSolidMajors(root);
    assert.deepEqual([...majors].sort((a, b) => a - b), [2, 3]);
    const fixtures = join(root, "fixtures/reactive-ir/subject/node_modules/solid-js");
    mkdirSync(fixtures, { recursive: true });
    const id = "fixtures/reactive-ir/subject/node_modules/solid-js/package.json";
    writeFileSync(join(root, id), JSON.stringify({ name: "solid-js", version: "3.0.0" }));
    const ask = (carried) =>
      dialectStubProblems({
        projectRoot: root,
        groups: ["reactive-ir"],
        tracked: new Set([id]),
        majors: carried,
      });
    // Both directions, because `[]` is also what a check that found no stub at
    // all would return.
    assert.match(ask(new Set([2]))[0], /names major 3/);
    assert.deepEqual(
      ask(majors),
      [],
      "coverage refuses a major no dialect carries; adding the dialect must be the whole fix"
    );
  });
});

// Codes are the portable half of a rule's identity -- a suppression comment
// names the code, and `docs/rules/` addresses a page by name. Two catalogs may
// deliberately share a code for one concept (`SC1001` is
// `strict-read-untracked` in both), which is why this asserts agreement rather
// than uniqueness: a code that meant one thing in one catalog and something
// else in another would make a suppression mean two things.
test("a code shared across catalogs names the same rule in each", () => {
  withSecondDialect((root) => {
    const byCode = new Map();
    for (const manifest of loadDialectManifests({ projectRoot: root })) {
      for (const rule of JSON.parse(readFileSync(join(root, manifest.ruleManifest), "utf8"))
        .rules) {
        const seen = byCode.get(rule.code);
        if (seen) {
          assert.equal(
            rule.name,
            seen,
            `${rule.code} is ${seen} in one catalog and ${rule.name} in another`
          );
        } else {
          byCode.set(rule.code, rule.name);
        }
      }
    }
    assert.equal(byCode.get("SC1001"), "strict-read-untracked");
  });
});

test("this repository's own catalogs satisfy the shared-code rule", () => {
  const byCode = new Map();
  for (const manifest of loadDialectManifests()) {
    for (const rule of JSON.parse(readFileSync(join(ROOT, manifest.ruleManifest), "utf8")).rules) {
      const seen = byCode.get(rule.code);
      if (seen) assert.equal(rule.name, seen, `${rule.code} disagrees across catalogs`);
      byCode.set(rule.code, rule.name);
    }
  }
  assert.ok(byCode.size > 0);
});
