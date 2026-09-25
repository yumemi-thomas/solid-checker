// Two certifications of one published artifact routinely differ, and the
// difference decides whether the artifact ships at all. Measured over a full
// census run: 89 artifact/entrypoint pairs, 87 agreeing exactly, 2 differing,
// 0 contradicting. Both differences were a root row closing a claim domain that
// the same artifact left open when it was reached as another package's
// dependency node. Dropping those cost the largest primitives contract.
import assert from "node:assert/strict";
import { describe, test } from "vitest";

import { bundleKey, claimsOf, collectBundles, relate } from "./bundle-accepted-contracts.mjs";

const document = (summary) =>
  JSON.stringify({
    entrypoints: { ".": { cases: [{ exports: { access: "s1" } }] } },
    summaries: { s1: summary }
  });

const invoke = {
  callbacks: [{ from: { arg: 0, path: [] }, operation: "callback-0" }],
  operations: [{ id: "callback-0", kind: "invoke", tracking: "untracked" }]
};

const open = document({ shape: "callable", call: { ...invoke, closed: ["creates"], creates: [] } });
const closing = document({
  shape: "callable",
  call: { ...invoke, closed: ["reads", "creates"], creates: [], reads: [] }
});

describe("resolving two certifications of one artifact", () => {
  test("a document that closes a domain the other left open refines it", () => {
    assert.equal(relate(claimsOf(closing), claimsOf(open)), "refines");
    assert.equal(relate(claimsOf(open), claimsOf(closing)), "coarsens");
  });

  test("identical claims are the same however the bytes were ordered", () => {
    assert.equal(relate(claimsOf(open), claimsOf(open)), "same");
  });

  // The trap this guards: `reads: []` with `reads` closed and no `reads` key at
  // all are the same bytes once the empty array is dropped, and only `closed`
  // tells "proved there is nothing here" from "states nothing here". If the
  // body comparison swallowed that, an open document would read as `same` and
  // could win the deterministic digest tie-break over a closing one.
  test("an empty closed domain is not the same claim as an absent one", () => {
    assert.notEqual(relate(claimsOf(open), claimsOf(closing)), "same");
  });

  test("closing different domains is a conflict, not a merge", () => {
    const closesReads = document({
      shape: "callable",
      call: { ...invoke, closed: ["reads"], reads: [] }
    });
    const closesCreates = document({
      shape: "callable",
      call: { ...invoke, closed: ["creates"], creates: [] }
    });
    assert.equal(relate(claimsOf(closesReads), claimsOf(closesCreates)), "conflict");
  });

  test("a different operation is a conflict however the domains close", () => {
    const tracked = document({
      shape: "callable",
      call: {
        callbacks: invoke.callbacks,
        operations: [{ id: "callback-0", kind: "invoke", tracking: "tracked" }],
        closed: ["reads", "creates"],
        creates: [],
        reads: []
      }
    });
    assert.equal(relate(claimsOf(tracked), claimsOf(open)), "conflict");
  });

  test("an export one document does not carry is a conflict", () => {
    const extra = JSON.stringify({
      entrypoints: { ".": { cases: [{ exports: { access: "s1", chain: "s1" } }] } },
      summaries: { s1: { shape: "callable", call: { ...invoke, closed: ["creates"], creates: [] } } }
    });
    assert.equal(relate(claimsOf(extra), claimsOf(open)), "conflict");
  });
});

// The defect this key exists for, measured on the ecosystem corpus: the same
// `@solid-primitives/utils@7.0.0-next.4` bytes certified on a floor row
// (`@solidjs/signals@2.0.0-rc.0`, no audited rows) and a head row (rc.6,
// audited), and only the head rows close `createMicrotask` `creates`. Keyed by
// artifact alone, "keep the closing certification" shipped the head closure to
// rc.0 projects.
describe("certifications of one artifact in different environments", () => {
  const signals = version => [
    { name: "@solidjs/signals", version, integrity: `sha512-signals-${version}` }
  ];
  const entry = (environment, document, digest) => ({
    packageName: "@solid-primitives/utils",
    packageVersion: "7.0.0-next.4",
    packageIntegrity: "sha512-utils",
    specifier: "@solid-primitives/utils",
    requestedEntrypoint: ".",
    exportConditions: ["import"],
    document: `objects/${digest}.main.json`,
    documentDigest: `sha256:${digest}`,
    receipt: `objects/${digest}.receipt.json`,
    receiptDigest: `sha256:${digest}r`,
    dependencyEnvironment: environment,
    _document: document
  });
  const result = (...entries) => ({
    bundles: entries.map(({ _document, ...published }) => published),
    objects: Object.fromEntries(
      entries.flatMap(({ _document, document, receipt }) => [
        [document, _document],
        [receipt, "{}"]
      ])
    )
  });

  test("the environment is part of the key", () => {
    const floor = entry(signals("2.0.0-rc.0"), open, "a");
    const head = entry(signals("2.0.0-rc.6"), closing, "b");
    assert.notEqual(bundleKey(floor), bundleKey(head));
    assert.equal(bundleKey(floor), bundleKey(entry(signals("2.0.0-rc.0"), closing, "c")));
  });

  test("a floor and a head certification are two bundles, and neither refines the other", () => {
    const floor = entry(signals("2.0.0-rc.0"), open, "a");
    const head = entry(signals("2.0.0-rc.6"), closing, "b");
    for (const order of [[floor, head], [head, floor]]) {
      const { ordered, conflicted, refinements } = collectBundles([result(...order)]);
      assert.equal(ordered.length, 2, "one bundle per environment");
      assert.equal(conflicted.size, 0);
      assert.equal(refinements.size, 0, "a closure proven in one environment refines nothing in another");
      const byEnvironment = new Map(
        ordered.map(bundle => [bundle.dependencyEnvironment[0].version, bundle.documentDigest])
      );
      assert.equal(byEnvironment.get("2.0.0-rc.0"), "sha256:a", "the floor keeps its own, open, claim");
      assert.equal(byEnvironment.get("2.0.0-rc.6"), "sha256:b");
    }
  });

  test("within one environment the closing certification still refines", () => {
    const narrow = entry(signals("2.0.0-rc.6"), open, "a");
    const full = entry(signals("2.0.0-rc.6"), closing, "b");
    const { ordered, refinements } = collectBundles([result(narrow), result(full)]);
    assert.equal(ordered.length, 1);
    assert.equal(ordered[0].documentDigest, "sha256:b");
    assert.equal(refinements.size, 1);
  });

  test("an entry that states no environment is refused rather than keyed", () => {
    const unstated = entry(undefined, open, "a");
    assert.throws(() => collectBundles([result(unstated)]), /states no dependency environment/);
  });
});
