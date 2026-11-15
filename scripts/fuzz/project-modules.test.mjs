import { describe, expect, it } from "vitest";
import { offSiteVerdicts } from "./classify.mjs";
import { judgeLegal } from "./legal.mjs";
import { judge } from "./matrix.mjs";
import { judgePattern } from "./patterns.mjs";
import { APP_FILE, accountViolations, fileOfLabel, findingsIn, framesIn, labelOf, lineOfLabel, linesIn, positionFile, programFindings, splitChecker } from "./site-judge.mjs";

// ADR 0993: every project source file of a case is judged like App.tsx. Hand-written rows, never a run's: each is a shape a judge read as App.tsx's only
// (a Chrome frame in `site1.tsx` unplaced, a checker row in `site1.tsx` never read), found by project-site mode `lazy-call-proj` on SETTLED_CLEANUP_UNOWNED.
const row = (file, rule, kind, line, column = 13) => ({ file, rule, kind, id: "SC", line, column, endLine: line });
const APP_TEXT = Array.from({ length: 12 }, (_, index) => `app line ${index + 1};`).join("\n");
const SITE_TEXT = Array.from({ length: 12 }, (_, index) => `site line ${index + 1};`).join("\n");
const files = { "App.tsx": APP_TEXT, "site1.tsx": SITE_TEXT };
const mod = (...rows) => ({ ok: true, findings: [], moduleFindings: rows });

describe("the shared helpers", () => {
  it("a position or finding with no file is App.tsx's", () => {
    expect(positionFile({ line: 1 })).toBe(APP_FILE);
    expect(positionFile({ file: "site1.tsx" })).toBe("site1.tsx");
    expect(positionFile(undefined)).toBe(APP_FILE);
  });
  it("frames and lines are one file's, never another's", () => {
    const stack = [{ file: "site1.tsx", line: 4 }, { line: 4 }, { file: "App.tsx", line: 9 }, { file: "sc-entry.ts" }];
    expect(linesIn(stack, "site1.tsx")).toEqual([4]);
    expect(linesIn(stack)).toEqual([4, 9]);
    expect(framesIn(stack, "other.ts")).toEqual([]);
  });
  it("the program's rows are App.tsx's and every module's, each with its file", () => {
    const checker = { findings: [{ rule: "a", line: 1 }], moduleFindings: [row("site1.tsx", "b", "violation", 4)] };
    expect(programFindings(checker).map((finding) => `${finding.file}:${finding.rule}`)).toEqual(["App.tsx:a", "site1.tsx:b"]);
    expect(findingsIn(checker, "site1.tsx").map((finding) => finding.rule)).toEqual(["b"]);
    expect(findingsIn(checker).map((finding) => finding.rule)).toEqual(["a"]);
  });
  it("labels keep App.tsx's as they were and carry a module's file", () => {
    expect(labelOf({ rule: "r", line: 7, file: "App.tsx" })).toBe("r@7");
    expect(labelOf({ rule: "r", line: 7 })).toBe("r@7");
    expect(labelOf({ rule: "r", line: 4, file: "site1.tsx" })).toBe("r@site1.tsx:4");
    expect([lineOfLabel("r@7"), lineOfLabel("r@site1.tsx:4")]).toEqual([7, 4]);
    expect([fileOfLabel("r@7"), fileOfLabel("r@site1.tsx:4")]).toEqual([APP_FILE, "site1.tsx"]);
  });
});

