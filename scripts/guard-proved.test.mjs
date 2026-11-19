import assert from "node:assert/strict";
import { test } from "vitest";

import { changesProblems, ledgerProblems, ledgersOf, knownProblems, owedProblems } from "./guard/proved.mjs";

// ADR 1012: the proved-verdict ledger and its naming file are held against the base, on in-memory files.
const decisions = '<a id="adr-1012"></a>\n## ADR 1012 — x\n<a id="adr-0803"></a>\n## ADR 0803 — y\n';
const ids = Array.from({ length: 24 }, (_, i) => `mx-lost-${i}`);
const ledgers = ledgersOf([["mx-recall.json", JSON.stringify({ grid: "mx-recall", ids: [...ids].sort(), entries: [["k1:C:3", "mx-keyed"]] })]]);
// ADR 1118: a new row is a fix's, with its Chrome witness and its collateral.
const row = (over = {}) => ({ grid: "mx-recall", ids, adr: "ADR 1012", why: "the merge's twenty-four", kind: "fix", witness: "mx-lost-0", collateral: 0, ...over });

test("changes.json: a row of the base that is edited or removed fails (the naming file only grows)", () => {
  // The laundering: a loss is named once; a later change edits the old row to name more ids, or deletes it so the audit trail disappears.
  const base = { rows: [row()] };
  assert.deepEqual(changesProblems({ head: { rows: [row()] }, base, decisions, ledgers }), []);
  assert.match(changesProblems({ head: { rows: [] }, base, decisions, ledgers })[0], /append-only/);
  assert.match(changesProblems({ head: { rows: [row({ why: "other" })] }, base, decisions, ledgers })[0], /append-only/);
  assert.match(changesProblems({ head: { rows: [row({ ids: ids.slice(0, 23) })] }, base, decisions, ledgers })[0], /append-only/);
});

test("changes.json: a new row needs an ADR in docs/decisions.md, a why, and exact ids the ledger holds", () => {
  const empty = { rows: [] };
  assert.deepEqual(changesProblems({ head: { rows: [row()] }, base: empty, decisions, ledgers }), []);
  assert.match(changesProblems({ head: { rows: [row({ adr: "ADR 9999" })] }, base: empty, decisions, ledgers })[0], /ADR 9999 is not an ADR in docs\/decisions.md/);
  assert.match(changesProblems({ head: { rows: [row({ adr: undefined })] }, base: empty, decisions, ledgers })[0], /no adr is not an ADR/);
  assert.match(changesProblems({ head: { rows: [row({ why: " " })] }, base: empty, decisions, ledgers })[0], /says no why/);
  const [unknown] = changesProblems({ head: { rows: [row({ ids: ["mx-never-proved"] })] }, base: empty, decisions, ledgers });
  assert.match(unknown, /names 1 id\(s\) or key\(s\) the ledger does not hold/);
  assert.match(changesProblems({ head: { rows: [row({ ids: [] })] }, base: empty, decisions, ledgers })[0], /non-empty list of distinct names/);
  assert.match(changesProblems({ head: { rows: [row({ ids: [ids[0], ids[0]] })] }, base: empty, decisions, ledgers })[0], /distinct names/);
});

