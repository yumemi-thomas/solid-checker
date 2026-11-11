// ADR 0950: scripts/what-if.mjs, the one reader of a what-if report. Its reader wants the taint exactly as the build writes it, its comparison of a what-if run with the shipped
// checker is the proof-ablation harness's, its groups are ADR 0954's, and a program that moved is set against Chrome's record with two questions only.
import assert from "node:assert/strict";
import { chmodSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "vitest";

import { AssumedReportError } from "./lib/assumed.mjs";
import { cleanReport, flippedReport } from "./lib/assumed-fixtures.mjs";
import {
  addOutcomes, calibration, canonical, chromeRecords, finishFold, finishJudge, fold, foldOne, groupOf, judgeMoves, judgeOne, judgeReport, label, liftingSetsOf, newFold, newJudge, parseSet,
  premiseSetsFromBaselines, programOutcomes, rankSets, rankText, readBaseline, readWhatIf, resultLines, rowOutcomes, sameReport, setsFromBaselines, slim, standsAt, summaryText, untainted,
} from "./what-if.mjs";

const HERE = "/p";
const finding = ({ line, id = "SC9012", kind = "uncertifiable", rule = "reactive-dispatch-unresolved", start = line * 10, blockedBy, premises, file = "App.tsx" } = {}) => ({
  id, kind, rule, primaryLocation: { path: `${HERE}/${file}`, startByte: start, endByte: start + 5, line, column: 3 }, ...(blockedBy ? { blockedBy } : {}), ...(premises ? { premises } : {}),
});
const handed = (fired) => ({ kind: "standard-member-impostor", scope: "program", formula: "handed-root-patch", fired });
const FIRED = ["OpenMemberWrite(*)", "StandardRootHanded"];
const taint = (report, hazards, flipped) => ({
  ...report,
  assumed: { hazards, flipped },
  findings: report.findings.map((one) => (flipped.length ? { ...one, premises: [{ kind: "assumed", hazards: flipped }] } : one)),
});

test("a set is a name and the ids the build reads: comma separated, each once, in order", () => {
  assert.deepEqual(parseSet("omw=OpenMemberWrite(*),RootPatch,OpenMemberWrite(*)"), { name: "omw", ids: ["OpenMemberWrite(*)", "RootPatch"] });
  assert.throws(() => parseSet("no-equals"), /name=Hazard/);
  assert.throws(() => parseSet("=RootPatch"), /name=Hazard/);
});

test("the baseline is the shipped checker's report: a what-if report handed in as the baseline is refused", () => {
  assert.deepEqual(readBaseline(JSON.stringify(cleanReport())), cleanReport());
  assert.throws(() => readBaseline(JSON.stringify(flippedReport())), AssumedReportError);
});

test("the what-if reader wants the taint exactly as the build writes it, for the assumption it asked for", () => {
  const base = { status: "uncertifiable", findings: [finding({ line: 5, blockedBy: handed(FIRED) })] };
  const ok = taint(base, ["RootPatch", "OpenMemberWrite(*)"], ["OpenMemberWrite(*)"]);
  assert.equal(readWhatIf(JSON.stringify(ok), ["RootPatch", "OpenMemberWrite(*)", "RootPatch"]).assumed.flipped[0], "OpenMemberWrite(*)");
  // nothing flipped: the key, and no premise
  assert.doesNotThrow(() => readWhatIf(JSON.stringify(taint(base, ["RootPatch"], [])), ["RootPatch"]));
  // a default build's report is no what-if report
  assert.throws(() => readWhatIf(JSON.stringify(base), ["RootPatch"]), /no `assumed`: a default build was handed in/);
  // another assumption than the one asked for
  assert.throws(() => readWhatIf(JSON.stringify(taint(base, ["RootPatch"], [])), ["StandardRootHanded"]), /asked for/);
  // a flipped id outside the assumption
  assert.throws(() => readWhatIf(JSON.stringify(taint(base, ["RootPatch"], ["StandardRootHanded"])), ["RootPatch"]), /subset/);
  // a finding without its premise once something flipped, and one with a premise when nothing did
  const bare = { ...ok, findings: base.findings };
  assert.throws(() => readWhatIf(JSON.stringify(bare), ["RootPatch", "OpenMemberWrite(*)"]), /premises/);
  const extra = { ...taint(base, ["RootPatch"], []), findings: [{ ...base.findings[0], premises: [{ kind: "assumed", hazards: ["RootPatch"] }] }] };
  assert.throws(() => readWhatIf(JSON.stringify(extra), ["RootPatch"]), /premises/);
});

test("a what-if run that assumed nothing is the shipped checker's report, key order aside", () => {
  const shipped = { status: "uncertifiable", metrics: { filesAnalyzed: 1, functionsAnalyzed: 3 }, findings: [finding({ line: 5, blockedBy: handed(FIRED) })] };
  const reordered = { findings: [{ blockedBy: shipped.findings[0].blockedBy, ...Object.fromEntries(Object.entries(shipped.findings[0]).reverse().filter(([key]) => key !== "blockedBy")) }], metrics: { functionsAnalyzed: 3, filesAnalyzed: 1 }, status: "uncertifiable" };
  const control = taint(reordered, [], []);
  assert.equal(sameReport(shipped, control), true);
  assert.deepEqual(untainted(taint(shipped, ["RootPatch"], ["RootPatch"])), shipped);
  assert.equal(sameReport(shipped, taint({ ...shipped, status: "certified" }, [], [])), false);
  assert.deepEqual(canonical({ b: [{ d: 1, c: 2 }], a: 1 }), { a: 1, b: [{ c: 2, d: 1 }] });
});

test("a row's group is its kind, formula and fired set (ADR 0954), or its kind alone", () => {
  assert.equal(groupOf(finding({ line: 1, blockedBy: handed(["b", "a"]) })), "standard-member-impostor handed-root-patch [a, b]");
  assert.equal(groupOf(finding({ line: 1, blockedBy: { kind: "function-run-unplaced" } })), "function-run-unplaced");
  assert.equal(groupOf(finding({ line: 1 })), "no-blocker");
});

test("each uncertifiable row has one outcome by its span: violation, still an obligation, another cause, gone; what is new is listed", () => {
  const base = {
    findings: [
      finding({ line: 1, blockedBy: handed(FIRED) }),
      finding({ line: 2, blockedBy: handed(FIRED) }),
      finding({ line: 3, blockedBy: handed(FIRED) }),
      finding({ line: 4, blockedBy: handed(FIRED) }),
      finding({ line: 5, blockedBy: handed(FIRED) }),
      finding({ line: 6, id: "SC1001", kind: "violation", rule: "strict-read-untracked" }),
    ],
  };
  const now = {
    findings: [
      finding({ line: 1, kind: "violation", id: "SC9012" }),
      finding({ line: 2, kind: "violation", id: "SC1001", rule: "strict-read-untracked" }),
      finding({ line: 3, blockedBy: handed(FIRED) }),
      finding({ line: 4, blockedBy: { kind: "function-run-unplaced" } }),
      finding({ line: 6, id: "SC1001", kind: "violation", rule: "strict-read-untracked" }),
      finding({ line: 9, id: "SC1001", kind: "violation", rule: "strict-read-untracked" }),
      finding({ line: 10 }),
    ],
  };
  const { rows, new: added } = rowOutcomes(base, now, HERE);
  assert.deepEqual(rows.map((row) => row.outcome), ["toV", "toVOther", "stillU", "blockedChanged", "toClean"]);
  assert.equal(rows[0].file, "App.tsx");
  assert.deepEqual(rows[1].now.map((one) => one.id), ["SC1001"]);
  assert.deepEqual(added.map((one) => `${one.line}:${one.kind}`), ["9:violation", "10:uncertifiable"]);
  const tally = addOutcomes({}, { rows });
  assert.deepEqual(tally["standard-member-impostor handed-root-patch [OpenMemberWrite(*), StandardRootHanded]"], { rows: 5, toV: 1, toVOther: 1, stillU: 1, blockedChanged: 1, toUOther: 0, besideV: 0, toClean: 1 });
});

test("a violation the shipped checker holds at a row's span already is no move of the row: the obligation beside it stays, or is gone beside it, and only an added violation is `fresh`", () => {
  const prefer = (line) => finding({ line, id: "SC8015", kind: "violation", rule: "prefer-show" });
  const owner = (line, extra = {}) => finding({ line, id: "SC4001", rule: "missing-owner", blockedBy: { kind: "function-run-unplaced" }, ...extra });
  const base = { findings: [owner(1), prefer(1), owner(2), prefer(2), owner(3), prefer(3), owner(4), prefer(4)] };
  const now = {
    findings: [
      // 1: nothing changed: the obligation stands beside the violation the shipped checker has
      owner(1), prefer(1),
      // 2: the obligation is gone, the violation it stood beside stands as before
      prefer(2),
      // 3: another violation is added beside the old one
      owner(3), prefer(3), finding({ line: 3, id: "SC2001", kind: "violation", rule: "no-write-in-pure-computation", start: 30 }),
      // 4: the obligation's own code is a violation now, beside the old one
      finding({ line: 4, id: "SC4001", kind: "violation", rule: "missing-owner" }), prefer(4),
    ],
  };
  const { rows, new: added } = rowOutcomes(base, now, HERE);
  assert.deepEqual(rows.map((row) => row.outcome), ["stillU", "besideV", "toVOther", "toV"]);
  assert.deepEqual(rows.map((row) => row.fresh.map((one) => one.id)), [[], [], ["SC2001"], ["SC4001"]]);
  assert.deepEqual(rows[0].now.map((one) => one.id), ["SC4001", "SC8015"]);
  assert.deepEqual(added, []);
  const tally = addOutcomes({}, { rows });
  assert.deepEqual(tally["function-run-unplaced"], { rows: 4, toV: 1, toVOther: 1, stillU: 1, blockedChanged: 0, toUOther: 0, besideV: 1, toClean: 0 });
  // without the violation beside it the same obligation, gone, is a clean site
  assert.equal(rowOutcomes({ findings: [owner(1)] }, { findings: [] }, HERE).rows[0].outcome, "toClean");
  // and a project counts it apart from a clean site and from a move
  const sets = [{ name: "a", ids: ["RootPatch"] }];
  const idle = { flipped: [], transitions: { cleanToU: [], cleanToV: [], vToU: [], vLost: [], uToV: [], uLost: [] }, outcomes: rowOutcomes(base, now, HERE), status: "violation", now: [] };
  const report = fold([{ program: "kui/x", project: "kui/x", bySet: { a: idle } }], sets);
  assert.deepEqual(report.projects.a["kui/x"], { programs: 1, rows: 4, toV: 2, toClean: 0, stillU: 1, other: 0, besideV: 1, newV: 0, newU: 0 });
});

test("the fold is the harness's: transitions per set, groups per set, projects, and the control", () => {
  const base = { findings: [finding({ line: 1, blockedBy: handed(FIRED) }), finding({ line: 2, blockedBy: handed(FIRED) })] };
  const now = { findings: [finding({ line: 1, kind: "violation" })] };
  const outcomes = rowOutcomes(base, now, HERE);
  const entry = { flipped: ["RootPatch"], transitions: { cleanToU: [], cleanToV: [], vToU: [], vLost: [], uToV: [{ site: "a" }], uLost: [{ site: "b" }] }, outcomes, status: "violation", now: [] };
  const idle = { flipped: [], transitions: { cleanToU: [], cleanToV: [], vToU: [], vLost: [], uToV: [], uLost: [] }, outcomes: { rows: [], new: [] }, status: "certified", now: [] };
  const sets = [{ name: "a", ids: ["RootPatch"] }, { name: "b", ids: ["StandardRootHanded"] }];
  const report = fold([
    { program: "kui/x", project: "kui/x", bySet: { control: { same: true, flipped: [] }, a: entry, b: idle } },
    { program: "kui/y", project: "kui/y", bySet: { control: { same: true, flipped: [] }, a: idle, b: idle } },
    { program: "broken", error: "exit 2" },
  ], sets);
  assert.equal(report.arms.a.counts.uToV, 1);
  assert.equal(report.arms.a.counts.uLost, 1);
  assert.deepEqual(report.arms.a.witnesses, ["kui/x"]);
  assert.equal(report.arms.a.inert, 1);
  assert.equal(report.arms.b.inert, 2);
  assert.deepEqual(report.moved, { a: ["kui/x"], b: [] });
  assert.equal(report.projects.a["kui/x"].toV, 1);
  assert.equal(report.projects.a["kui/x"].toClean, 1);
  assert.deepEqual(report.control, { programs: 2, same: 2, flipped: 0 });
  assert.equal(report.errors, 1);
  assert.match(summaryText({ source: "corpus", programs: 3, ...report }, sets), /control: 2 of 2 identical/);
  // what is stored of an entry: counts always, the moved rows and the findings now only when something moved
  assert.deepEqual(Object.keys(slim(idle)).sort(), ["flipped", "rows", "status"]);
  assert.deepEqual(Object.keys(slim(entry)).sort(), ["flipped", "moved", "new", "now", "rows", "status", "transitions"]);
});

test("a finding the shipped checker had no finding at is counted per project, a violation apart from an obligation (and the fold by one program at a time is the fold)", () => {
  const base = { findings: [finding({ line: 1, blockedBy: handed(FIRED) })] };
  const now = { findings: [finding({ line: 1, kind: "violation" }), finding({ line: 9, id: "SC1001", kind: "violation", rule: "strict-read-untracked" }), finding({ line: 10 }), finding({ line: 11 })] };
  const entry = { flipped: ["RootPatch"], transitions: { cleanToU: [{ site: "x" }, { site: "y" }], cleanToV: [{ site: "z" }], vToU: [], vLost: [], uToV: [{ site: "a" }], uLost: [] }, outcomes: rowOutcomes(base, now, HERE), status: "violation", now: [] };
  const sets = [{ name: "a", ids: ["RootPatch"] }];
  const results = [{ program: "p", project: "demo/p", bySet: { a: entry } }, { program: "q", error: "exit 2" }];
  const whole = fold(results, sets);
  assert.deepEqual(whole.projects.a["demo/p"], { programs: 1, rows: 1, toV: 1, toClean: 0, stillU: 0, other: 0, besideV: 0, newV: 1, newU: 2 });
  const state = newFold(sets);
  for (const result of results) foldOne(state, result);
  assert.deepEqual(finishFold(state), whole);
  assert.equal(whole.errors, 1);
});

test("a control entry that is an error stops the fold and names the program", () => {
  const sets = [{ name: "a", ids: ["RootPatch"] }];
  assert.throws(() => foldOne(newFold(sets), { program: "demo/p", bySet: { control: { error: "the what-if build exited 3: boom" } } }), /the control run failed at demo\/p: the what-if build exited 3: boom/);
});

test("the corpus asks for the lifting sets of its own veto groups too, those the build can name, with enough rows, and not twice", () => {
  const row = (fired) => finding({ line: 1, blockedBy: handed(fired) });
  const rows = (fired, count) => Array.from({ length: count }, () => row(fired));
  const baselines = [
    { findings: [...rows(["OpenMemberWrite(*)", "OpenMemberWrite(<symbol>)", "RootPatch", "StandardRootReceived"], 30), finding({ line: 2, blockedBy: { kind: "function-run-unplaced" } })] },
    { findings: rows(["OpenMemberWrite(*)", "RootPatch", "StandardRootReceived"], 5) },
    { findings: [finding({ line: 3, blockedBy: { kind: "tuple-iteration-written", formula: "array-slots-withdrawn", fired: ["TupleSlotIterationMayBeMutated"] } })] },
  ];
  const derived = setsFromBaselines(baselines, [{ name: "open", ids: ["OpenMemberWrite(*)"] }], 20);
  const names = derived.map((set) => set.name);
  // the veto of the first group lifts with the open write `*` alone (the symbol one is no write by itself), with RootPatch, or with StandardRootReceived; the first is asked already, the
  // other two have the rows of both groups (35), and no tuple-slot atom is asked of a build that cannot name it
  assert.deepEqual(names, ["RootPatch", "StandardRootReceived"]);
  assert.ok(setsFromBaselines(baselines, [], 1).some((set) => set.name === "OpenMemberWrite(*)"), "it is a set when it is not asked already");
  assert.ok(setsFromBaselines(baselines, [], 1000).length === 0);
});

// A program's Chrome record: the oracle's row (diagnostics with `covering`, and the shipped checker's findings).
const record = (diagnostics, findings = []) => ({ id: "p", diagnostics, findings });
const diagnostic = ({ code = "STRICT_READ_UNTRACKED", line, stack = [], covering = [] }) => ({ code, file: "App.tsx", line, stack, covering });
const row = (extra) => ({ group: "g", outcome: "toClean", file: "App.tsx", line: 5, id: "SC9012", kind: "uncertifiable", rule: "reactive-dispatch-unresolved", now: [], ...extra });
const at = (line, kind = "violation", rule = "strict-read-untracked", id = "SC1001") => ({ file: "App.tsx", line, id, kind, rule });

test("a row that disappeared: Chrome silent, still covered, or a false clean", () => {
  const covered = diagnostic({ line: 12, stack: ["App.tsx:12:3", "App.tsx:5:3"], covering: [label(row())] });
  // Chrome reported nothing the row covered
  assert.deepEqual(judgeMoves({ moved: [row()], now: [] }, record([diagnostic({ line: 30 })])).map((v) => v.verdict), ["clean-silent"]);
  // what the row covered is a code with no finding left at its line or a frame of its stack: a false clean
  assert.deepEqual(judgeMoves({ moved: [row()], now: [] }, record([covered])).map((v) => v.verdict), ["FALSE-CLEAN"]);
  assert.deepEqual(judgeMoves({ moved: [row()], now: [at(40)] }, record([covered])).map((v) => v.verdict), ["FALSE-CLEAN"]);
  // an obligation at a frame still covers it; so does a violation of a rule that claims the code at the diagnostic's line
  assert.deepEqual(judgeMoves({ moved: [row()], now: [at(5, "uncertifiable", "reactive-dispatch-unresolved", "SC9012")] }, record([covered])).map((v) => v.verdict), ["clean-covered-elsewhere"]);
  assert.deepEqual(judgeMoves({ moved: [row()], now: [at(12)] }, record([covered])).map((v) => v.verdict), ["clean-covered-elsewhere"]);
  // another rule's violation at the line claims another code: not covered
  assert.deepEqual(judgeMoves({ moved: [row()], now: [at(12, "violation", "no-store-mutation-outside-setter", "SC4001")] }, record([covered])).map((v) => v.verdict), ["FALSE-CLEAN"]);
});

test("a row that became a violation: confirmed where Chrome reports the rule's code, else a wrong rule or a false positive; a new violation is judged the same way", () => {
  const moved = (now) => row({ outcome: "toV", now });
  const chrome = record([diagnostic({ line: 12, stack: ["App.tsx:12:3", "App.tsx:5:3"] })]);
  assert.deepEqual(judgeMoves({ moved: [moved([at(5)])] }, chrome).map((v) => v.verdict), ["violation-confirmed"]);
  assert.deepEqual(judgeMoves({ moved: [moved([at(12)])] }, chrome).map((v) => v.verdict), ["violation-confirmed"]);
  assert.deepEqual(judgeMoves({ moved: [moved([at(5, "violation", "no-store-mutation-outside-setter", "SC4001")])] }, chrome).map((v) => v.verdict), ["WRONG-RULE"]);
  assert.deepEqual(judgeMoves({ moved: [moved([at(40)])] }, chrome).map((v) => v.verdict), ["FALSE-POSITIVE"]);
  assert.deepEqual(judgeMoves({ moved: [moved([at(40)])] }, record([])).map((v) => v.verdict), ["FALSE-POSITIVE"]);
  assert.deepEqual(judgeMoves({ new: [at(40), at(12), at(7, "uncertifiable")] }, chrome).map((v) => [v.group, v.verdict]), [["new", "FALSE-POSITIVE"], ["new", "violation-confirmed"]]);
  // a row that is still an obligation, or another cause, is no verdict
  assert.deepEqual(judgeMoves({ moved: [row({ outcome: "blockedChanged" })] }, chrome), []);
  // a violation the shipped checker held at the span already is not the assumption's: only the added one is judged (`fresh`); a row stored without `fresh` has every violation at the span
  assert.deepEqual(judgeMoves({ moved: [row({ outcome: "toVOther", now: [at(40), at(12)], fresh: [at(12)] })] }, chrome).map((v) => [v.now, v.verdict]), [[label(at(12)), "violation-confirmed"]]);
  assert.deepEqual(judgeMoves({ moved: [row({ outcome: "toVOther", now: [at(40), at(12)] })] }, chrome).map((v) => v.verdict), ["FALSE-POSITIVE", "violation-confirmed"]);
  assert.deepEqual(judgeMoves({ moved: [row({ outcome: "besideV", now: [at(40)], fresh: [] })] }, chrome), []);
});

test("the comparison's reading of `covered` is held to the oracle's own on the shipped checker's findings", () => {
  const findings = [{ file: "App.tsx", line: 5, id: "SC9012", kind: "uncertifiable", rule: "reactive-dispatch-unresolved" }];
  const agrees = diagnostic({ line: 12, stack: ["App.tsx:5:3"], covering: ["App.tsx:5 SC9012 uncertifiable reactive-dispatch-unresolved"] });
  const bare = diagnostic({ line: 30 });
  const differs = diagnostic({ line: 20, covering: [] , stack: ["App.tsx:5:9"] });
  assert.deepEqual(calibration(record([agrees, bare], findings)), { diagnostics: 2, agree: 2, disagree: [] });
  const off = calibration(record([differs], findings));
  assert.equal(off.agree, 0);
  assert.equal(off.disagree.length, 1);
  assert.equal(standsAt(agrees, { file: "App.tsx", line: 5 }), true);
  assert.equal(standsAt(agrees, { file: "Other.tsx", line: 5 }), false);
});

test("a grid report's moved programs are judged against the records read from a directory, and one with no record is not guessed", () => {
  const root = mkdtempSync(join(tmpdir(), "what-if-judge-"));
  try {
    mkdirSync(join(root, "mx-recall-X"), { recursive: true });
    writeFileSync(join(root, "mx-recall-X", "chrome-1.json"), JSON.stringify({ results: [{ id: "has", diagnostics: [diagnostic({ line: 12, covering: [label(row())] })], findings: [] }, { id: "has~clean", diagnostics: [] }] }));
    writeFileSync(join(root, "mx-recall-X", "programs.jsonl"), `${JSON.stringify({ id: "has", outcome: "recall-gap" })}\n${JSON.stringify({ id: "lost", outcome: "typed-away" })}\n`);
    const records = chromeRecords(root);
    assert.deepEqual([...records.keys()], ["has"]);
    assert.deepEqual([...programOutcomes(root)], [["has", "recall-gap"], ["lost", "typed-away"]]);
    const entry = { flipped: ["RootPatch"], transitions: { uLost: [{ site: "s" }] }, moved: [row()], new: [], now: [] };
    const report = { results: [{ program: "has", grid: "mx-recall", code: "X", bySet: { s: entry } }, { program: "lost", grid: "mx-recall", code: "X", bySet: { s: entry } }] };
    const judged = judgeReport(report, records, programOutcomes(root));
    assert.deepEqual(judged.sets.s.verdicts, { "FALSE-CLEAN": 1 });
    assert.equal(judged.sets.s.noRecord, 1);
    assert.deepEqual(judged.sets.s.noRecordBy, { "typed-away": 1 });
    assert.equal(judged.sets.s.problems[0].program, "has");
    assert.equal(judged.calibration.programs, 1);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("a report's results are read from the file it names, a program a line, or from its own list, and judged one at a time as they come", async () => {
  const root = mkdtempSync(join(tmpdir(), "what-if-lines-"));
  try {
    const entry = { flipped: ["RootPatch"], transitions: { uLost: [{ site: "s" }] }, moved: [row()], new: [], now: [] };
    const results = [{ program: "a", grid: "mx-recall", code: "X", bySet: { s: entry } }, { program: "b", grid: "mx-recall", code: "X", bySet: { s: entry } }];
    writeFileSync(join(root, "report.results.jsonl"), results.map((result) => `${JSON.stringify(result)}\n`).join(""));
    const reported = { resultsFile: "report.results.jsonl" };
    const read = [];
    for await (const result of resultLines(reported, join(root, "report.json"))) read.push(result);
    assert.deepEqual(read, results);
    const inline = [];
    for await (const result of resultLines({ results }, join(root, "report.json"))) inline.push(result);
    assert.deepEqual(inline, results);
    const records = new Map([["a", { id: "a", diagnostics: [diagnostic({ line: 12, covering: [label(row())] })], findings: [] }]]);
    const state = newJudge();
    for await (const result of resultLines(reported, join(root, "report.json"))) judgeOne(state, result, records, new Map([["b", "typed-away"]]));
    assert.deepEqual(finishJudge(state), judgeReport({ results }, records, new Map([["b", "typed-away"]])));
    assert.deepEqual(finishJudge(state).sets.s.noRecordBy, { "typed-away": 1 });
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("the lifting sets of a scan's groups, with the rows each would lift", async () => {
  const root = mkdtempSync(join(tmpdir(), "what-if-sets-"));
  try {
    const scan = join(root, "scan.jsonl");
    const formulas = (fired) => [{ kind: "standard-member-impostor", formula: "handed-root-patch", fired }];
    writeFileSync(scan, [
      '{"scan":{"programs":3}}',
      JSON.stringify({ id: "a", grid: "mx-recall", formulas: formulas(["OpenMemberWrite(*)", "RootPatch", "StandardRootHanded", "StandardRootReceived"]) }),
      JSON.stringify({ id: "b", grid: "mx-recall", formulas: formulas(["OpenMemberWrite(*)", "RootPatch", "StandardRootReceived"]) }),
      JSON.stringify({ id: "c", grid: "mx-recall", error: "exit 2" }),
    ].join("\n"));
    const { sets } = await liftingSetsOf(scan);
    const byName = Object.fromEntries(sets.map((set) => [set.ids.join(" + "), set.rows]));
    assert.equal(byName["OpenMemberWrite(*)"], 2);
    assert.equal(byName["RootPatch + StandardRootHanded"], 1);
    assert.equal(byName["StandardRootHanded + StandardRootReceived"], 1);
    assert.equal(byName.RootPatch, 1);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("the corpus command runs the shipped checker and the what-if build, and stops on a control that differs", async () => {
  const { spawnSync } = await import("node:child_process");
  const root = mkdtempSync(join(tmpdir(), "what-if-corpus-"));
  try {
    const project = join(root, "apps", "demo", "proj");
    mkdirSync(project, { recursive: true });
    writeFileSync(join(project, "tsconfig.json"), "{}");
    const manifest = join(root, "manifest.json");
    writeFileSync(manifest, JSON.stringify({ configuration: { runtimeTarget: "browser", runtimeBuild: "development", rendering: "csr" }, projects: [{ app: "demo", root: "demo/proj", project: "tsconfig.json" }] }));
    const typefacts = join(root, "typefacts");
    writeFileSync(typefacts, "producer\n");
    const rows = [finding({ line: 5, blockedBy: handed(FIRED) }), finding({ line: 7, blockedBy: { kind: "function-run-unplaced" } })];
    const fake = (name, source) => {
      const script = join(root, `${name}.mjs`);
      const path = join(root, name);
      writeFileSync(script, source);
      writeFileSync(path, `#!/bin/sh\nexec "${process.execPath}" "${script}" "$@"\n`);
      chmodSync(path, 0o755);
      return path;
    };
    const shipped = fake("shipped", `process.stdout.write(JSON.stringify({ status: "uncertifiable", findings: ${JSON.stringify(rows)}, hazards: [{ id: "OpenMemberWrite(*)", sites: [] }] }));`);
    const whatIf = (control) => fake(`what-if-${control}`, `
      const at = process.argv.indexOf("--assume");
      const ids = process.argv[at + 1].split(",").filter(Boolean);
      const flipped = ids.filter((id) => id === "OpenMemberWrite(*)");
      const rows = ${JSON.stringify(rows)};
      // the assumption lifts the first row; the control of the broken build also drops a row it had no right to
      const kept = ids.length === 0 ? rows.slice(0, ${control === "broken" ? 1 : 2}) : flipped.length ? rows.slice(1) : rows;
      const findings = kept.map((row) => flipped.length ? { ...row, premises: [{ kind: "assumed", hazards: flipped }] } : row);
      process.stdout.write(JSON.stringify({ status: findings.length ? "uncertifiable" : "certified", findings, ...(ids.length === 0 ? { hazards: [{ id: "OpenMemberWrite(*)", sites: [] }] } : {}), assumed: { hazards: ids, flipped } }));`);
    const run = (checkerWhatIf) => spawnSync(process.execPath, [new URL("./what-if.mjs", import.meta.url).pathname, "corpus", "--what-if-bin", checkerWhatIf, "--checker-bin", shipped, "--typefacts", typefacts,
      "--manifest", manifest, "--apps-root", join(root, "apps"), "--set", "omw=OpenMemberWrite(*)", "--set", "patch=RootPatch", "--jobs", "2", "--out", join(root, "report.json")], { encoding: "utf8" });
    const good = run(whatIf("good"));
    assert.equal(good.status, 0, good.stderr);
    const report = JSON.parse((await import("node:fs")).readFileSync(join(root, "report.json"), "utf8"));
    assert.equal(report.kind, "what-if");
    assert.equal(report.resultsFile, "report.results.jsonl");
    const lines = (await import("node:fs")).readFileSync(join(root, report.resultsFile), "utf8").split("\n").filter(Boolean).map((line) => JSON.parse(line));
    assert.deepEqual(lines.map((line) => `${line.program} ${line.set}`).sort(), ["demo/tsconfig.json control", "demo/tsconfig.json omw", "demo/tsconfig.json patch"]);
    assert.equal(lines.find((line) => line.set === "omw").entry.moved.length, 1);
    assert.equal(lines.find((line) => line.set === "omw").entry.now, undefined, "the corpus keeps no list of findings now");
    assert.deepEqual(report.baselines.map((one) => [one.program, one.status]), [["demo/tsconfig.json", "uncertifiable"]]);
    assert.deepEqual(report.control, { programs: 1, same: 1, flipped: 0 });
    assert.equal(report.arms.omw.counts.uLost, 1);
    assert.equal(report.arms.patch.witnesses.length, 0);
    assert.equal(report.groups.omw["standard-member-impostor handed-root-patch [OpenMemberWrite(*), StandardRootHanded]"].toClean, 1);
    assert.equal(report.projects.omw["demo/tsconfig.json"].toClean, 1);
    assert.match(good.stdout, /control: 1 of 1 identical/);
    // a what-if build whose control is not the shipped checker's report stops the run: no report is written over the good one
    const broken = run(whatIf("broken"));
    assert.equal(broken.status, 2);
    assert.match(broken.stderr, /the control differs from the shipped checker/);
    // a control that fails (the build exits 3 when it assumes nothing, or is killed) stops the run and names the run: it is not counted and passed over
    const failing = fake("what-if-failing-control", `
      const at = process.argv.indexOf("--assume");
      const ids = process.argv[at + 1].split(",").filter(Boolean);
      if (ids.length === 0) { process.stderr.write("boom"); process.exit(3); }
      process.stdout.write(JSON.stringify({ status: "certified", findings: [], assumed: { hazards: ids, flipped: [] } }));`);
    const failed = run(failing);
    assert.equal(failed.status, 2);
    assert.match(failed.stderr, /the control run failed at demo\/tsconfig\.json: the what-if build exited 3: boom/);
    // the shipped checker handed in as the what-if build is an error, not a baseline
    const same = run(shipped);
    assert.equal(same.status, 2);
    assert.match(same.stderr, /the what-if build and the shipped checker are two binaries/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

// ADR 0965: a typed cause is a premise a set can assume away.
test("premise sets: one per typed cause with enough uncertifiable rows, the biggest first, none twice, no violation and no row without a kind", () => {
  const baseline = (...kinds) => ({ findings: kinds.map((kind, at) => finding({ line: at + 1, ...(kind === "V" ? { kind: "violation", id: "SC1002", rule: "reactive-read-after-await" } : { blockedBy: { kind } }) })) });
  const baselines = [
    baseline("function-run-unplaced", "function-run-unplaced", "unresolved-invocation", "V"),
    baseline("function-run-unplaced", "unresolved-invocation", "search-depth-limit"),
    { findings: [finding({ line: 9 })] },
  ];
  assert.deepEqual(premiseSetsFromBaselines(baselines, [], 2), [
    { name: "premise:function-run-unplaced", ids: ["Premise(function-run-unplaced)"] },
    { name: "premise:unresolved-invocation", ids: ["Premise(unresolved-invocation)"] },
  ]);
  assert.deepEqual(premiseSetsFromBaselines(baselines, [], 1).map(({ name }) => name), ["premise:function-run-unplaced", "premise:unresolved-invocation", "premise:search-depth-limit"]);
  assert.deepEqual(premiseSetsFromBaselines(baselines, [{ name: "mine", ids: ["Premise(function-run-unplaced)"] }], 2).map(({ name }) => name), ["premise:unresolved-invocation"]);
  assert.deepEqual(premiseSetsFromBaselines(baselines, [], 5), []);
  // a kind is kebab-case or no cause: a row whose blocker names something else asks for no set
  assert.deepEqual(premiseSetsFromBaselines([baseline("NotAKind", "NotAKind")], [], 1), []);
});

test("the ranking of a corpus report: the sets by the rows they lift, the share of the corpus's rows, a set that moves nothing last", () => {
  const group = (toClean, toV = 0) => ({ rows: toClean + toV, toClean, toV, toVOther: 0, stillU: 0, blockedChanged: 0, toUOther: 0, besideV: 0 });
  const report = {
    kind: "what-if", source: "corpus", programs: 2, control: { programs: 2, same: 2, flipped: 0 },
    baselines: [{ rows: { u: 60 } }, { rows: { u: 40 } }],
    sets: [{ name: "a", ids: ["Premise(a-b)"] }, { name: "b", ids: ["Premise(c-d)"] }, { name: "c", ids: ["Premise(e-f)"] }],
    arms: { a: { counts: { uLost: 10 } }, b: { counts: { uLost: 40, uToV: 2 } }, c: { counts: {} } },
    groups: { a: { "a-b": group(12) }, b: { "c-d": group(30, 5), other: group(10) }, c: {} },
    moved: { a: ["p"], b: ["p", "q"], c: [] },
  };
  assert.deepEqual(rankSets(report).map(({ name, sites, rows, toV, programs }) => ({ name, sites, rows, toV, programs })), [
    { name: "b", sites: 42, rows: 45, toV: 5, programs: 2 },
    { name: "a", sites: 10, rows: 12, toV: 0, programs: 1 },
    { name: "c", sites: 0, rows: 0, toV: 0, programs: 0 },
  ]);
  assert.equal(rankSets(report)[0].share, 0.45);
  const text = rankText(report, 2);
  assert.match(text, /100 uncertifiable rows/);
  assert.match(text, /45 rows \( 45\.0%\)\s+42 sites\s+2 program\(s\) toV 5 Premise\(c-d\)/);
  assert.equal(text.split("\n").filter(Boolean).length, 3, "the header and the top two");
});
