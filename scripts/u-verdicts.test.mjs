import assert from "node:assert/strict";
import { test } from "vitest";

import { CLASSES, join, readVerdicts, render, rowClass } from "./u-verdicts.mjs";

// ADR 0954: the blocker groups joined with the verdicts a grid run holds, on in-memory scan lines.
const FIRED = ["OpenMemberWrite(*)", "StandardRootHanded"];
const handed = (fired, atSite = false) => ({ rule: "SC9012", kind: "standard-member-impostor", formula: "handed-root-patch", fired, missing: null, line: 4, atSite });
const program = (id, side, formulas, kinds = {}) => ({ id, grid: side === "legal" ? "lg-recall" : "mx-recall", side, u: { total: 0, kinds }, formulas });
const verdicts = readVerdicts(
  [
    "id\toutcome\tproblem",
    "l1\tclean\t-",
    "l2\tuncertifiable\t-",
    "l3\tnot-legal\tnot-legal",
    "m1\trecall-gap\t-",
    "m2\tagree\t-",
    "m3\ttyped-away\t-",
    "",
  ].join("\n"),
);

test("a verdict table skips its header line and blank lines", () => {
  assert.equal(verdicts.size, 6);
  assert.equal(verdicts.get("l1"), "clean");
  assert.equal(verdicts.get("m2"), "agree");
});

test("a row is read against its program's outcome and where it stands", () => {
  // legal grids: Chrome ran the program and found no defect
  assert.equal(rowClass("legal", "clean", false), "legal");
  assert.equal(rowClass("legal", "uncertifiable", true), "legal");
  assert.equal(rowClass("legal", "not-legal", true), "unjudged");
  // misuse grids: recall-gap is a confirmed defect the checker only declined
  assert.equal(rowClass("misuse", "recall-gap", true), "blocking");
  assert.equal(rowClass("misuse", "recall-gap", false), "away");
  assert.equal(rowClass("misuse", "agree", true), "shadowed");
  assert.equal(rowClass("misuse", "stacked-claim", false), "away");
  assert.equal(rowClass("misuse", "typed-away", true), "unjudged");
  // no held verdict: never guessed
  assert.equal(rowClass("misuse", undefined, true), "unjudged");
  assert.equal(rowClass("legal", undefined, false), "unjudged");
});

const scans = [
  { scan: { grids: ["mx-recall", "lg-recall"], codes: null, contexts: null, ids: null, sample: null, sampleSeed: null, shard: null, seed: "1", programs: 8 } },
  program("l1", "legal", [handed(FIRED), handed(FIRED)], { "standard-member-impostor": [0, 2] }),
  program("l2", "legal", [handed(FIRED)], { "standard-member-impostor": [1, 0], "function-run-unplaced": [0, 3] }),
  program("l3", "legal", [handed(FIRED, true)], { "standard-member-impostor": [1, 0] }),
  program("m1", "misuse", [handed(FIRED, true), handed(["OpenMemberWrite(*)"], true)], { "standard-member-impostor": [2, 0] }),
  program("m2", "misuse", [handed(FIRED, true), handed(FIRED, false)], { "standard-member-impostor": [1, 1] }),
  program("m3", "misuse", [handed(FIRED, true)], { "standard-member-impostor": [1, 0] }),
  program("u1", "misuse", [{ rule: "SC4002", kind: "provider-not-proven-above", formula: null, fired: [], missing: { fact: "provider-above", member: "Ctx" }, line: 9, atSite: true }], { "provider-not-proven-above": [1, 0] }),
  { id: "e1", grid: "mx-recall", error: "exit 2" },
];

test("rows are counted per formula group and per class, and an unheld program is unjudged", () => {
  const result = join(scans, verdicts);
  assert.equal(result.programs, 8);
  assert.equal(result.errors, 1);
  assert.equal(result.unheld, 1);
  const group = (fired) => result.formulas.find((entry) => JSON.stringify(entry.fired) === JSON.stringify(fired));
  assert.deepEqual(CLASSES, ["legal", "away", "blocking", "shadowed", "unjudged"]);
  const both = group(FIRED);
  // l1 twice and l2 (legal), l3 (not-legal: unjudged), m1 at the site (blocking), m2 at the site (shadowed) and away, m3 (typed-away: unjudged)
  assert.deepEqual([both.total, both.legal, both.blocking, both.shadowed, both.away, both.unjudged], [8, 3, 1, 1, 1, 2]);
  const open = group(["OpenMemberWrite(*)"]);
  assert.deepEqual([open.total, open.blocking], [1, 1]);
  assert.deepEqual(result.missing.map(({ fact, member, total, unjudged }) => [fact, member, total, unjudged]), [["provider-above", "Ctx", 1, 1]]);
});

test("a group lists the ways to lift its veto, and the kinds count every row", () => {
  const result = join(scans, verdicts);
  assert.deepEqual(result.formulas[0].lifting, [["OpenMemberWrite(*)"], ["StandardRootHanded"]]);
  const kinds = Object.fromEntries(result.kinds.map((entry) => [entry.kind, entry.total]));
  assert.deepEqual(kinds, { "standard-member-impostor": 9, "function-run-unplaced": 3, "provider-not-proven-above": 1 });
  assert.equal(result.rows, 13);
  // l1's two away rows and l2's three function-run-unplaced rows are legal; l3's is unjudged
  assert.equal(result.classes.legal, 2 + 1 + 3);
});

test("the report is the same bytes whatever order the programs come in", () => {
  const forward = render(join(scans, verdicts), { top: 20 });
  const backward = render(join([...scans].reverse(), verdicts), { top: 20 });
  assert.equal(forward, backward);
  assert.match(forward, /formula groups \(top 2 of 2;/);
  assert.match(forward, /handed-root-patch \[OpenMemberWrite\(\*\), StandardRootHanded\]/);
  assert.match(forward, /lifts with: OpenMemberWrite\(\*\)/);
  assert.match(forward, /provider-above Ctx/);
  // misuse% of the group with two judged-misuse rows among the six it has judged: 2 of 6 (blocking 1 + shadowed 1)
  assert.match(forward, /misuse%/);
  assert.match(forward, /33\.3%\s+standard-member-impostor handed-root-patch \[OpenMemberWrite\(\*\), StandardRootHanded\]/);
});

test("a ranking ties by key, and --top cuts the list", () => {
  const programs = [
    program("l1", "legal", [handed(["B"]), handed(["A"])]),
    program("l2", "legal", [handed(["A"]), handed(["B"])]),
  ];
  const result = join(programs, readVerdicts("id\toutcome\tproblem\nl1\tclean\t-\nl2\tclean\t-\n"));
  assert.deepEqual(result.formulas.map((entry) => entry.fired), [["A"], ["B"]]);
  const text = render(result, { top: 1 });
  assert.match(text, /top 1 of 2/);
  assert.doesNotMatch(text, /\[B\]/);
});

test("a sampled scan says so, with the factor to the grid", () => {
  const sampled = [{ scan: { grids: ["mx-recall"], codes: null, contexts: ["aw-helper-late"], ids: null, sample: 0.1, sampleSeed: "1", shard: null, seed: "1", programs: 5 } }, ...scans.slice(1, 3)];
  const text = render(join(sampled, verdicts));
  assert.match(text, /SAMPLE 0\.1 \(seed 1\): counts are the sample's, the grid's are about 10 times these/);
  assert.match(text, /contexts aw-helper-late/);
});
