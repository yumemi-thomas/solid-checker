import assert from "node:assert/strict";
import { test } from "vitest";

import { judgeWitness, recordedCodes, vanishedRows } from "./flip/judge.mjs";

// The witness judge (scripts/flip/judge.mjs), on in-memory rows: no checker, no Chrome.
const fired = { diagnostics: ["STRICT_READ_UNTRACKED"], errors: [], logs: [] };
const quiet = { diagnostics: [], errors: [], logs: [] };
const certified = { status: "certified", findings: [] };
const finding = (id, rule, kind, line = 5, analysisContext, file = "App.tsx") => ({ id, rule, kind, primaryLocation: { path: `/w/x/${file}`, line, column: 3 }, analysisContext });

test("a row where Chrome fired fails when nothing covers the code, whatever its kind", () => {
  for (const kind of ["clean", "keep", "regress"]) {
    const verdict = judgeWitness({ rule: "SC1001", kind, runtime: fired, before: { findings: [] } }, certified);
    assert.equal(verdict.ok, false, kind);
    assert.match(verdict.why, /no finding covers it/);
  }
});

test("an SC9012 clean row does not pass on an uncertifiable that covers nothing", () => {
  const out = { status: "uncertifiable", findings: [finding("SC9012", "value-use-uncertain", "uncertifiable", 9, "value-use-unclassified")] };
  assert.equal(judgeWitness({ rule: "SC9012", kind: "clean", runtime: fired }, out).ok, false);
});

test("an SC9012 clean row passes only at its closedAt: an SC9012 at another line, or none pinned, says nothing about the site", () => {
  const row = { rule: "SC9012", kind: "clean", runtime: fired };
  const at = (line) => ({ status: "uncertifiable", findings: [finding("SC9012", "reactive-dispatch-unresolved", "uncertifiable", line, "value-in-carrier")] });
  assert.equal(judgeWitness(row, at(9)).ok, false);
  assert.equal(judgeWitness({ ...row, closedAt: { file: "App.tsx", line: 9, blocker: "value-in-carrier" } }, at(9)).ok, true);
  assert.equal(judgeWitness({ ...row, closedAt: { file: "App.tsx", line: 9, blocker: "value-in-carrier" } }, at(4)).ok, false);
  assert.equal(judgeWitness({ ...row, closedAt: { file: "App.tsx", line: 9, blocker: "value-forwarded" } }, at(9)).ok, false);
  const proved = { status: "violations", findings: [finding("SC1001", "strict-read-untracked", "violation", 8)] };
  assert.equal(judgeWitness({ ...row, closedAt: { file: "App.tsx", line: 8, id: "SC1001" } }, proved).ok, true);
  assert.equal(judgeWitness({ ...row, closedAt: { file: "App.tsx", line: 7, id: "SC1001" } }, proved).ok, false);
  // Review R4: the line is the site only in its file.
  const inHelper = { status: "uncertifiable", findings: [finding("SC9012", "reactive-dispatch-unresolved", "uncertifiable", 9, "value-in-carrier", "h.ts")] };
  assert.equal(judgeWitness({ ...row, closedAt: { file: "App.tsx", line: 9, blocker: "value-in-carrier" } }, inHelper).ok, false);
  assert.equal(judgeWitness({ ...row, closedAt: { file: "h.ts", line: 9, blocker: "value-in-carrier" } }, inHelper).ok, true);
  assert.equal(judgeWitness({ ...row, closedAt: { line: 9, blocker: "value-in-carrier" } }, at(9)).ok, false);
});

test("an SC9012 covers a fired code only at the row's closedAt (review 4c): elsewhere, or on a row with none, it covers nothing", () => {
  const withheld = (line) => ({ status: "uncertifiable", findings: [finding("SC9012", "reactive-dispatch-unresolved", "uncertifiable", line, "value-forwarded")] });
  for (const kind of ["keep", "regress"]) {
    assert.match(judgeWitness({ rule: "SC1001", kind, runtime: fired, before: { findings: [] } }, withheld(4)).why, /no finding covers it/, kind);
    assert.match(judgeWitness({ rule: "SC1001", kind, runtime: fired, before: { findings: [] }, closedAt: { file: "App.tsx", line: 9, blocker: "value-forwarded" } }, withheld(4)).why, /no finding covers it/, kind);
  }
  assert.equal(judgeWitness({ rule: "SC9012", kind: "clean", runtime: fired, closedAt: { file: "App.tsx", line: 4, blocker: "value-forwarded" } }, withheld(4)).ok, true);
});

test("a covering finding keeps the row", () => {
  const out = { status: "violations", findings: [finding("SC1001", "strict-read-untracked", "violation")] };
  assert.equal(judgeWitness({ rule: "SC1001", kind: "clean", runtime: fired }, out).ok, true);
});

