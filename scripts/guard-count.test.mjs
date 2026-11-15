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

// ADR 0995: a row that left a SITE is a fall, even when the fixture's total holds.
const at = (path, blocker = "function-run-unplaced") => ({ code: "SC1001", kind: "uncertifiable", blocker, path });
const sites = (...rows) => ({ findings: rows });
// `was` and `now` are the fixtures the fall is measured over; `baseOnly` and `nowOnly` are the proof's fixtures (read, not measured).
const siteRun = (ledger, base, { was, now, baseOnly = {}, nowOnly = {}, computed = { [K]: 1 } }) =>
  checkLedger({
    ledger, base, computed, decisions, baseSnapshots: new Map(Object.entries(was)),
    read: (path) => files[path] ?? null,
    snapshotOf: (fixture) => ({ ...now, ...nowOnly })[fixture] ?? null,
    baseSnapshotOf: (fixture) => ({ ...was, ...baseOnly })[fixture] ?? null,
  });

test("a row that moved to another path is a fall, though the fixture's total holds", () => {
  // The laundering: the uncertifiable row at a.tsx (which a proof would have to clear) is gone from there and a new one stands at b.tsx.
  // The fixture's count is 1 -> 1, so count.mjs saw nothing before ADR 0995.
  const was = { "reactive-ir/m": sites(at("a.tsx")) };
  const now = { "reactive-ir/m": sites(at("b.tsx")) };
  const failures = siteRun({ inRepo: { [K]: 1 } }, { inRepo: { [K]: 1 } }, { was, now });
  assert.equal(failures.length, 1);
  assert.match(failures[0], /^reactive-ir\/m SC1001 function-run-unplaced at a\.tsx: 1 -> 0, no drops row/);
  // Recorded as a move: an ADR, the path it moved to, and no proof (it is no clean).
  const moved = { fixture: "reactive-ir/m", key: K, path: "a.tsx", from: 1, to: 0, adr: "ADR 0812", kind: "moved", movedTo: "b.tsx" };
  assert.deepEqual(siteRun({ inRepo: { [K]: 1 }, drops: [moved] }, { inRepo: { [K]: 1 } }, { was, now }), []);
  assert.match(siteRun({ inRepo: { [K]: 1 }, drops: [{ ...moved, movedTo: "c.tsx" }] }, { inRepo: { [K]: 1 } }, { was, now })[0], /movedTo c\.tsx gained 0/);
  assert.match(siteRun({ inRepo: { [K]: 1 }, drops: [{ ...moved, movedTo: "a.tsx" }] }, { inRepo: { [K]: 1 } }, { was, now })[0], /movedTo a\.tsx/);
  assert.match(siteRun({ inRepo: { [K]: 1 }, drops: [{ ...moved, proof: "rust/x.rs::proves_y" }] }, { inRepo: { [K]: 1 } }, { was, now })[0], /names no proof/);
  assert.match(siteRun({ inRepo: { [K]: 1 }, drops: [{ ...moved, adr: "ADR 0999" }] }, { inRepo: { [K]: 1 } }, { was, now })[0], /ADR 0999 is not in docs/);
  // A moved row is no kind of fixture-level drop.
  assert.match(run({ inRepo: { [K]: 2 }, drops: [{ ...drop, kind: "moved" }] }, { inRepo: { [K]: 3 } }, { [K]: 2 })[0], /moved|names no proof/);
});

const proofFixtures = { proved: "reactive-ir/proved", open: "reactive-ir/open" };
const rowsOf = (path, times) => Array.from({ length: times }, () => at(path));

