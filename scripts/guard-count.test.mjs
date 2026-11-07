import assert from "node:assert/strict";
import { test } from "vitest";

import { checkLedger, countRows } from "./guard/count.mjs";

// The one-way uncertifiable count (scripts/guard/count.mjs), on in-memory ledgers.
const row = (code, blocker) => ({ code, kind: "uncertifiable", blocker });
const K = "SC1001 function-run-unplaced";
const decisions = '<a id="adr-0812"></a>\n## ADR 0812';
const files = { "rust/x.rs": "fn proves_y() {}\nconst NOTE: &str = \"mentions_z\";" };
const snaps = { "reactive-ir/proved": { findings: [] }, "reactive-ir/open": { findings: [row("SC1001", "function-run-unplaced")] } };
// The base's snapshots: `proved` held the key there and holds fewer now.
const baseSnaps = { "reactive-ir/proved": { findings: [row("SC1001", "function-run-unplaced")] }, "reactive-ir/open": { findings: [row("SC1001", "function-run-unplaced")] } };
const run = (ledger, base, computed, { current = snaps, baseSnapshots = null } = {}) =>
  checkLedger({
    ledger, base, computed, decisions, baseSnapshots,
    read: (path) => files[path] ?? null,
    snapshotOf: (fixture) => current[fixture] ?? null,
    baseSnapshotOf: (fixture) => baseSnaps[fixture] ?? null,
  });
const drop = { key: K, from: 3, to: 2, adr: "ADR 0812", proof: "rust/x.rs::proves_y", fixtures: { proved: "reactive-ir/proved", open: "reactive-ir/open" } };

test("rows are counted by code and blocker, uncertifiable only", () => {
  assert.deepEqual(countRows([{ findings: [row("SC1001", "function-run-unplaced"), row("SC1001", "function-run-unplaced"), { code: "SC1001", kind: "violation" }] }]), { [K]: 2 });
});

test("a rise passes once recorded, and fails unrecorded", () => {
  assert.deepEqual(run({ inRepo: { [K]: 4 } }, { inRepo: { [K]: 3 } }, { [K]: 4 }), []);
  assert.equal(run({ inRepo: { [K]: 3 } }, { inRepo: { [K]: 3 } }, { [K]: 4 }).length, 1);
});

test("a fall needs a drops row with every field holding", () => {
  assert.match(run({ inRepo: { [K]: 2 } }, { inRepo: { [K]: 3 } }, { [K]: 2 })[0], /no drops row/);
  assert.deepEqual(run({ inRepo: { [K]: 2 }, drops: [drop] }, { inRepo: { [K]: 3 } }, { [K]: 2 }), []);
  // The rules of the stricter gate (the W carrier brief, job 2): the proof is a symbol the file declares, not one it mentions in a
  // string; `proved` held the key at the base and holds fewer now (`open` holds as many as at the base, so it is no proved side).
  for (const broken of [{ adr: "ADR 0999" }, { proof: "rust/x.rs::missing" }, { proof: "rust/none.rs::proves_y" }, { proof: "rust/x.rs::mentions_z" }, { proof: "rust/x.rs::proves_y::extra" }, { fixtures: { proved: "reactive-ir/open", open: "reactive-ir/open" } }, { fixtures: { proved: "reactive-ir/proved", open: "reactive-ir/proved" } }]) {
    assert.equal(run({ inRepo: { [K]: 2 }, drops: [{ ...drop, ...broken }] }, { inRepo: { [K]: 3 } }, { [K]: 2 }).length, 1, JSON.stringify(broken));
  }
});