test("a contradicted violation does not hide a missed code", () => {
  const out = { status: "violations", findings: [finding("SC2001", "no-write-in-pure-computation", "violation")] };
  assert.equal(judgeWitness({ rule: "SC1001", kind: "keep", runtime: fired, before: { findings: [] } }, out).ok, false);
});

test("a quiet row keeps today's kind rules", () => {
  assert.equal(judgeWitness({ rule: "SC1001", kind: "regress", runtime: quiet }, certified).ok, false);
  assert.equal(judgeWitness({ rule: "SC1001", kind: "regress", runtime: quiet }, { status: "uncertifiable", findings: [finding("SC1001", "strict-read-untracked", "uncertifiable", 5, "function-run-unplaced")] }).ok, true);
  assert.equal(judgeWitness({ rule: "SC1001", kind: "warn", runtime: quiet }, { status: "violations", findings: [finding("SC1001", "strict-read-untracked", "violation")] }).ok, false);
});

test("an uncertifiable with no named blocker fails any row", () => {
  const out = { status: "uncertifiable", findings: [finding("SC1001", "strict-read-untracked", "uncertifiable", 5, "someFunctionName")] };
  assert.match(judgeWitness({ rule: "SC1001", kind: "regress", runtime: quiet }, out).why, /^blocker/);
});

test("a thrown error's code counts as fired", () => {
  assert.deepEqual(recordedCodes({ diagnostics: [], errors: ["render: [REACTIVE_WRITE_IN_OWNED_SCOPE] Writing to reactive state"] }), ["REACTIVE_WRITE_IN_OWNED_SCOPE"]);
  assert.deepEqual(recordedCodes({ diagnostics: [], errors: ["click: [FLUSH_IN_ACTION] x"] }), ["FLUSH_IN_ACTION"]);
  assert.deepEqual(recordedCodes({ diagnostics: [], errors: ["render: node._effectFn is not a function"] }), []);
});

test("a base row removed or changed needs a retired row naming an existing ADR", () => {
  const row = { rule: "SC1001", kind: "clean", source: "a", runtime: fired };
  const decisions = '<a id="adr-0801"></a>\n## ADR 0801';
  assert.deepEqual(vanishedRows({ x: row }, { x: row }, [], decisions), []);
  assert.equal(vanishedRows({ x: row }, {}, [], decisions).length, 1);
  assert.equal(vanishedRows({ x: row }, { x: { ...row, kind: "regress" } }, [], decisions).length, 1);
  assert.equal(vanishedRows({ x: row }, {}, [{ id: "x", adr: "ADR 0999", why: "w" }], decisions).length, 1);
  assert.equal(vanishedRows({ x: row }, {}, [{ id: "x", adr: "ADR 0801" }], decisions).length, 1);
  assert.deepEqual(vanishedRows({ x: row }, {}, [{ id: "x", adr: "ADR 0801", why: "proved" }], decisions), []);
});

test("every field of a row is compared, both ways, and a retired row of a row still there names the fields it excuses", () => {
  const row = { rule: "SC1001", kind: "clean", source: "a", runtime: fired };
  const decisions = '<a id="adr-0801"></a>\n## ADR 0801';
  const closed = { ...row, closedBy: "SC9012", closedAt: { file: "App.tsx", line: 3, blocker: "value-in-carrier" } };
  assert.equal(vanishedRows({ x: row }, { x: closed }, [], decisions).length, 1);
  assert.equal(vanishedRows({ x: closed }, { x: row }, [], decisions).length, 1);
  assert.equal(vanishedRows({ x: row }, { x: { ...row, rule: "SC1003" } }, [], decisions).length, 1);
  assert.equal(vanishedRows({ x: row }, { x: closed }, [{ id: "x", adr: "ADR 0801", why: "w" }], decisions).length, 1);
  assert.equal(vanishedRows({ x: row }, { x: closed }, [{ id: "x", adr: "ADR 0801", why: "w", fields: ["closedBy"] }], decisions).length, 1);
  // The stricter gate (the W carrier brief, job 2): naming the fields is no longer enough, or the excuse would cover any later
  // change to them too. It pins each field's value at the base (`from`) and now (`to`).
  const named = { id: "x", adr: "ADR 0801", why: "w", fields: ["closedBy", "closedAt"] };
  assert.equal(vanishedRows({ x: row }, { x: closed }, [named], decisions).length, 1);
  const pinned = { ...named, from: { closedBy: null, closedAt: null }, to: { closedBy: closed.closedBy, closedAt: closed.closedAt } };
  assert.deepEqual(vanishedRows({ x: row }, { x: closed }, [pinned], decisions), []);
  // A later change to an excused field is no longer excused.
  const moved = { ...closed, closedAt: { ...closed.closedAt, line: 4 } };
  assert.equal(vanishedRows({ x: row }, { x: moved }, [pinned], decisions).length, 1);
});
