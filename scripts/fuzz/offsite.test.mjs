import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { offSiteVerdicts } from "./classify.mjs";
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
const finding = (rule, line, kind = "violation") => ({ rule, kind, line, endLine: line, id: "SC", file: "App.tsx" });
const event = (code, ...lines) => ({ code, message: "m", positions: lines.map((line) => ({ file: "App.tsx", line })) });
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
  it("a code of the rule reported elsewhere is unconnected, never a false positive", () => {
    expect(verdictsOf([finding("strict-read-untracked", 20)], run([event("STRICT_READ_UNTRACKED", 10)]))).toEqual(["unconnected"]);
    // Unplaced (no line at all): a diagnostic nobody can place disturbs the whole run (classify.mjs `disturbances`).
    expect(verdictsOf([finding("strict-read-untracked", 20)], run([event("STRICT_READ_UNTRACKED")]))).toEqual(["unjudged(disturbed)"]);
  });
  it("a finding whose lines never ran, or that no counter covers, or a disturbed run, judges nothing", () => {
    expect(verdictsOf([finding("strict-read-untracked", 20)], run([], { h1: 1, h2: 0, h3: 1 }))).toEqual(["unexecuted"]);
    expect(verdictsOf([finding("strict-read-untracked", 25)], run([]))).toEqual(["unjudged(no-site)"]);
    expect(verdictsOf([finding("strict-read-untracked", 20)], run([], null))).toEqual(["unjudged(no-hit-ledger)"]);
    expect(verdictsOf([finding("strict-read-untracked", 20)], run([], undefined, { errors: [{ kind: "page-error", message: "boom", positions: [] }] }))).toEqual(["unjudged(disturbed)"]);
  });
});

describe("the misuse grid's judge, off the site", () => {
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
  it("a remote violation of the site's own rule stays its agreement: the code fired", () => {
    const verdict = judge({ code: "STRICT_READ_UNTRACKED", site, typecheck: { ok: true }, checker: strictAtSite, runtime: run([event("STRICT_READ_UNTRACKED", 10)]), built });
    expect(verdict.outcome).toBe("agree");
    expect(verdict.detail.offSite).toEqual(["strict-read-untracked@20:unconnected"]);
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
  it("fails a cell that gains one, and passes one that keeps or loses it", () => {
    const missed = { X: "false-clean-certified" };
    expect(gate(matrixOf([row(1)]), matrixOf([row(1, missed)])).ok).toBe(false);
    expect(gate(matrixOf([row(1, missed)]), matrixOf([row(1, missed)])).ok).toBe(true);
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
