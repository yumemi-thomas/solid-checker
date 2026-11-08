import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { haltPlace, offSiteVerdicts, runtimeEvents } from "./classify.mjs";
import { rowProblem } from "./codes.mjs";
import { readGrid } from "./focus.mjs";
import { judgeLegalProgram, legalCells } from "./legal.mjs";
import { buildMatrix, cellList, censusCells, censusShapes, gate, generateShape, judge } from "./matrix.mjs";

// A program with its site at line 10 and a decoy read at line 20, each with a counter (`hit`), and a glue call at line 30.
const built = {
  sites: [{ id: "s1", line: 10, hit: "h1", code: "STRICT_READ_UNTRACKED" }, { id: "s2", line: 20, hit: "h2" }],
  glue: [{ id: "g1", line: 30, hit: "h3" }],
  lineTable: [],
  plan: { clicks: [] },
};
const site = { line: 10 };
// The same program with its text: a repeat compares the source at Chrome's frame with the finding's (column 1 here).
const textOf = (rows) => Array.from({ length: 40 }, (_, index) => rows[index + 1] ?? "x;").join("\n");
const withText = (rows) => ({ ...built, files: { "App.tsx": textOf(rows) } });
const finding = (rule, line, kind = "violation") => ({ rule, kind, line, column: 1, endLine: line, id: "SC", file: "App.tsx" });
const event = (code, ...lines) => ({ code, message: "m", positions: lines.map((line) => ({ file: "App.tsx", line, column: 1 })) });
const run = (diagnostics, hits = { h1: 1, h2: 1, h3: 1 }, extra = {}) => ({ diagnostics, errors: [], witnesses: [], hits, ...extra });
const verdictsOf = (findings, runtime) => offSiteVerdicts({ findings, built, runtime, plan: built.plan }).map((row) => row.verdict + (row.why ? `(${row.why})` : ""));

