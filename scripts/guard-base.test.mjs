import assert from "node:assert/strict";
import { test } from "vitest";

import { adrSection, candidateProblems, heldCap, heldProblems, judgeFrozenProblems } from "./guard/base.mjs";

// ADR 0995: ratchets that only the base can hold, on in-memory files.
const decisions = '<a id="adr-0995"></a>\n## ADR 0995 — x\nthe case new-held-case is held here\n<a id="adr-0803"></a>\n## ADR 0803 — y\nolder-case\n';

const frozen = (over = {}) => ({
  tables: { codeRules: { A: ["r1"] }, handedCodes: ["X"] },
  rows: [{ id: "SC1001/c1", codes: ["A"], findings: [], expected: { outcome: "false-clean", certified: true } }],
  siteRows: [{ id: "site-1", expected: { outcome: "agree" } }],
  legalRows: [],
  patternClaimRows: [],
  changes: [],
  ...over,
});

test("judge-frozen: a row regenerated to expect the checker's new answer is a moved id and needs a changes row", () => {
  // The laundering: a judge change made a frozen false clean read `fine`; the row is rewritten to expect `fine` (a regenerated file),
  // and judge-frozen.test.mjs, which compares the judge with the file, goes green. Nothing named the ADR or the Chrome program.
  const base = frozen();
  const head = frozen({ rows: [{ ...base.rows[0], expected: { outcome: "fine", certified: true } }] });
  const [problem] = judgeFrozenProblems({ head, base, decisions });
  assert.match(problem, /id SC1001\/c1 moved \(row changed in rows\) with no new `changes` row/);
  const row = { id: "SC1001/c1", was: "false-clean", now: "fine", adr: "ADR 0995", why: "the code is claimed by no rule here", program: "patterns/x/y" };
  assert.deepEqual(judgeFrozenProblems({ head: { ...head, changes: [row] }, base, decisions }), []);
  for (const broken of [{ adr: "ADR 0999" }, { adr: undefined }, { program: "" }, { why: " " }, { id: "other" }]) {
    assert.ok(judgeFrozenProblems({ head: { ...head, changes: [{ ...row, ...broken }] }, base, decisions }).length >= 1, JSON.stringify(broken));
  }
});

test("judge-frozen: a removed row, a changed table and a deleted changes row are moves too; an added row is not", () => {
  const base = frozen({ changes: [{ id: "site-1", was: "a", now: "b", adr: "ADR 0803", why: "w", program: "p" }] });
  assert.match(judgeFrozenProblems({ head: { ...base, rows: [] }, base, decisions })[0], /removed from rows/);
  assert.match(judgeFrozenProblems({ head: { ...base, tables: { ...base.tables, handedCodes: ["X", "Y"] } }, base, decisions })[0], /table handedCodes moved/);
  assert.match(judgeFrozenProblems({ head: { ...base, changes: [] }, base, decisions })[0], /append-only/);
  assert.deepEqual(judgeFrozenProblems({ head: { ...base, rows: [...base.rows, { id: "new", expected: {} }] }, base, decisions }), []);
  // A table row with its own change row passes; a change row of the base does not cover a new move of the same id.
  const table = { table: "handedCodes", added: "Y", adr: "ADR 0995", why: "w", program: "p" };
  assert.deepEqual(judgeFrozenProblems({ head: { ...base, tables: { ...base.tables, handedCodes: ["X", "Y"] }, changes: [...base.changes, table] }, base, decisions }), []);
  const again = frozen({ siteRows: [{ id: "site-1", expected: { outcome: "false-clean" } }], changes: base.changes });
  assert.match(judgeFrozenProblems({ head: again, base, decisions })[0], /site-1 moved/);
});

test("candidates: a new key needs an ADR and an owner; shrinking is free", () => {
  const base = { candidates: { a: { owner: "o", why: "w" }, b: { owner: "o", why: "w" } } };
  assert.deepEqual(candidateProblems({ head: { candidates: { a: base.candidates.a } }, base, decisions }), []);
  // The laundering: a hit the gate does not accept is listed as a candidate (the gate only prints those), with no owner and no ADR.
  const [first, second] = candidateProblems({ head: { candidates: { ...base.candidates, c: { why: "later" } } }, base, decisions });
  assert.match(first, /new candidate c: no adr is not an ADR/);
  assert.match(second, /no owner/);
  assert.deepEqual(candidateProblems({ head: { candidates: { ...base.candidates, c: { why: "later", owner: "Mac 3", adr: "ADR 0995" } } }, base, decisions }), []);
  assert.equal(candidateProblems({ head: { candidates: { ...base.candidates, c: { owner: "Mac 3", adr: "ADR 0999" } } }, base, decisions }).length, 1);
});

const held = (id, adr) => ({ id, held: { adr, why: "w" } });
test("held cases: a new held id needs an ADR section that names it, written after the base", () => {
  const base = { cases: [held("older-case", "0803"), { id: "free" }] };
  // The laundering: HELD_CAP is raised by one and a case held under ADR 0803, whose text already describes the whole class.
  const head = { cases: [...base.cases, held("sneaked", "0803")] };
  let problems = heldProblems({ head, base, headCap: 3, baseCap: 2, decisions, baseDecisions: decisions });
  assert.match(problems[0], /held case sneaked: ADR 0803's section does not name the case/);
  // An ADR whose section names the id but already did at the base was not written for it.
  problems = heldProblems({ head: { cases: [...base.cases, held("older-case-2", "0803")] }, base, headCap: 3, baseCap: 2, decisions: decisions.replace("older-case\n", "older-case older-case-2\n"), baseDecisions: decisions.replace("older-case\n", "older-case older-case-2\n") });
  assert.match(problems[0], /already named it at the base/);
  // A case an ADR written now names passes, and an old case newly held counts as new.
  const fresh = { cases: [...base.cases, held("new-held-case", "0995")] };
  assert.deepEqual(heldProblems({ head: fresh, base, headCap: 3, baseCap: 2, decisions, baseDecisions: "" }), []);
  const newlyHeld = { cases: [held("older-case", "0803"), held("free", "0995")] };
  assert.match(heldProblems({ head: newlyHeld, base, headCap: 3, baseCap: 2, decisions, baseDecisions: "" })[0], /ADR 0995's section does not name the case/);
  // A restored case lowers the cap freely.
  assert.deepEqual(heldProblems({ head: { cases: [{ id: "older-case" }, { id: "free" }] }, base, headCap: 1, baseCap: 2, decisions, baseDecisions: decisions }), []);
});

test("HELD_CAP may rise by no more than the new held cases", () => {
  const base = { cases: [held("older-case", "0803")] };
  const fresh = { cases: [...base.cases, held("new-held-case", "0995")] };
  assert.match(heldProblems({ head: fresh, base, headCap: 5, baseCap: 1, decisions, baseDecisions: "" })[0], /rose 1 -> 5 with 1 new held case/);
  assert.match(heldProblems({ head: base, base, headCap: 2, baseCap: 1, decisions, baseDecisions: "" })[0], /rose 1 -> 2 with 0 new/);
  assert.equal(heldCap("export const HELD_CAP = 59;"), 59);
  assert.equal(heldCap("nothing"), null);
  assert.match(heldProblems({ head: base, base, headCap: null, baseCap: 1, decisions, baseDecisions: "" })[0], /not readable/);
});

test("adrSection reads from an anchor to the next", () => {
  assert.match(adrSection(decisions, "0995"), /new-held-case/);
  assert.doesNotMatch(adrSection(decisions, "0995"), /older-case/);
  assert.equal(adrSection(decisions, "0001"), null);
});