test("a path that lost rows while its fixture's total fell by less is not covered by the fixture-level row", () => {
  // a.tsx 2 -> 0 and b.tsx 0 -> 1: the total fell 2 -> 1. A fixture-level row for 2 -> 1 would, before ADR 0995, have recorded the whole
  // thing as one clean; one of the rows only moved.
  const was = { "reactive-ir/m": sites(...rowsOf("a.tsx", 2)) };
  const now = { "reactive-ir/m": sites(at("b.tsx")) };
  const baseOnly = { "reactive-ir/proved": sites(...rowsOf("a.tsx", 2)), "reactive-ir/open": sites(at("o.tsx")) };
  const nowOnly = { "reactive-ir/proved": sites(at("a.tsx")), "reactive-ir/open": sites(at("o.tsx")) };
  const total = { fixture: "reactive-ir/m", key: K, from: 2, to: 1, adr: "ADR 0812", proof: "rust/x.rs::proves_y", fixtures: proofFixtures };
  const failures = siteRun({ inRepo: { [K]: 3 }, drops: [total] }, { inRepo: { [K]: 3 } }, { was, now, baseOnly, nowOnly, computed: { [K]: 3 } });
  assert.equal(failures.length, 1);
  assert.match(failures[0], /at a\.tsx: 2 -> 0, no drops row: the fixture's total fell by less than its paths lost/);
});

test("when the total fell by exactly what the paths lost, the fixture-level row answers them", () => {
  const was = { "reactive-ir/m": sites(at("a.tsx"), at("b.tsx")) };
  const now = { "reactive-ir/m": sites(at("b.tsx")) };
  const baseOnly = { "reactive-ir/proved": sites(at("a.tsx")), "reactive-ir/open": sites(at("o.tsx")) };
  const nowOnly = { "reactive-ir/proved": sites(), "reactive-ir/open": sites(at("o.tsx")) };
  const total = { fixture: "reactive-ir/m", key: K, from: 2, to: 1, adr: "ADR 0812", proof: "rust/x.rs::proves_y", fixtures: proofFixtures };
  assert.deepEqual(siteRun({ inRepo: { [K]: 2 }, drops: [total] }, { inRepo: { [K]: 3 } }, { was, now, baseOnly, nowOnly, computed: { [K]: 2 } }), []);
});

test("a row at a path is proved at that path, not by the fixture's total", () => {
  // `proved` lost its row at a.tsx only by gaining one at z.tsx: its total holds, so nothing was proved. And a proved fixture whose total
  // fell elsewhere (c.tsx) while a.tsx held proves nothing at a.tsx.
  const was = { "reactive-ir/m": sites(at("a.tsx")) };
  const now = { "reactive-ir/m": sites(at("b.tsx")) };
  const baseOnly = { "reactive-ir/proved": sites(at("a.tsx")), "reactive-ir/open": sites(at("o.tsx")) };
  const rowAt = { fixture: "reactive-ir/m", key: K, path: "a.tsx", from: 1, to: 0, adr: "ADR 0812", proof: "rust/x.rs::proves_y", fixtures: proofFixtures };
  const nowOnly = { "reactive-ir/proved": sites(at("z.tsx")), "reactive-ir/open": sites(at("o.tsx")) };
  const failures = siteRun({ inRepo: { [K]: 1 }, drops: [rowAt] }, { inRepo: { [K]: 1 } }, { was, now, baseOnly, nowOnly });
  assert.equal(failures.length, 1);
  assert.match(failures[0], /fixtures\.proved reactive-ir\/proved is no snapshot that held a SC1001 function-run-unplaced row at a\.tsx at the base and holds fewer now/);
  const elsewhere = { "reactive-ir/proved": sites(at("a.tsx")), "reactive-ir/open": sites(at("o.tsx")) };
  assert.match(siteRun({ inRepo: { [K]: 1 }, drops: [rowAt] }, { inRepo: { [K]: 1 } }, { was, now, baseOnly: { ...baseOnly, "reactive-ir/proved": sites(at("a.tsx"), at("c.tsx")) }, nowOnly: elsewhere })[0], /at a\.tsx at the base and holds fewer now/);
  // Clearing it at the path makes the row hold.
  const cleared = { ...nowOnly, "reactive-ir/proved": sites() };
  assert.deepEqual(siteRun({ inRepo: { [K]: 1 }, drops: [rowAt] }, { inRepo: { [K]: 1 } }, { was, now, baseOnly, nowOnly: cleared }), []);
});

test("a deleted fixture's paths fall with its total: the fixture-level row answers, none is asked per path", () => {
  const was = { "reactive-ir/m": sites(at("a.tsx"), at("b.tsx")) };
  const failures = siteRun({ inRepo: {} }, { inRepo: { [K]: 2 } }, { was, now: {}, computed: {} });
  assert.equal(failures.filter((failure) => / at /.test(failure)).length, 0);
  assert.match(failures[0], /reactive-ir\/m SC1001 function-run-unplaced: 2 -> 0, no drops row/);
});