describe("the matrix judge: a site in a project module (SETTLED_CLEANUP_UNOWNED, a code Chrome cannot place)", () => {
  const code = "SETTLED_CLEANUP_UNOWNED";
  const site = { id: "s1", line: 4, aux: [], file: "site1.tsx" };
  // The one-shot's `fire` raised it after the callback returned: the stack names the module's own frame and the handler's.
  const fire = (positions = [{ file: "site1.tsx", line: 4, column: 13 }, { file: "App.tsx", line: 9, column: 3 }]) => ({ diagnostics: [{ code, message: "m", positions }], errors: [], witnesses: [] });
  const verdict = (checker, runtime = fire(), extra = {}) => judge({ code, site, typecheck: { ok: true }, built: { files }, runtime, checker, ...extra });

  it("a violation of the rule in the site's module agrees (was a false clean: its row was never read)", () => {
    const rows = mod(row("site1.tsx", "missing-owner", "violation", 4));
    expect(verdict(rows)).toMatchObject({ outcome: "agree" });
  });
  it("an uncertifiable row of the rule in the module is the checker declining: a recall gap", () => {
    expect(verdict(mod(row("site1.tsx", "missing-owner", "uncertifiable", 4)))).toMatchObject({ outcome: "recall-gap" });
  });
  it("no row in any file is a false clean", () => {
    expect(verdict(mod()).outcome).toBe("false-clean");
  });
  it("a row of the rule in App.tsx at the same line number is not the module's row: judged by file, the code is claimed only where the program has a row", () => {
    const outcome = verdict({ ok: true, findings: [row("App.tsx", "missing-owner", "violation", 4)] });
    // The program has a violation of the rule (in App.tsx): a code Chrome cannot place is judged by file, so it is the program's claim; the module's own line is not it.
    expect(["agree", "false-clean"]).toContain(outcome.outcome);
    expect(outcome.detail?.findings ?? []).not.toContain("missing-owner/violation@4");
  });
  it("a diagnostic frame in a package file places nothing in the module", () => {
    const runtime = fire([{ file: "fz-site/index.js", line: 4, column: 13 }, { file: "App.tsx", line: 9, column: 3 }]);
    expect(verdict(mod(row("site1.tsx", "missing-owner", "violation", 4)), runtime)).toMatchObject({ outcome: "agree" });
  });
});

describe("the real-Chrome oracle's record lists every file's rows in `findings`", () => {
  it("splitChecker keeps App.tsx's rows in findings and moves the others to moduleFindings; an App-only result is the same object", () => {
    const appOnly = { ok: true, findings: [row("App.tsx", "r", "violation", 1)] };
    expect(splitChecker(appOnly)).toBe(appOnly);
    const mixed = { ok: true, findings: [row("App.tsx", "r", "violation", 1), row("site1.tsx", "missing-owner", "violation", 4)], moduleFindings: [row("other.ts", "r", "violation", 2)] };
    const split = splitChecker(mixed);
    expect(split.findings.map((finding) => finding.file)).toEqual(["App.tsx"]);
    expect(split.moduleFindings.map((finding) => finding.file)).toEqual(["other.ts", "site1.tsx"]);
  });
  it("the matrix judge reads such a record as the fast path's (a module row in findings is the module's, never App.tsx's by its line)", () => {
    const code = "SETTLED_CLEANUP_UNOWNED";
    const site = { id: "s1", line: 4, aux: [], file: "site1.tsx" };
    const runtime = { diagnostics: [{ code, message: "m", positions: [{ file: "site1.tsx", line: 4, column: 13 }] }], errors: [], witnesses: [] };
    const violation = row("site1.tsx", "missing-owner", "violation", 4);
    const verdict = (checker) => judge({ code, site, typecheck: { ok: true }, built: { files }, runtime, checker }).outcome;
    expect(verdict({ ok: true, findings: [violation] })).toBe(verdict({ ok: true, findings: [], moduleFindings: [violation] }));
    expect(verdict({ ok: true, findings: [violation] })).toBe("agree");
  });
  it("the legal judge too", () => {
    const site = { id: "s1", line: 4, aux: [], file: "site1.tsx", hit: "h1" };
    const input = (checker) => ({ code: "REACTIVE_WRITE_IN_OWNED_SCOPE", site, typecheck: { ok: true }, runtime: { diagnostics: [], errors: [], witnesses: [], hits: { h1: 1 } }, checker });
    const violation = row("site1.tsx", "no-write-in-pure-computation", "violation", 4);
    expect(judgeLegal(input({ ok: true, findings: [violation] })).outcome).toBe(judgeLegal(input({ ok: true, findings: [], moduleFindings: [violation] })).outcome);
  });
});

