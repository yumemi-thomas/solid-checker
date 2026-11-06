import assert from "node:assert/strict";
import { test } from "vitest";

import { judgeWitness, recordedCodes, vanishedRows } from "./flip/judge.mjs";

// The witness judge (scripts/flip/judge.mjs), on in-memory rows: no checker, no Chrome.
const fired = { diagnostics: ["STRICT_READ_UNTRACKED"], errors: [], logs: [] };
const quiet = { diagnostics: [], errors: [], logs: [] };
const certified = { status: "certified", findings: [] };
const finding = (id, rule, kind, line = 5, analysisContext) => ({ id, rule, kind, primaryLocation: { line }, analysisContext });

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