test("a fall is counted per fixture: a rise in another fixture does not net it", () => {
  const two = { findings: [row("SC1001", "function-run-unplaced"), row("SC1001", "function-run-unplaced")] };
  const one = { findings: [row("SC1001", "function-run-unplaced")] };
  // At the base `a` held 2 and `b` 1; now `a` holds 1 and `b` 2: the same total, one fall in `a`.
  const baseSnapshots = new Map([["reactive-ir/a", two], ["reactive-ir/b", one]]);
  const current = { ...snaps, "reactive-ir/a": one, "reactive-ir/b": two };
  const failures = run({ inRepo: { [K]: 3 } }, { inRepo: { [K]: 3 } }, { [K]: 3 }, { current, baseSnapshots });
  assert.equal(failures.length, 1);
  assert.match(failures[0], /^reactive-ir\/a SC1001 function-run-unplaced: 2 -> 1, no drops row/);
  // A drops row naming the fixture answers it; one without the fixture does not.
  const fixed = { ...drop, fixture: "reactive-ir/a", from: 2, to: 1 };
  assert.deepEqual(run({ inRepo: { [K]: 3 }, drops: [fixed] }, { inRepo: { [K]: 3 } }, { [K]: 3 }, { current, baseSnapshots }), []);
  assert.equal(run({ inRepo: { [K]: 3 }, drops: [{ ...fixed, fixture: undefined }] }, { inRepo: { [K]: 3 } }, { [K]: 3 }, { current, baseSnapshots }).length, 1);
});

test("a relabel records no proof, and its target gains what the key lost, in that fixture", () => {
  const J = "SC1001 read-backing-unproven";
  const relabel = { fixture: "reactive-ir/open", key: K, from: 1, to: 0, adr: "ADR 0812", kind: "relabel", relabeledTo: J };
  const moved = { "reactive-ir/open": { findings: [row("SC1001", "read-backing-unproven")] }, "reactive-ir/proved": baseSnaps["reactive-ir/proved"] };
  const check = (drops, current = moved) => run({ inRepo: { [K]: 1, [J]: 1 }, drops }, { inRepo: { [K]: 2 } }, { [K]: 1, [J]: 1 }, { current, baseSnapshots: Object.entries(baseSnaps) });
  assert.deepEqual(check([relabel]), []);
  assert.match(check([{ ...relabel, proof: "rust/x.rs::proves_y" }])[0], /names no proof/);
  assert.match(check([{ ...relabel, fixtures: drop.fixtures }])[0], /names no proof/);
  assert.match(check([{ ...relabel, relabeledTo: "SC1001 other" }])[0], /gained 0/);
  assert.match(check([relabel], { "reactive-ir/open": { findings: [] }, "reactive-ir/proved": baseSnaps["reactive-ir/proved"] })[0], /gained 0/);
  assert.match(check([{ ...relabel, kind: "other" }])[0], /not a kind/);
});

test("a key that goes is a fall", () => {
  assert.match(run({ inRepo: {} }, { inRepo: { [K]: 3 } }, {})[0], /3 -> 0, no drops row/);
});

test("editing the ledger in the same commit does not hide a fall: the base decides", () => {
  assert.equal(run({ inRepo: { [K]: 2 } }, { inRepo: { [K]: 3 } }, { [K]: 2 }).length, 1);
});

test("drops and renames are append-only", () => {
  assert.match(run({ inRepo: { [K]: 2 }, drops: [] }, { inRepo: { [K]: 2 }, drops: [drop] }, { [K]: 2 })[0], /append-only/);
});

test("a rename keeps the count, with an ADR", () => {
  const J = "SC1001 read-backing-unproven";
  assert.deepEqual(run({ inRepo: { [J]: 3 }, renames: [{ from: K, to: J, adr: "ADR 0812" }] }, { inRepo: { [K]: 3 } }, { [J]: 3 }), []);
  assert.equal(run({ inRepo: { [J]: 2 }, renames: [{ from: K, to: J, adr: "ADR 0812" }] }, { inRepo: { [K]: 3 } }, { [J]: 2 }).length, 2);
});

test("with no ledger at the base only the exact match is checked", () => {
  assert.deepEqual(run({ inRepo: { [K]: 1 } }, null, { [K]: 1 }), []);
});