describe("the matrix judge: a violation in a module away from the site is judged like one in App.tsx", () => {
  const code = "REACTIVE_WRITE_IN_OWNED_SCOPE";
  const site = { id: "s1", line: 8, aux: [] };
  const built = { files, sites: [{ id: "s1", line: 8, hit: "h1" }], glue: [], lineTable: [], plan: { clicks: [] }, ledger: [] };
  const write = (positions) => ({ diagnostics: [{ code, message: "m", positions }], errors: [], witnesses: [], hits: { h1: 1 } });
  const siteV = row("App.tsx", "no-write-in-pure-computation", "violation", 8, 3);
  const verdict = (moduleFindings, positions = [{ file: "App.tsx", line: 8, column: 3 }]) => judge({ code, site, typecheck: { ok: true }, built, runtime: write(positions), checker: { ok: true, findings: [siteV], moduleFindings } });

  it("the site's agreement stands when the module's violation has a diagnostic frame there", () => {
    const frames = [{ file: "App.tsx", line: 8, column: 3 }, { file: "site1.tsx", line: 4, column: 13 }];
    expect(verdict([row("site1.tsx", "no-write-in-pure-computation", "violation", 4)], frames).outcome).toBe("agree");
  });
  it("a violation in the module with no diagnostic at its site is a false positive (candidate)", () => {
    const result = verdict([row("site1.tsx", "no-write-in-pure-computation", "violation", 4)]);
    expect(result.outcome).toBe("false-positive");
    expect(result.detail.at).toEqual([4]);
    // The site's own code was reported (at App.tsx's site): not at this module line, so the rule's report is elsewhere, as for an App.tsx violation.
    expect(result.detail.offSite).toEqual(["no-write-in-pure-computation@site1.tsx:4:false-positive(repeat-other-source)"]);
  });
  it("a frame in App.tsx on the module violation's line number does not account for it: files never cross", () => {
    const result = verdict([row("site1.tsx", "no-write-in-pure-computation", "violation", 8)], [{ file: "App.tsx", line: 8, column: 3 }]);
    expect(result.outcome).toBe("false-positive");
  });
  it("a module's uncertifiable row is judged by nobody: it is not a violation", () => {
    expect(verdict([row("site1.tsx", "no-write-in-pure-computation", "uncertifiable", 4)]).outcome).toBe("agree");
  });
});

describe("offSiteVerdicts reads a module violation at that module's frames", () => {
  const built = { sites: [{ id: "s1", line: 8, hit: "h1" }], glue: [], lineTable: [], plan: { clicks: [] }, ledger: [], files };
  const run = (positions) => ({ diagnostics: positions ? [{ code: "STRICT_READ_UNTRACKED", message: "m", positions }] : [], errors: [], witnesses: [], hits: { h1: 1 } });
  const verdicts = (findings, runtime) => offSiteVerdicts({ findings, built, runtime, plan: built.plan }).map((one) => `${one.finding}:${one.verdict}${one.why ? `(${one.why})` : ""}`);
  const violation = row("site1.tsx", "strict-read-untracked", "violation", 4, 13);

  it("a frame of the code in that module on the violation's lines substantiates it", () => {
    expect(verdicts([violation], run([{ file: "site1.tsx", line: 4, column: 13 }]))).toEqual(["strict-read-untracked@site1.tsx:4:substantiated"]);
  });
  it("the same line number in App.tsx substantiates nothing there", () => {
    expect(verdicts([violation], run([{ file: "App.tsx", line: 4, column: 13 }]))).not.toEqual(["strict-read-untracked@site1.tsx:4:substantiated"]);
  });
  it("no report at all is a false positive (the lines of a module carry no counter: unknown counts as ran)", () => {
    expect(verdicts([violation], run(null))).toEqual(["strict-read-untracked@site1.tsx:4:false-positive(module)"]);
  });
  it("the rule's code reported only elsewhere in the program is a repeat, not an unplaced report", () => {
    const elsewhere = verdicts([violation], run([{ file: "App.tsx", line: 9, column: 1 }]))[0];
    expect(elsewhere).toMatch(/^strict-read-untracked@site1.tsx:4:(stacked|false-positive)/);
    expect(elsewhere).not.toContain("unjudged");
  });
  it("App.tsx's own rows keep their label and verdicts", () => {
    expect(verdicts([row("App.tsx", "strict-read-untracked", "violation", 8, 3)], run([{ file: "App.tsx", line: 8, column: 3 }]))).toEqual(["strict-read-untracked@8:substantiated"]);
  });
});