describe("offSiteVerdicts", () => {
  it("a code of the rule on the finding's lines substantiates it", () => {
    expect(verdictsOf([finding("strict-read-untracked", 20)], run([event("STRICT_READ_UNTRACKED", 20)]))).toEqual(["substantiated"]);
    // A frame of the stack (the call that ran the read) counts as well as the innermost one.
    expect(verdictsOf([finding("strict-read-untracked", 30)], run([event("STRICT_READ_UNTRACKED", 10, 30)]))).toEqual(["substantiated"]);
  });
  it("lines that ran where the runtime reported no code of the rule anywhere are a false positive", () => {
    expect(verdictsOf([finding("strict-read-untracked", 20)], run([event("FLUSH_IN_EFFECT_CALLBACK", 10)]))).toEqual(["false-positive"]);
  });
  it("two reports of one read through two calls stay two: each call's warning is confirmed (Part 16: `f1(); f1();` merged by its read line)", () => {
    const twice = { diagnostics: [event("STRICT_READ_UNTRACKED", 10, 20), event("STRICT_READ_UNTRACKED", 10, 30)], errors: [], witnesses: [], hits: { h1: 1, h2: 1, h3: 1 } };
    expect(runtimeEvents(twice).map((row) => row.positions.map((position) => position.line))).toEqual([[10, 20], [10, 30]]);
    expect(verdictsOf([finding("strict-read-untracked", 20), finding("strict-read-untracked", 30)], twice)).toEqual(["substantiated", "substantiated"]);
    // An exact repeat still merges.
    expect(runtimeEvents({ diagnostics: [event("STRICT_READ_UNTRACKED", 10, 20), event("STRICT_READ_UNTRACKED", 10, 20)] })).toHaveLength(1);
  });
  it("a code of the rule reported only elsewhere does not confirm the finding: a repeat, scored by its pair's twin (coordinator, Part 16; was unconnected)", () => {
    const same = withText({ 10: "r1();", 20: "r1();" });
    expect(offSiteVerdicts({ findings: [finding("strict-read-untracked", 20)], built: same, runtime: run([event("STRICT_READ_UNTRACKED", 10)]), plan: built.plan })).toEqual([{ finding: "strict-read-untracked@20", verdict: "stacked", first: "STRICT_READ_UNTRACKED", why: "repeat" }]);
    // Review of 1d6abdb80: `await x; a(); b();` warns per source, so a report at `a` is no repeat of a finding at `b`; nor is one with no text.
    expect(offSiteVerdicts({ findings: [finding("strict-read-untracked", 20)], built: withText({ 10: "a();", 20: "b();" }), runtime: run([event("STRICT_READ_UNTRACKED", 10)]), plan: built.plan }).map((row) => `${row.verdict}(${row.why})`)).toEqual(["false-positive(repeat-other-source)"]);
    expect(verdictsOf([finding("strict-read-untracked", 20)], run([event("STRICT_READ_UNTRACKED", 10)]))).toEqual(["false-positive(repeat-other-source)"]);
    // An operator keyword is not the source: `delete s1.v` at 20 repeats the write at `s1.v` (column 8) at 10.
    expect(offSiteVerdicts({ findings: [finding("no-store-mutation-outside-setter", 20)], built: withText({ 10: "delete s1.v;", 20: "delete s1.v;" }), runtime: run([{ code: "STORE_WRITE_OUTSIDE_SETTER", message: "m", positions: [{ file: "App.tsx", line: 10, column: 8 }] }]), plan: built.plan })[0].verdict).toBe("stacked");
    expect(offSiteVerdicts({ findings: [finding("no-store-mutation-outside-setter", 20)], built: withText({ 10: "s1.v = 7;", 20: "s1.w = 7;" }), runtime: run([event("STORE_WRITE_OUTSIDE_SETTER", 10)]), plan: built.plan })[0].verdict).toBe("false-positive");
    // Unplaced (no line at all): a diagnostic nobody can place disturbs the whole run (classify.mjs `disturbances`), and may be this one.
    expect(verdictsOf([finding("strict-read-untracked", 20)], run([event("STRICT_READ_UNTRACKED")]))).toEqual(["unjudged(disturbed-unplaced)"]);
  });
  it("a finding whose lines never ran, or that no counter covers, judges nothing", () => {
    expect(verdictsOf([finding("strict-read-untracked", 20)], run([], { h1: 1, h2: 0, h3: 1 }))).toEqual(["unexecuted"]);
    expect(verdictsOf([finding("strict-read-untracked", 25)], run([]))).toEqual(["unjudged(no-site)"]);
    expect(verdictsOf([finding("strict-read-untracked", 20)], run([], null))).toEqual(["unjudged(no-hit-ledger)"]);
  });
  // Coordinator, Part 16: 185 violations hid as unjudged(disturbed) on the smoke. A disturbed run is judged; what the halt changes is named.
  const threw = (code, line) => ({ errors: [{ kind: "page-error", message: code ? `[${code}] m` : "boom", positions: line ? [{ file: "App.tsx", line, column: 1 }] : [] }] });
  it("a disturbed run: lines that ran with nothing of the rule anywhere are a false positive; another code thrown on them first is a stacked claim", () => {
    expect(verdictsOf([finding("strict-read-untracked", 20)], run([], undefined, threw(null)))).toEqual(["false-positive(disturbed)"]);
    // Solid reports the code (a diagnostic on the line), then the throw halts the run.
    const stacked = offSiteVerdicts({ findings: [finding("missing-owner", 20)], built, runtime: run([event("PRIMITIVE_IN_EFFECT_CALLBACK", 20)], undefined, threw(null)), plan: built.plan });
    expect(stacked).toEqual([{ finding: "missing-owner@20", verdict: "stacked", first: "PRIMITIVE_IN_EFFECT_CALLBACK", why: "disturbed" }]);
    // Thrown on another line: nothing stands on these, a false positive.
    expect(verdictsOf([finding("missing-owner", 20)], run([event("PRIMITIVE_IN_EFFECT_CALLBACK", 10)], undefined, threw(null)))).toEqual(["false-positive(disturbed)"]);
  });
  it("a disturbed run: the rule's code placed only off the lines is a false positive, unplaced it judges nothing, lines that never ran stay unexecuted", () => {
    expect(offSiteVerdicts({ findings: [finding("strict-read-untracked", 20)], built: withText({ 10: "r1();", 20: "r1();" }), runtime: run([event("STRICT_READ_UNTRACKED", 10)], undefined, threw(null)), plan: built.plan }).map((row) => `${row.verdict}(${row.why})`)).toEqual(["stacked(disturbed-repeat)"]);
    expect(verdictsOf([finding("strict-read-untracked", 20)], run([event("STRICT_READ_UNTRACKED", 10), event("PRIMITIVE_IN_EFFECT_CALLBACK", 20)], undefined, threw(null)))).toEqual(["false-positive(disturbed-repeat-other-source)"]);
    expect(verdictsOf([finding("strict-read-untracked", 20)], run([event("STRICT_READ_UNTRACKED")], undefined, threw(null)))).toEqual(["unjudged(disturbed-unplaced)"]);
    expect(verdictsOf([finding("strict-read-untracked", 20)], run([], { h1: 1, h2: 0, h3: 1 }, threw(null)))).toEqual(["unexecuted"]);
    expect(verdictsOf([finding("strict-read-untracked", 20)], run([], null, threw(null)))).toEqual(["unjudged(disturbed-no-hit-ledger)"]);
  });
});

