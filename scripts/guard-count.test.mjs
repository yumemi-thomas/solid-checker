import assert from "node:assert/strict";
import { test } from "vitest";

import { checkLedger, countRows } from "./guard/count.mjs";

// The one-way uncertifiable count (scripts/guard/count.mjs), on in-memory ledgers.
const row = (code, blocker) => ({ code, kind: "uncertifiable", blocker });
const K = "SC1001 function-run-unplaced";
const decisions = '<a id="adr-0812"></a>\n## ADR 0812';
const files = { "rust/x.rs": "fn proves_y() {}" };
const snaps = { "reactive-ir/proved": { findings: [] }, "reactive-ir/open": { findings: [row("SC1001", "function-run-unplaced")] } };
const run = (ledger, base, computed) =>
  checkLedger({ ledger, base, computed, decisions, read: (path) => files[path] ?? null, snapshotOf: (fixture) => snaps[fixture] ?? null });
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
  for (const broken of [{ adr: "ADR 0999" }, { proof: "rust/x.rs::missing" }, { proof: "rust/none.rs::proves_y" }, { fixtures: { proved: "reactive-ir/open", open: "reactive-ir/open" } }, { fixtures: { proved: "reactive-ir/proved", open: "reactive-ir/proved" } }]) {
    assert.equal(run({ inRepo: { [K]: 2 }, drops: [{ ...drop, ...broken }] }, { inRepo: { [K]: 3 } }, { [K]: 2 }).length, 1, JSON.stringify(broken));
  }
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
