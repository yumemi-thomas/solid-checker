import assert from "node:assert/strict";
import { test } from "vitest";

import {
  census,
  censusTarget,
  compare,
  consumerState,
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

// Which denominator a pin is allowed to claim. Two things disqualify a run, and
// both are about the run answering a different question with the same buckets:
// more than one Solid target has no single denominator, and a requested
// export-condition set changes which *bytes* every row was certified about.
//
// The second is not hypothetical. `--conditions node` makes
// `@solid-primitives/platform`'s probe gates complete -- the default set
// resolves `@solidjs/web` to its client build, whose `window` the Node probe
// realm has not got -- so a conditioned run reports coverage the default one
// cannot reach, for an artifact the default one does not certify. A pin that
// took those numbers would read a change of artifact as a change of coverage.
test("a pin is refused for a run with no single denominator", () => {
  const exits = [];
  const original = process.exit;
  const errors = [];
  const originalError = console.error;
  process.exit = code => {
    exits.push(code);
    throw new Error("exited");
  };
  console.error = message => errors.push(message);
  const attempt = run => {
    try {
      return censusTarget(run);
    } catch (error) {
      if (error.message !== "exited") throw error;
      return null;
    }
  };
  try {
    assert.equal(attempt({ scope: { solidTargets: ["2"], conditions: [] } }), "solid2");
    assert.deepEqual(exits, []);

    assert.equal(attempt({ scope: { solidTargets: [], conditions: [] } }), null);
    assert.match(errors.at(-1), /restrict the run with a single --solid/);

    assert.equal(attempt({ scope: { solidTargets: ["2"], conditions: ["node"] } }), null);
    assert.match(errors.at(-1), /requested export conditions \[node\]/);
    assert.match(errors.at(-1), /re-run without --conditions to pin/);

    // A run that predates the field is not a conditioned run.
    assert.equal(attempt({ scope: { solidTargets: ["2"] } }), "solid2");
  } finally {
    process.exit = original;
    console.error = originalError;
  }
});

// The consumer view. The census buckets say what a summary *states*; these say
// what an import of it still finds open, by the analyzer's rule
// (`push_unknown_contract_claims`): an open `reads` or `creates` raises SC9005
// wherever the name is imported, an open `returns` or `callbacks` only on some
// uses, and a proven non-callable value has no call-path domain left open.
function consumerDocument(entrypoints = {}) {
  return {
    package: { name: "@example/utils" },
    summaries: {
      constant: { shape: "plain" },
      store: { shape: { kind: "store" } },
      "closed-plain-choice": {
        shape: { kind: "choice", closed: ["alternatives"], alternatives: ["plain"] }
      },
      "open-choice": { shape: { kind: "choice", alternatives: ["plain"] } },
      "callable-choice": {
        shape: { kind: "choice", closed: ["alternatives"], alternatives: ["plain", "callable"] }
      },
      "all-closed": {
        shape: "callable",
        call: { closed: ["callbacks", "reads", "returns", "creates"], callbacks: [], reads: [] }
      },
      "returns-open": {
        shape: "callable",
        call: { closed: ["callbacks", "reads", "creates"], callbacks: [], reads: [] }
      },
      "reads-only": { shape: "callable", call: { closed: ["reads"], reads: [] } },
      degenerate: { shape: "callable", call: {} },
      shapeless: { call: { closed: ["callbacks", "reads", "returns", "creates"] } }
    },
    entrypoints
  };
}

test("a consumer's view is what an import still finds open", () => {
  const source = consumerDocument();
  // `shape_may_be_callable`: every shape but `callable`, `component`,
  // `unknown` and a choice that may hold one is proven never invoked, so
  // `project_export_semantics` closes all its call-path domains.
  assert.equal(consumerState(source, "constant"), "value");
  assert.equal(consumerState(source, "store"), "value");
  assert.equal(consumerState(source, "closed-plain-choice"), "value");
  assert.equal(consumerState(source, "open-choice"), "every-import");
  assert.equal(consumerState(source, "callable-choice"), "every-import");
  assert.equal(consumerState(source, "all-closed"), "clean");
  assert.equal(consumerState(source, "returns-open"), "some-uses");
  // The disagreement this view exists for: one closed domain is a determined
  // negative to the census and an import with `creates` still open here.
  assert.equal(summaryState(source, "reads-only").state, "closed-empty");
  assert.equal(consumerState(source, "reads-only"), "every-import");
  assert.equal(consumerState(source, "degenerate"), "every-import");
  // Nothing unstated clears an import. A summary with no shape is taken as
  // callable, so only its own closed domains can clear it, and a reference the
  // document does not carry clears nothing.
  assert.equal(consumerState(source, "shapeless"), "clean");
  const shapelessAndOpen = { ...source, summaries: { shapeless: { call: {} } } };
  assert.equal(consumerState(shapelessAndOpen, "shapeless"), "every-import");
  assert.equal(consumerState(source, "missing"), "every-import");
});

test("the consumer view counts every in-surface site once, and nothing else", () => {
  const surfaces = surfacesFromDocuments([
    consumerDocument({
      ".": {
        cases: [
          {
            exports: {
              isServer: "constant",
              pick: "reads-only",
              map: "returns-open",
              mystery: "degenerate"
            }
          }
        ]
      },
      "./src/hidden.ts": { cases: [{ exports: { hidden: "all-closed" } }] }
    })
  ]);
  const rows = [
    { package: "@example/utils", export: "isServer", sites: 4 },
    { package: "@example/utils", export: "pick", sites: 3 },
    { package: "@example/utils", export: "map", sites: 2 },
    { package: "@example/utils", export: "mystery", sites: 1 },
    { package: "@example/utils", export: "hidden", sites: 5 },
    { package: "@nobody/here", export: "thing", sites: 7 }
  ];
  const { totals, packages } = census(rows, surfaces);
  const inSurface = totals.operations + totals["closed-empty"] + totals.degenerate;
  assert.equal(
    Object.values(totals.consumer).reduce((sum, sites) => sum + sites, 0),
    inSurface
  );
  // Both directions of disagreement at once: `isServer` is degenerate to the
  // census and finds nothing open, `pick` is determined and finds `creates` open.
  assert.equal(totals.degenerate, 5);
  assert.equal(totals["closed-empty"], 5);
  assert.deepEqual(totals.consumer, { value: 4, clean: 0, "some-uses": 2, "every-import": 4 });
  // A wildcard-only export and an unmeasured package stay in their own buckets.
  assert.equal(totals.absent, 5);
  assert.equal(totals.unmeasured, 7);
  assert.deepEqual(
    packages.find(row => row.package === "@example/utils").consumer,
    totals.consumer
  );
});

test("the consumer view keeps the best answer at an entrypoint a consumer can name", () => {
  const surfaces = surfacesFromDocuments([
    consumerDocument({
      ".": { cases: [{ exports: { access: "degenerate" } }] },
      "./immutable": { cases: [{ exports: { access: "returns-open" } }] },
      "./src/access.ts": { cases: [{ exports: { access: "all-closed" } }] }
    })
  ]);
  const { totals } = census([{ package: "@example/utils", export: "access", sites: 6 }], surfaces);
  // The wildcard-reached `all-closed` answers no import anybody writes.
  assert.deepEqual(totals.consumer, { value: 0, clean: 0, "some-uses": 6, "every-import": 0 });
});

test("the consumer view is gated from the first pin that carries it", () => {
  const buckets = {
    operations: 440,
    ownerRequirement: 34,
    degenerate: 127,
    absent: 722,
    unmeasured: 84
  };
  const view = { value: 158, clean: 0, "some-uses": 439, "every-import": 555 };
  const pinned = { totals: { ...buckets, consumer: view } };
  const withView = change => ({ totals: { ...buckets, consumer: { ...view, ...change } } });
  assert.deepEqual(compare(pinned, withView({})), []);
  // More sites raising SC9005 wherever imported is a regression, and so is a
  // site losing its nothing-open answer, whichever bucket it lands in.
  assert.equal(compare(pinned, withView({ "every-import": 556, "some-uses": 438 })).length, 1);
  assert.equal(compare(pinned, withView({ value: 157, "some-uses": 440 })).length, 1);
  assert.equal(compare(pinned, withView({ clean: 0, value: 150, "every-import": 563 })).length, 2);
  // `some-uses` has no direction, and trading `value` for `clean` finds nothing
  // more open.
  assert.deepEqual(compare(pinned, withView({ "every-import": 500, "some-uses": 494 })), []);
  assert.deepEqual(compare(pinned, withView({ value: 150, clean: 8 })), []);
  // A pin written before the view existed is compared on the census buckets
  // alone: reading its silence as zero would fail every run on `every-import`.
  assert.deepEqual(compare({ totals: buckets }, withView({})), []);
  const worseBuckets = { totals: { ...buckets, degenerate: 128, consumer: view } };
  assert.equal(compare({ totals: buckets }, worseBuckets).length, 1);
});