describe("halted claims (coordinator's ruling, Part 17; ADR 0900)", () => {
  // Solid halted (REACTIVITY_HALTED at hit-order place `at`) after a DOM error of the program's own; the decoy's counter h2 at line 20.
  const halt = (at, extra = {}) => ({ code: "REACTIVITY_HALTED", message: "m", positions: [], ...(at === undefined ? {} : { orderAt: at }), ...extra });
  const dom = { errors: [{ kind: "window-error", message: "Failed to execute 'insertBefore' on 'Node'", positions: [] }] };
  const halted = (diagnostics, order, hits = { h1: 1, h2: 1, h3: 1 }) => run(diagnostics, hits, { ...dom, order });
  it("haltPlace: before, after, unordered, or no halt", () => {
    expect(haltPlace(halted([halt(2)], ["h1", "h3", "h2"]), ["h2"])).toBe("before");
    expect(haltPlace(halted([halt(2)], ["h1", "h2", "h3"]), ["h2"])).toBe("after");
    // The first halt counts (the earliest place), whatever order the records came in.
    expect(haltPlace(halted([halt(3), halt(1)], ["h1", "h2", "h3"]), ["h2"])).toBe("before");
    expect(haltPlace(halted([halt(3)], ["h1", "h2", "h3"]), ["h2"])).toBe("after");
    // A halt with no place (an older runner, the oracle), a log that misses a hit, a halt past the log's limit, no order at all: no evidence.
    expect(haltPlace(halted([halt()], ["h1", "h3", "h2"]), ["h2"])).toBe("unordered");
    expect(haltPlace(halted([halt(2), halt()], ["h1", "h3", "h2"]), ["h2"])).toBe("unordered");
    expect(haltPlace(halted([halt(2)], ["h1", "h3"]), ["h2"])).toBe("unordered");
    expect(haltPlace(halted([halt(400)], ["h1", "h3", "h2"]), ["h2"])).toBe("unordered");
    expect(haltPlace(run([halt(2)], { h1: 1, h2: 1, h3: 1 }, dom), ["h2"])).toBe("unordered");
    expect(haltPlace(halted([halt(2)], ["h1", "h3", "h2"]), [])).toBe("unordered");
    // Every counter on the lines must run after the halt; a twice-run counter with one run before it is "after".
    expect(haltPlace(halted([halt(2)], ["h2", "h1", "h2"], { h1: 1, h2: 2, h3: 0 }), ["h2"])).toBe("after");
    expect(haltPlace(halted([], ["h1", "h2"]), ["h2"])).toBe(null);
  });
  it("a V whose lines ran only after the halt is a halted claim; with no place it is unjudged; a V that ran before it stays a false positive", () => {
    expect(verdictsOf([finding("missing-owner", 20)], halted([halt(2)], ["h1", "h3", "h2"]))).toEqual(["halted(halt-before-site)"]);
    expect(verdictsOf([finding("missing-owner", 20)], halted([halt()], ["h1", "h3", "h2"]))).toEqual(["unjudged(halt-unordered)"]);
    expect(verdictsOf([finding("missing-owner", 20)], halted([halt(2)], ["h1", "h2", "h3"]))).toEqual(["false-positive(disturbed)"]);
    // Thrown on the V's lines first: a stacked claim as before (ADR 0887), the halt does not change it.
    expect(offSiteVerdicts({ findings: [finding("missing-owner", 20)], built, runtime: halted([event("PRIMITIVE_IN_EFFECT_CALLBACK", 20), halt(2)], ["h1", "h3", "h2"]), plan: built.plan })[0].verdict).toBe("stacked");
    // An undisturbed run is never a halted claim: no halt, no change (and a REACTIVITY_HALTED alone disturbs the run).
    expect(verdictsOf([finding("missing-owner", 20)], run([], { h1: 1, h2: 1, h3: 1 }, { order: ["h1", "h2"] }))).toEqual(["false-positive"]);
  });
  it("the misuse and legal grids score them halted-claim and unplaced, counted problems", () => {
    const fp = { ok: true, findings: [finding("missing-owner", 20)] };
    const at = (runtime) => judge({ code: "STRICT_READ_UNTRACKED", site, typecheck: { ok: true }, checker: fp, runtime, built }).outcome;
    expect(at(halted([halt(2)], ["h1", "h3", "h2"]))).toBe("halted-claim");
    expect(at(halted([halt()], ["h1", "h3", "h2"]))).toBe("unplaced");
    expect(at(halted([halt(2)], ["h1", "h2", "h3"]))).toBe("false-positive");
    expect(rowProblem({ outcome: "halted-claim" })).toBe("halted-claim");
  });
});

