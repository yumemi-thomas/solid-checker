import assert from "node:assert/strict";
import { test } from "vitest";

import {
  census,
  compare,
  nameableEntrypoint,
  summaryState,
  surfacesFromDocuments
} from "./contract-coverage-census.mjs";

function document(overrides = {}) {
  return {
    package: { name: "@example/utils" },
    summaries: {
      "with-ops": { call: { operations: [{ id: "read-0", kind: "read" }] } },
      "with-owner": {
        call: {
          operations: [
            {
              id: "cleanup-0",
              kind: "cleanup",
              owner: { requires: "required", requiresCleanup: "required" }
            }
          ]
        }
      },
      "closed-nothing": { call: { closed: ["reads", "creates"], reads: [], creates: [] } },
      degenerate: { call: {} }
    },
    entrypoints: {},
    ...overrides
  };
}

test("a determined negative is not a gap, and a degenerate summary is", () => {
  const source = document();
  assert.equal(summaryState(source, "with-ops").state, "operations");
  // The distinction the whole census turns on: `closed` says the census
  // *proved* the domain empty, which is a complete answer about an export that
  // genuinely does nothing. An empty `call` says nothing was determined.
  assert.equal(summaryState(source, "closed-nothing").state, "closed-empty");
  assert.equal(summaryState(source, "degenerate").state, "degenerate");
  // A reference the document does not carry cannot be read as a negative.
  assert.equal(summaryState(source, "missing").state, "degenerate");
});

test("only an owner requirement is an owner requirement", () => {
  const source = document();
  assert.equal(summaryState(source, "with-owner").owner, true);
  assert.equal(summaryState(source, "with-ops").owner, false);
  assert.equal(summaryState(source, "closed-nothing").owner, false);
});

test("a wildcard-reached entrypoint is not one a consumer can name", () => {
  // `policy2_artifact_acceptance_root` binds the entrypoint, so a summary at a
  // source path answers no bare-specifier import. Crediting these inflated the
  // 2026-09-15 census by a whole package.
  assert.equal(nameableEntrypoint("."), true);
  assert.equal(nameableEntrypoint("./immutable"), true);
  assert.equal(nameableEntrypoint("./client/spa"), true);
  assert.equal(nameableEntrypoint("./src/dom.ts"), false);
  assert.equal(nameableEntrypoint("./dist/index.js"), false);
  assert.equal(nameableEntrypoint("./types/client.d.ts"), false);
});

test("an export published at a wildcard path only is absent to its consumers", () => {
  const surfaces = surfacesFromDocuments([
    document({
      entrypoints: {
        "./src/dom.ts": { cases: [{ exports: { contains: "with-ops" } }] }
      }
    })
  ]);
  const rows = [{ package: "@example/utils", export: "contains", sites: 24 }];
  const { totals } = census(rows, surfaces);
  assert.equal(totals.operations, 0);
  assert.equal(totals.absent, 24);
});

test("the strongest statement at a nameable entrypoint is the one a consumer gets", () => {
  const surfaces = surfacesFromDocuments([
    document({
      entrypoints: {
        ".": { cases: [{ exports: { access: "degenerate" } }] },
        "./immutable": { cases: [{ exports: { access: "with-ops" } }] }
      }
    })
  ]);
  const rows = [{ package: "@example/utils", export: "access", sites: 10 }];
  const { totals } = census(rows, surfaces);
  assert.equal(totals.operations, 10);
  assert.equal(totals.degenerate, 0);
});

test("a package no catalog published is unmeasured, not absent", () => {
  // `absent` is a verdict about a package whose catalog is present and does not
  // carry the name. Collapsing the two would let a run that certified *nothing*
  // report a perfect zero-absent census.
  const rows = [{ package: "@nobody/here", export: "thing", sites: 7 }];
  const { totals, packages } = census(rows, new Map());
  assert.equal(totals.unmeasured, 7);
  assert.equal(totals.absent, 0);
  assert.equal(packages[0].measured, false);
});

test("every site is counted exactly once", () => {
  const surfaces = surfacesFromDocuments([
    document({
      entrypoints: {
        ".": {
          cases: [
            {
              exports: {
                access: "with-ops",
                noop: "closed-nothing",
                mystery: "degenerate"
              }
            }
          ]
        }
      }
    })
  ]);
  const rows = [
    { package: "@example/utils", export: "access", sites: 5 },
    { package: "@example/utils", export: "noop", sites: 3 },
    { package: "@example/utils", export: "mystery", sites: 2 },
    { package: "@example/utils", export: "gone", sites: 1 }
  ];
  const { totals } = census(rows, surfaces);
  const measured = totals.sites - totals.unmeasured;
  assert.equal(
    totals.operations + totals["closed-empty"] + totals.degenerate + totals.absent,
    measured,
    "double counting here is exactly how the first draft reported 63.8% coverage"
  );
  assert.equal(totals.operations, 5);
  assert.equal(totals["closed-empty"], 3);
  assert.equal(totals.degenerate, 2);
  assert.equal(totals.absent, 1);
});

test("the pin fails on a regression in either direction", () => {
  const pinned = {
    totals: { operations: 578, ownerRequirement: 22, degenerate: 1115, absent: 119, unmeasured: 146 }
  };
  assert.deepEqual(compare(pinned, { totals: { ...pinned.totals } }), []);
  // Fewer statements is a regression; so is more of anything unknown.
  assert.equal(
    compare(pinned, { totals: { ...pinned.totals, operations: 500 } }).length,
    1
  );
  assert.equal(compare(pinned, { totals: { ...pinned.totals, absent: 200 } }).length, 1);
  assert.equal(
    compare(pinned, { totals: { ...pinned.totals, unmeasured: 300 } }).length,
    1
  );
  // Improvement is never a failure: a newly certified entrypoint should move
  // these, and a gate that refuses every movement is one nobody re-pins.
  assert.deepEqual(
    compare(pinned, { totals: { ...pinned.totals, operations: 700, degenerate: 900 } }),
    []
  );
});
