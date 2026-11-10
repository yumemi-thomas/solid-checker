import assert from "node:assert/strict";
import { test } from "vitest";

import { checkRises, measureRises, names, rowsFor } from "./guard/rises.mjs";

// ADR 0955: the per-fact rises of the uncertifiable count, on in-memory snapshots.
const snapshot = (...rows) => ({ findings: rows.map(([code, blocker, kind = "uncertifiable"]) => ({ code, blocker, kind })) });
const u = (code, blocker, times) => Array.from({ length: times }, () => [code, blocker]);
const decisions = '<a id="adr-0955"></a>\n## ADR 0955 — x\n';

test("a rise is a count that went up on a fixture the base has, per code and blocker", () => {
  const base = new Map([
    ["reactive-ir/a", snapshot(...u("SC9012", "standard-member-impostor", 2), ...u("SC1001", "function-run-unplaced", 3))],
    ["reactive-ir/b", snapshot(...u("SC1001", "function-run-unplaced", 1))],
  ]);
  const now = new Map([
    ["reactive-ir/a", snapshot(...u("SC9012", "standard-member-impostor", 5), ...u("SC1001", "function-run-unplaced", 1), ["SC2001", "unplaced-write"])],
    ["reactive-ir/b", snapshot(...u("SC1001", "function-run-unplaced", 1))],
    // a fixture the base lacks has no rise: it is new, not raised
    ["reactive-ir/c", snapshot(...u("SC9012", "standard-member-impostor", 9))],
  ]);
  assert.deepEqual(measureRises(base, now), [
    { fixture: "reactive-ir/a", key: "SC2001 unplaced-write", from: 0, to: 1 },
    { fixture: "reactive-ir/a", key: "SC9012 standard-member-impostor", from: 2, to: 5 },
  ]);
  // a violation row is no uncertifiable row
  assert.deepEqual(measureRises(new Map([["f", snapshot()]]), new Map([["f", snapshot(["SC2001", "x", "violation"])]])), []);
});

const rise = { fixture: "reactive-ir/a", key: "SC9012 standard-member-impostor", from: 2, to: 5 };
const row = { ...rise, adr: "ADR 0955", fact: "handed-root-patch [OpenMemberWrite(*), StandardRootHanded]" };

test("a rise a row names is named, any other is unnamed and no failure", () => {
  assert.equal(names(row, rise), true);
  assert.equal(names(row, { ...rise, to: 6 }), false);
  const result = checkRises({ ledger: { rises: [row] }, base: { rises: [] }, measured: [rise, { ...rise, key: "SC1001 other", from: 0, to: 1 }], decisions });
  assert.deepEqual(result.failures, []);
  assert.deepEqual(result.named, [rise]);
  assert.deepEqual(result.unnamed, [{ ...rise, key: "SC1001 other", from: 0, to: 1 }]);
  // nothing measured, nothing named: still no failure
  assert.deepEqual(checkRises({ ledger: { rises: [] }, base: null, measured: [rise], decisions }).failures, []);
});

test("the list is append-only against the base", () => {
  const dropped = checkRises({ ledger: { rises: [] }, base: { rises: [row] }, measured: [], decisions });
  assert.match(dropped.failures[0], /append-only: a row at the base is gone or changed \(reactive-ir\/a SC9012 standard-member-impostor 2 -> 5\)/);
  const changed = checkRises({ ledger: { rises: [{ ...row, fact: "another" }] }, base: { rises: [row] }, measured: [], decisions });
  assert.equal(changed.failures.length, 1);
  assert.deepEqual(checkRises({ ledger: { rises: [row, { ...row, from: 5, to: 7 }] }, base: { rises: [row] }, measured: [], decisions }).failures, []);
});

test("a row is well formed: text, a rise, an ADR the decisions hold", () => {
  const failures = (bad) => checkRises({ ledger: { rises: [bad] }, base: null, measured: [], decisions }).failures;
  assert.match(failures({ ...row, fact: " " })[0], /fact is empty/);
  assert.match(failures({ ...row, fixture: undefined })[0], /fixture is empty/);
  assert.match(failures({ ...row, to: 2 })[0], /to is above from/);
  assert.match(failures({ ...row, from: -1 })[0], /to is above from/);
  assert.match(failures({ ...row, adr: "ADR 0999" })[0], /ADR 0999 is not in docs\/decisions\.md/);
  assert.match(failures({ ...row, adr: undefined })[0], /no ADR is not in docs\/decisions\.md/);
  assert.deepEqual(failures(row), []);
});

test("--append names every unnamed rise with one ADR and one fact", () => {
  const rows = rowsFor([rise], { adr: "ADR 0955", fact: "the fact", why: "why" });
  assert.deepEqual(rows, [{ ...rise, adr: "ADR 0955", fact: "the fact", why: "why" }]);
  assert.deepEqual(rowsFor([rise], { adr: "ADR 0955", fact: "the fact" }), [{ ...rise, adr: "ADR 0955", fact: "the fact" }]);
});