describe("the misuse grid's judge, off the site", () => {
  it("a stacked violation is a stacked-claim for a proved pair and a false positive for any other (stacked-pairs.json)", () => {
    const halted = run([event("PRIMITIVE_IN_EFFECT_CALLBACK", 20)], undefined, { errors: [{ kind: "page-error", message: "boom", positions: [] }] });
    const proved = judge({ code: "STRICT_READ_UNTRACKED", site, typecheck: { ok: true }, checker: { ok: true, findings: [finding("missing-owner", 20)] }, runtime: halted, built });
    expect(proved.outcome).toBe("stacked-claim");
    expect(proved.detail).toMatchObject({ primary: "not-fired", pairs: ["PRIMITIVE_IN_EFFECT_CALLBACK|missing-owner"], at: [20] });
    const unproved = judge({ code: "STRICT_READ_UNTRACKED", site, typecheck: { ok: true }, checker: { ok: true, findings: [finding("flush-in-effect-callback", 20)] }, runtime: halted, built });
    expect(unproved.outcome).toBe("false-positive");
    expect(unproved.detail).toMatchObject({ primary: "not-fired", rules: ["flush-in-effect-callback"], at: [20] });
  });
  const strictAtSite = { ok: true, findings: [finding("strict-read-untracked", 10), finding("strict-read-untracked", 20)] };
  it("an agree with a contradicted stray violation is a false positive, and keeps the site's own outcome", () => {
    const checker = { ok: true, findings: [finding("flush-in-effect-callback", 10), finding("strict-read-untracked", 20)] };
    const runtime = run([event("FLUSH_IN_EFFECT_CALLBACK", 10)]);
    const verdict = judge({ code: "FLUSH_IN_EFFECT_CALLBACK", site, typecheck: { ok: true }, checker, runtime, built });
    expect(verdict.outcome).toBe("false-positive");
    expect(verdict.detail).toMatchObject({ primary: "agree", rules: ["strict-read-untracked"], at: [20] });
    // Without the printed program (an old caller) the site is judged alone.
    expect(judge({ code: "FLUSH_IN_EFFECT_CALLBACK", site, typecheck: { ok: true }, checker, runtime }).outcome).toBe("agree");
  });
  // Was "stays its agreement: the code fired" (coordinator, Part 16): the code at the site does not confirm the warning at 20. Chrome reports a
  // strict read per call, so no twin proves a strict-read repeat, and it is a false positive; a proved repeat pair is a stacked claim.
  it("a remote violation of the site's own rule is a repeat: a false positive unless its pair's twin proves it", () => {
    const verdict = judge({ code: "STRICT_READ_UNTRACKED", site, typecheck: { ok: true }, checker: strictAtSite, runtime: run([event("STRICT_READ_UNTRACKED", 10)]), built: withText({ 10: "r1();", 20: "r1();" }) });
    expect(verdict.outcome).toBe("false-positive");
    expect(verdict.detail).toMatchObject({ primary: "agree", offSite: ["strict-read-untracked@20:stacked<STRICT_READ_UNTRACKED(repeat)"], rules: ["strict-read-untracked"] });
    const store = { ok: true, findings: [finding("no-store-mutation-outside-setter", 10), finding("no-store-mutation-outside-setter", 20)] };
    expect(judge({ code: "STORE_WRITE_OUTSIDE_SETTER", site, typecheck: { ok: true }, checker: store, runtime: run([event("STORE_WRITE_OUTSIDE_SETTER", 10)]), built: withText({ 10: "s1.v = 7;", 20: "s1.v = 7;" }) })).toMatchObject({ outcome: "stacked-claim", detail: { pairs: ["STORE_WRITE_OUTSIDE_SETTER|no-store-mutation-outside-setter"] } });
  });
  it("a violation whose rule's code Chrome reported with no line is an unjudged claim, listed", () => {
    const verdict = judge({ code: "STRICT_READ_UNTRACKED", site, typecheck: { ok: true }, checker: strictAtSite, runtime: run([event("STRICT_READ_UNTRACKED", 10)], undefined, { unattributed: ["STRICT_READ_UNTRACKED"] }), built });
    expect(verdict).toMatchObject({ outcome: "unjudged-claim", detail: { rules: ["strict-read-untracked"], at: [20] } });
  });
  it("a false clean at the site stays a false clean, with the off-site row beside it", () => {
    const verdict = judge({ code: "FLUSH_IN_EFFECT_CALLBACK", site, typecheck: { ok: true }, checker: { ok: true, findings: [finding("strict-read-untracked", 20)] }, runtime: run([event("FLUSH_IN_EFFECT_CALLBACK", 10)]), built });
    expect(verdict.outcome).toBe("false-clean");
    expect(verdict.detail.offSite).toEqual(["strict-read-untracked@20:false-positive"]);
  });
});