test("changes.json: a group row (the audit's) names a code, a proved from, an unproved to and a positive count, and not ids as well", () => {
  const empty = { rows: [] };
  const group = (over = {}) => ({ grid: "mx-recall", code: "STORE_WRITE_OUTSIDE_SETTER", from: "agree", to: "recall-gap", count: 24, adr: "ADR 1012", why: "x", kind: "fix", witness: "mx-lost-0", collateral: 0, ...over });
  assert.deepEqual(changesProblems({ head: { rows: [group()] }, base: empty, decisions, ledgers }), []);
  assert.match(changesProblems({ head: { rows: [group({ to: "agree" })] }, base: empty, decisions, ledgers })[0], /to must be an outcome that is not proved/);
  assert.match(changesProblems({ head: { rows: [group({ from: "recall-gap" })] }, base: empty, decisions, ledgers })[0], /from must be the grid's proved outcome \(agree\)/);
  assert.match(changesProblems({ head: { rows: [group({ count: 0 })] }, base: empty, decisions, ledgers })[0], /count must be a positive integer/);
  assert.match(changesProblems({ head: { rows: [group({ code: "" })] }, base: empty, decisions, ledgers })[0], /names no code/);
  assert.match(changesProblems({ head: { rows: [group({ ids })] }, base: empty, decisions, ledgers })[0], /not both and not neither/);
  const legal = group({ grid: "lg-recall", from: "clean", to: "uncertifiable" });
  assert.deepEqual(changesProblems({ head: { rows: [legal] }, base: empty, decisions, ledgers }), []);
  assert.match(changesProblems({ head: { rows: [{ ...legal, from: "agree" }] }, base: empty, decisions, ledgers })[0], /\(clean\)/);
});

test("ledgers: an id the base held and the head lost fails, whatever else the file gained", () => {
  // The laundering: a regenerated ledger drops the 24 ids a change lost, so the smoke compares nothing about them.
  const sorted = [...ids].sort();
  const base = ledgersOf([["mx-recall.json", JSON.stringify({ grid: "mx-recall", ids: sorted })]]);
  const head = ledgersOf([["mx-recall.json", JSON.stringify({ grid: "mx-recall", ids: [...sorted.slice(0, 20), "mx-new"] })]]);
  const [problem] = ledgerProblems({ head, base });
  assert.match(problem, /mx-recall lost 4 id\(s\)/);
  assert.deepEqual(ledgerProblems({ head: ledgersOf([["mx-recall.json", JSON.stringify({ grid: "mx-recall", ids: [...sorted, "mx-new"] })]]), base }), []);
  assert.match(ledgerProblems({ head: new Map(), base })[0], /is gone/);
  assert.deepEqual(ledgerProblems({ head, base: new Map() }), []);
  assert.deepEqual([...ledgersOf([["changes.json", "{}"], ["README.md", "x"]]).keys()], []);
});

test("changes.json (ADR 1118): a proof change appends no row; a fix's row names its witness and its collateral, and a key names an entry", () => {
  const empty = { rows: [] };
  assert.match(changesProblems({ head: { rows: [row({ kind: undefined })] }, base: empty, decisions, ledgers })[0], /is not a fix's \(kind: "fix"\): a proof change appends no row/);
  assert.match(changesProblems({ head: { rows: [row({ kind: "proof" })] }, base: empty, decisions, ledgers })[0], /a proof change appends no row/);
  assert.match(changesProblems({ head: { rows: [row({ witness: "" })] }, base: empty, decisions, ledgers })[0], /names no witness/);
  assert.match(changesProblems({ head: { rows: [row({ collateral: -1 })] }, base: empty, decisions, ledgers })[0], /collateral must be the count/);
  assert.match(changesProblems({ head: { rows: [row({ collateral: undefined })] }, base: empty, decisions, ledgers })[0], /collateral must be the count/);
  assert.deepEqual(changesProblems({ head: { rows: [row({ ids: undefined, keys: ["k1:C:3"] })] }, base: empty, decisions, ledgers }), []);
  assert.match(changesProblems({ head: { rows: [row({ ids: undefined, keys: ["k9:C:3"] })] }, base: empty, decisions, ledgers })[0], /names 1 id\(s\) or key\(s\) the ledger does not hold/);
  // a base row written before ADR 1118 stays valid as it is (append-only, never re-judged)
  const old = { grid: "mx-recall", ids, adr: "ADR 1012", why: "before 1118" };
  assert.deepEqual(changesProblems({ head: { rows: [old] }, base: { rows: [old] }, decisions, ledgers }), []);
});

test("owed.json (ADR 1118): a fix's collateral is owed work that only falls, and open-work names it", () => {
  const fix = row({ collateral: 5 });
  const work = "- ADR 1012: 5 true proofs owed after the fix\n";
  const check = (head, base, { changes = { rows: [fix] }, baseChanges = { rows: [] }, openWork = work } = {}) => owedProblems({ head: { rows: head }, base: { rows: base }, changes, baseChanges, decisions, openWork });
  const owed = (n, over = {}) => ({ adr: "ADR 1012", owed: n, why: "1012's collateral", ...over });
  assert.deepEqual(check([owed(5)], []), []);
  assert.match(check([owed(5)], [], { changes: { rows: [] } })[0], /a new entry for ADR 1012 with no new fix row/);
  assert.deepEqual(check([owed(3)], [owed(5)], { changes: { rows: [] } }), []);
  assert.match(check([owed(6)], [owed(5)], { changes: { rows: [] } })[0], /more than the base's 5: owed collateral only falls/);
  assert.match(check([], [owed(5)], { changes: { rows: [] } })[0], /is gone/);
  assert.match(check([owed(5)], [], { openWork: "nothing" })[0], /no line of docs\/open-work.md names it/);
  assert.deepEqual(check([owed(0)], [owed(5)], { changes: { rows: [] }, openWork: "" }), []);
  assert.match(check([owed(5, { adr: "ADR 9999" })], [])[0], /not an ADR/);
});

test("ledgers (ADR 1118): a key the base held stays an entry or retires; an unkeyed id may become an entry", () => {
  const file = (value) => ledgersOf([["mx-recall.json", JSON.stringify({ grid: "mx-recall", ...value })]]);
  const base = file({ entries: [["k1:C:3", "a"], ["k2:C:3", "b"]], ids: ["c"] });
  assert.deepEqual(ledgerProblems({ head: file({ entries: [["k1:C:3", "a"], ["k3:C:3", "c"]], retired: [["k2:C:3", "b"]] }), base }), []);
  assert.match(ledgerProblems({ head: file({ entries: [["k1:C:3", "a"]], ids: ["c"] }), base })[0], /lost 1 key\(s\) \(k2:C:3\)/);
  assert.match(ledgerProblems({ head: file({ entries: [["k1:C:3", "a"], ["k2:C:3", "b"]] }), base })[0], /lost 1 id\(s\) \(c\)/);
  const unsorted = [];
  ledgersOf([["mx-recall.json", JSON.stringify({ grid: "mx-recall", entries: [["k2:C:3", "b"], ["k1:C:3", "a"]] })]], unsorted);
  assert.match(unsorted[0], /not sorted by key/);
});

test("audit-known.json: only removing ids is free; a new entry, an added id, a changed idRegex or cap needs a new ADR and a why", () => {
  const entry = (over = {}) => ({ grid: "mx-recall", kind: "unplaced", item: "pk-lazy", idRegex: "-pk-lazy", cap: false, ids: { lean: ["a", "b"], full: ["a", "b", "c"] }, adr: "ADR 1012", why: "x", ...over });
  const check = (head, base) => knownProblems({ head: { entries: [head] }, base: { entries: base ? [base] : [] }, decisions });
  assert.deepEqual(check(entry(), null), []);
  assert.deepEqual(check(entry({ ids: { lean: ["a"], full: ["a", "c"] } }), entry()), []);
  assert.deepEqual(check(entry({ ids: { lean: [], full: [] } }), entry()), []);
  // an id added needs a new ADR: the same ADR the base named does not do, an unknown one does not either
  const added = entry({ ids: { lean: ["a", "b"], full: ["a", "b", "c", "d"] } });
  assert.match(check(added, entry())[0], /changed \(1 full id\(s\) added \(d\)\) under ADR 1012, the ADR the base already named: any change but removing ids needs a new ADR/);
  assert.deepEqual(check({ ...added, adr: "ADR 0803" }, entry()), []);
  assert.match(check({ ...added, adr: "ADR 9999" }, entry())[0], /not an ADR in docs\/decisions.md/);
  assert.match(check({ ...added, adr: "ADR 0803", why: "" }, entry())[0], /says no why/);
  // the swap: one id fixed, a new one of the class: the count is the same, the id set is not
  assert.match(check(entry({ ids: { lean: ["a", "b"], full: ["a", "b", "z"] } }), entry())[0], /1 full id\(s\) added \(z\)/);
  // widening the net or turning an exact entry into a cap needs one too
  assert.match(check(entry({ idRegex: "-pk-" }), entry())[0], /its idRegex changed/);
  assert.match(check(entry({ cap: true }), entry())[0], /its cap changed/);
  assert.deepEqual(check(entry({ cap: true, adr: "ADR 0803" }), entry()), []);
  // an entry's ids are sorted and distinct, both modes
  assert.match(check(entry({ ids: { lean: ["b", "a"], full: ["a"] } }), null)[0], /ids\.lean must be a sorted list of distinct ids/);
  assert.match(check(entry({ ids: { lean: ["a", "a"], full: ["a"] } }), null)[0], /ids\.lean must be a sorted list/);
  assert.match(check(entry({ ids: { lean: ["a"] } }), null)[0], /ids\.full must be a sorted list/);
  // another new entry (another key) is new
  assert.match(check(entry({ item: "other", adr: "ADR 9999" }), entry())[0], /not an ADR/);
});

test("ledger files: a file names its own grid and lists its ids sorted and distinct", () => {
  const problems = [];
  const good = ledgersOf([["mx-recall.json", JSON.stringify({ grid: "mx-recall", ids: ["a", "b"] })]], problems);
  assert.deepEqual(problems, []);
  assert.deepEqual([...good.get("mx-recall").ids], ["a", "b"]);
  const bad = [];
  ledgersOf([["mx-recall.json", JSON.stringify({ grid: "mx-census", ids: ["a"] })]], bad);
  assert.match(bad[0], /its grid is "mx-census", the file's is mx-recall/);
  const dup = [];
  ledgersOf([["mx-recall.json", JSON.stringify({ grid: "mx-recall", ids: ["a", "a"] })]], dup);
  assert.match(dup[0], /duplicate ids/);
  const unsorted = [];
  ledgersOf([["mx-recall.json", JSON.stringify({ grid: "mx-recall", ids: ["b", "a"] })]], unsorted);
  assert.match(unsorted[0], /ids are not sorted/);
});