describe("the legal judge reads a module's violation at the site's file", () => {
  const code = "REACTIVE_WRITE_IN_OWNED_SCOPE";
  const site = { id: "s1", line: 4, aux: [], file: "site1.tsx", hit: "h1" };
  const input = (moduleFindings, runtime) => ({ code, site, typecheck: { ok: true }, runtime: { diagnostics: [], errors: [], witnesses: [], hits: { h1: 1 }, ...runtime }, checker: { ok: true, findings: [], moduleFindings } });

  it("a violation in the site's module on a legal program is contradicted: a false positive", () => {
    expect(judgeLegal(input([row("site1.tsx", "no-write-in-pure-computation", "violation", 4)])).outcome).toBe("false-positive");
  });
  it("a diagnostic of the code in the module's frame makes the program not legal, never a pass", () => {
    const runtime = { diagnostics: [{ code, message: "m", positions: [{ file: "site1.tsx", line: 4, column: 1 }] }] };
    expect(judgeLegal({ ...input([], runtime), atSite: true }).outcome).toBe("not-legal");
  });
  it("an App.tsx violation at the same line number is not the site's: another file's lines are not the site's region", () => {
    const result = judgeLegal({ ...input([]), checker: { ok: true, findings: [row("App.tsx", "no-write-in-pure-computation", "violation", 4)] } });
    expect(result.outcome).not.toBe("false-positive");
    expect(result.detail.stray).toEqual(["no-write-in-pure-computation@4"]);
  });
  it("a violation in the module away from the site's lines is listed with its file", () => {
    const result = judgeLegal(input([row("site1.tsx", "no-write-in-pure-computation", "violation", 9)]));
    expect(result.detail.stray).toEqual(["no-write-in-pure-computation@site1.tsx:9"]);
  });
});

describe("the pattern lane and the shared accounting are file-aware (unchanged by ADR 0993; pinned)", () => {
  const at = (rule, line, path, kind = "violation") => ({ id: "SC", rule, kind, primaryLocation: { path, line, column: 5, endLine: line } });
  const reading = (...stack) => ({ codes: ["STRICT_READ_UNTRACKED"], stacks: { STRICT_READ_UNTRACKED: [stack] } });

  it("a V in site1.tsx confirmed by a frame there is fine", () => {
    expect(judgePattern(reading({ file: "site1.tsx", line: 4, column: 5 }), [at("strict-read-untracked", 4, "/w/p/site1.tsx")]).outcome).toBe("fine");
  });
  it("a V in site1.tsx with no diagnostic there is a false positive", () => {
    expect(judgePattern({ codes: [], stacks: {} }, [at("strict-read-untracked", 4, "/w/p/site1.tsx")]).outcome).toBe("false-positive");
  });
  it("a frame in site1.tsx with no row is a false clean", () => {
    expect(judgePattern(reading({ file: "site1.tsx", line: 4, column: 5 }), []).outcome).toBe("false-clean");
  });
  it("a frame in App.tsx on the module violation's line number accounts for nothing", () => {
    const verdict = judgePattern(reading({ file: "App.tsx", line: 4, column: 5 }), [at("strict-read-untracked", 4, "/w/p/site1.tsx"), at("strict-read-untracked", 4, "/w/p/App.tsx")]);
    expect(verdict.outcome).toBe("false-positive");
  });
  it("accountViolations places a violation by its own file's frame only", () => {
    const rows = accountViolations({
      violations: [{ rule: "strict-read-untracked", file: "site1.tsx", line: 4, column: 5 }, { rule: "strict-read-untracked", file: "App.tsx", line: 4, column: 5 }],
      diagnostics: [{ code: "STRICT_READ_UNTRACKED", frames: [{ file: "site1.tsx", line: 4, column: 5 }] }],
    });
    expect(rows.map((one) => `${one.violation.file}:${one.verdict}`)).toEqual(["site1.tsx:placed", "App.tsx:unaccounted"]);
  });
});