describe("the judge, past the row's own code (ADR 0614)", () => {
  const judgeOf = (code, findings, events) => judge({ code, site, typecheck: { ok: true }, checker: { ok: true, findings }, runtime: run(events), built, program: undefined });
  it("another rule's violation on the site's lines that ran with its code unreported is a false positive", () => {
    const verdict = judgeOf("FLUSH_IN_EFFECT_CALLBACK", [finding("flush-in-effect-callback", 10), finding("strict-read-untracked", 10)], [event("FLUSH_IN_EFFECT_CALLBACK", 10)]);
    expect(verdict).toMatchObject({ outcome: "false-positive", detail: { primary: "agree", rules: ["strict-read-untracked"], at: [10] } });
  });
  it("a wrong rule the runtime contradicts is a false positive: the row's code fired, and the rule's never did", () => {
    const verdict = judgeOf("FLUSH_IN_EFFECT_CALLBACK", [finding("strict-read-untracked", 10)], [event("FLUSH_IN_EFFECT_CALLBACK", 10)]);
    expect(verdict).toMatchObject({ outcome: "false-positive", detail: { primary: "wrong-rule", rules: ["strict-read-untracked"], at: [10] } });
  });
  it("a claiming violation on a site that ran and reported nothing is a false positive, not a generator problem", () => {
    const verdict = judgeOf("STRICT_READ_UNTRACKED", [finding("strict-read-untracked", 10)], []);
    expect(verdict).toMatchObject({ outcome: "false-positive", detail: { primary: "not-fired", rules: ["strict-read-untracked"] } });
  });
  it("a finding that says the runtime does not warn is contradicted by nothing", () => {
    expect(judgeOf("STRICT_READ_UNTRACKED", [{ ...finding("strict-read-untracked", 10), runtimeSilent: true }], []).outcome).toBe("not-fired");
    expect(judgeOf("FLUSH_IN_EFFECT_CALLBACK", [finding("flush-in-effect-callback", 10), { ...finding("strict-read-untracked", 12), runtimeSilent: true }], [event("FLUSH_IN_EFFECT_CALLBACK", 10)]).outcome).toBe("agree");
  });
  it("a not-fired site that never ran stays not-fired", () => {
    const verdict = judge({ code: "STRICT_READ_UNTRACKED", site, typecheck: { ok: true }, checker: { ok: true, findings: [finding("strict-read-untracked", 10)] }, runtime: { ...run([]), hits: {} }, built });
    expect(verdict.outcome).toBe("not-fired");
  });
});

