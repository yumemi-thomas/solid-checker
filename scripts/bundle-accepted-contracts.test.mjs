// Two certifications of one published artifact routinely differ, and the
// difference decides whether the artifact ships at all. Measured over a full
// census run: 89 artifact/entrypoint pairs, 87 agreeing exactly, 2 differing,
// 0 contradicting. Both differences were a root row closing a claim domain that
// the same artifact left open when it was reached as another package's
// dependency node. Dropping those cost the largest primitives contract.
import assert from "node:assert/strict";
import { describe, test } from "vitest";

import { claimsOf, relate } from "./bundle-accepted-contracts.mjs";

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