describe("a rule that stands beside the row's code", () => {
  const checker = { ok: true, findings: [finding("primitive-in-effect-callback", 10), finding("missing-owner", 10)] };
  it("is not judged on the site when the code fired: its throw hides the rule's own warning (ADR 0483)", () => {
    const verdict = judge({ code: "PRIMITIVE_IN_EFFECT_CALLBACK", site, typecheck: { ok: true }, checker, runtime: run([event("PRIMITIVE_IN_EFFECT_CALLBACK", 10)]), built });
    expect(verdict.outcome).toBe("agree");
  });
  it("is judged off the site and on a not-fired program: nothing hid it there", () => {
    const away = { ok: true, findings: [finding("primitive-in-effect-callback", 10), finding("missing-owner", 20)] };
    expect(judge({ code: "PRIMITIVE_IN_EFFECT_CALLBACK", site, typecheck: { ok: true }, checker: away, runtime: run([event("PRIMITIVE_IN_EFFECT_CALLBACK", 10)]), built }).outcome).toBe("false-positive");
    expect(judge({ code: "PRIMITIVE_IN_EFFECT_CALLBACK", site, typecheck: { ok: true }, checker: { ok: true, findings: [finding("missing-owner", 10)] }, runtime: run([]), built }).outcome).toBe("false-positive");
  });
});

describe("the legal grid's judge, off the site", () => {
  const legalBuilt = { ...built, sites: [{ id: "s1", line: 10, hit: "h1", code: "FLUSH_IN_EFFECT_CALLBACK" }, built.sites[1]] };
  it("a clean site with a contradicted violation on a decoy is a false positive", () => {
    const verdict = judgeLegalProgram({ code: "FLUSH_IN_EFFECT_CALLBACK", built: legalBuilt, typecheck: { ok: true }, checker: { ok: true, findings: [finding("strict-read-untracked", 20)] }, runtime: run([]), plan: built.plan });
    expect(verdict.outcome).toBe("false-positive");
    expect(verdict.detail).toMatchObject({ primary: "clean", rules: ["strict-read-untracked"] });
  });
  it("a decoy violation the runtime substantiates leaves the site clean", () => {
    const verdict = judgeLegalProgram({ code: "FLUSH_IN_EFFECT_CALLBACK", built: legalBuilt, typecheck: { ok: true }, checker: { ok: true, findings: [finding("strict-read-untracked", 20)] }, runtime: run([event("STRICT_READ_UNTRACKED", 20)]), plan: built.plan });
    expect(verdict.outcome).toBe("clean");
  });
});

describe("the gate, off the site", () => {
  const row = (seed, outcome) => ({ code: "A", context: "body", seed, outcome, binding: "direct", layer: "none" });
  const matrixOf = (rows) => buildMatrix(rows, { options: { seed: 1, perCell: 4 }, header: { label: "x" }, typedAway: {} });
  it("a program that enters false-positive fails, from agree or from a recall gap", () => {
    const result = gate(matrixOf([row(1, "agree"), row(2, "recall-gap")]), matrixOf([row(1, "false-positive"), row(2, "false-positive")]));
    expect(result.ok).toBe(false);
    expect(result.regressions.find((r) => r.why.startsWith("false-positive")).programs).toEqual(["1: agree -> false-positive", "2: recall-gap -> false-positive"]);
  });
  it("a false clean or a recall gap that stops firing fails: the miss would leave the measure", () => {
    const result = gate(matrixOf([row(1, "false-clean"), row(2, "recall-gap"), row(3, "not-fired")]), matrixOf([row(1, "not-fired"), row(2, "not-fired"), row(3, "not-fired")]));
    expect(result.regressions.find((r) => r.why.startsWith("not-fired")).programs).toEqual(["1: false-clean -> not-fired", "2: recall-gap -> not-fired"]);
  });
});

describe("focus, a certified secondary false clean", () => {
  it("flags a row whose second code was missed in a certified file, and only that one", () => {
    const dir = mkdtempSync(join(tmpdir(), "sc-offsite-"));
    try {
      const rows = [
        { id: "a", outcome: "agree", detail: { secondary: { STRICT_READ_UNTRACKED: "false-clean-certified" } } },
        { id: "b", outcome: "agree", detail: { secondary: { STRICT_READ_UNTRACKED: "false-clean" } } },
      ];
      writeFileSync(join(dir, "programs.jsonl"), `${rows.map((row) => JSON.stringify(row)).join("\n")}\n`);
      expect(readGrid(dir).flagged.map((row) => `${row.id}:${row.outcome}`)).toEqual(["a:secondary-false-clean"]);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});

describe("rowProblem, the one test of focus, the audit and smoke", () => {
  it("names a problem outcome, a certified second-code miss on any outcome, and nothing else", () => {
    expect(rowProblem({ outcome: "false-clean" })).toBe("false-clean");
    expect(rowProblem({ outcome: "agree", detail: { secondary: { X: "false-clean-certified" } } })).toBe("secondary-false-clean");
    expect(rowProblem({ outcome: "agree", detail: { secondary: { X: "false-clean" } } })).toBeNull();
    expect(rowProblem({ outcome: "clean" })).toBeNull();
  });
});

describe("the gate, certified second-code misses", () => {
  const row = (seed, secondary) => ({ code: "A", context: "body", seed, outcome: "agree", binding: "direct", layer: "none", detail: secondary ? { secondary } : {} });
  const matrixOf = (rows) => buildMatrix(rows, { options: { seed: 1, perCell: 4 }, header: { label: "x" }, typedAway: {} });
  const missed = { X: "false-clean-certified" };
  it("fails a cell that gains one", () => {
    const result = gate(matrixOf([row(1)]), matrixOf([row(1, missed)]));
    expect(result.ok).toBe(false);
    expect(result.regressions.map((regression) => regression.why)).toEqual(["certified second-code misses 0 -> 1"]);
  });
  it("fails one that keeps it: no baseline excuses a certified false clean", () => {
    const result = gate(matrixOf([row(1, missed)]), matrixOf([row(1, missed)]));
    expect(result.ok).toBe(false);
    expect(result.regressions).toEqual([]);
    expect(result.certifiedSecondary.map((miss) => miss.code)).toEqual(["X"]);
  });
  it("passes one that loses it", () => {
    expect(gate(matrixOf([row(1, missed)]), matrixOf([row(1)])).ok).toBe(true);
  });
});

describe("the census (ADR 0615)", () => {
  const pair = (cell) => `${cell.code}|${cell.context}`;
  const cells = censusCells();
  it("crosses a pair no cell lists, once, and every code and context a table holds", () => {
    expect(new Set(cells.map(pair)).size).toBe(cells.length);
    const listed = new Set([...cellList({ recall: true }), ...legalCells({ recall: true })].map(pair));
    expect(cells.filter((cell) => listed.has(pair(cell)))).toEqual([]);
    // What a table adds for one code reaches the others with no table entry.
    expect(cells.map(pair)).toContain("LOADING_ON_OUTSIDE_HOLD|memo-async");
  });
  it("leaves out a thrown code where a promise swallows it, its silence being no measure", () => {
    expect(cells.map(pair)).not.toContain("CONTEXT_NOT_FOUND|resolve-thunk");
    expect(cells.map(pair)).not.toContain("DYNAMIC_STATIC_PROMISE|until-thunk");
  });
  it("is restricted by codes and contexts, as the grids' cells are", () => {
    expect(censusCells({ codes: ["RESOLVE_IN_REACTIVE_SCOPE"], contexts: ["for"] })).toEqual([{ code: "RESOLVE_IN_REACTIVE_SCOPE", context: "for" }]);
  });
  it("gives a pair the code's plain shapes, and they build", () => {
    const [shape] = censusShapes("RESOLVE_IN_REACTIVE_SCOPE");
    expect(shape).toMatchObject({ binding: "direct", layer: "none", census: true });
    const { built } = generateShape({ seed: 1 }, { code: "RESOLVE_IN_REACTIVE_SCOPE", context: "for" }, shape, 0);
    expect(built.files["App.tsx"]).toContain("resolve(");
  });
  it("builds every shape of every pair: a pair no template can place is left out in censusCells, with its reason", () => {
    const built = cells.flatMap((cell) => censusShapes(cell.code).map((shape, index) => generateShape({ seed: 1 }, cell, shape, index).built));
    expect(new Set(built.map((program) => program.id)).size).toBe(built.length);
  });
  it("gives `loading-on-outside` its boundary when the code brings no two values of its own", () => {
    const cell = { code: "NO_OWNER_CLEANUP", context: "loading-on-outside" };
    const [shape] = censusShapes(cell.code);
    const app = generateShape({ seed: 1 }, cell, shape, 0).built.files["App.tsx"];
    expect(app).toMatch(/<Loading [^>]*on=\{t\d+\(\)\}>/);
    expect(app).toContain("createMemo(async");
  });
});
