import { describe, expect, it } from "vitest";
import { ENTRY, RECORD, judgePattern } from "./patterns.mjs";
import { accountViolations, frameAt, isUnplaceable, placelessExcess, unplaceableCodes } from "./site-judge.mjs";

// Hand-written rows, never a run's: each is a shape a judge got wrong by reading the file, not the site (ADR 0979).
const v = (rule, line, extra = {}) => ({ rule, line, column: 5, ...extra });
const d = (code, frames = [], extra = {}) => ({ code, frames, ...extra });
const verdicts = (rows) => rows.map((row) => `${row.violation.rule}@${row.violation.line}:${row.verdict}${row.why ? `(${row.why})` : ""}`);

describe("a code Chrome places", () => {
  // `strict-read-untracked` claims STRICT_READ_UNTRACKED, which Chrome reports at the read with the read's frame.
  it("accounts for a violation by a frame of the code's diagnostic on its lines", () => {
    const rows = accountViolations({ violations: [v("strict-read-untracked", 7)], diagnostics: [d("STRICT_READ_UNTRACKED", [{ line: 7, column: 5 }])] });
    expect(verdicts(rows)).toEqual(["strict-read-untracked@7:placed"]);
  });

  it("accounts for a violation by a deeper frame of the stack (the call that led to the read)", () => {
    const rows = accountViolations({ violations: [v("strict-read-untracked", 3)], diagnostics: [d("STRICT_READ_UNTRACKED", [{ line: 11, column: 2 }, { line: 3, column: 9 }])] });
    expect(verdicts(rows)).toEqual(["strict-read-untracked@3:placed"]);
  });

  it("accounts for a violation by its span, not only its first line", () => {
    const rows = accountViolations({ violations: [v("strict-read-untracked", 7, { endLine: 9 })], diagnostics: [d("STRICT_READ_UNTRACKED", [{ line: 8, column: 3 }])] });
    expect(verdicts(rows)).toEqual(["strict-read-untracked@7:placed"]);
  });

  it("flags a hidden extra violation: the code reported at one site no longer covers the other", () => {
    const rows = accountViolations({
      violations: [v("strict-read-untracked", 7), v("strict-read-untracked", 15)],
      diagnostics: [d("STRICT_READ_UNTRACKED", [{ line: 7, column: 5 }])],
    });
    expect(verdicts(rows)).toEqual(["strict-read-untracked@7:placed", "strict-read-untracked@15:unaccounted(elsewhere)"]);
  });

  it("flags a violation of a rule none of whose codes Chrome reported", () => {
    const rows = accountViolations({ violations: [v("strict-read-untracked", 7)], diagnostics: [] });
    expect(verdicts(rows)).toEqual(["strict-read-untracked@7:unaccounted(none)"]);
  });

  it("leaves a violation unjudged when a diagnostic of its code had no frame to place it by", () => {
    const rows = accountViolations({ violations: [v("strict-read-untracked", 7)], diagnostics: [d("STRICT_READ_UNTRACKED", [])] });
    expect(verdicts(rows)).toEqual(["strict-read-untracked@7:unplaced"]);
  });

  it("does not place a violation by a frame of another file at the same line", () => {
    const rows = accountViolations({ violations: [v("strict-read-untracked", 7)], diagnostics: [d("STRICT_READ_UNTRACKED", [{ file: "helpers.ts", line: 7, column: 5 }])] });
    expect(verdicts(rows)).toEqual(["strict-read-untracked@7:unaccounted(elsewhere)"]);
    const inHelper = accountViolations({ violations: [v("strict-read-untracked", 7, { file: "helpers.ts" })], diagnostics: [d("STRICT_READ_UNTRACKED", [{ file: "helpers.ts", line: 7, column: 5 }])] });
    expect(verdicts(inHelper)).toEqual(["strict-read-untracked@7:placed"]);
  });

  it("takes the caller's own frame test", () => {
    const rows = accountViolations({ violations: [v("strict-read-untracked", 7)], diagnostics: [d("STRICT_READ_UNTRACKED", [{ line: 99, column: 1 }])], connects: () => true });
    expect(verdicts(rows)).toEqual(["strict-read-untracked@7:placed"]);
  });

  it("frameAt compares the line span in the same file", () => {
    expect(frameAt(v("r", 4, { endLine: 6 }), { line: 6 })).toBe(true);
    expect(frameAt(v("r", 4, { endLine: 6 }), { line: 7 })).toBe(false);
    expect(frameAt(v("r", 4), { file: "App.tsx", line: 4 })).toBe(true);
    expect(frameAt(v("r", 4), { line: undefined })).toBe(false);
  });
});

describe("which codes Chrome cannot place", () => {
  it("lists the four the pinned bytes raise from the flush, a settle or a fire, and not LOADING_ON_ACCESSOR, whose stack holds the <Loading> element's frame", () => {
    expect([...unplaceableCodes].sort()).toEqual(["ASYNC_OUTSIDE_LOADING_BOUNDARY", "CAUGHT_REACTIVE_READ", "LOADING_ON_OUTSIDE_HOLD", "SETTLED_CLEANUP_UNOWNED"]);
    expect(isUnplaceable("LOADING_ON_ACCESSOR")).toBe(false);
  });
  it("places a loading-on-accessor violation by the <Loading> element's frame, and flags one Chrome did not reach", () => {
    const rows = accountViolations({ violations: [v("loading-on-accessor", 7), v("loading-on-accessor", 21)], diagnostics: [d("LOADING_ON_ACCESSOR", [{ line: 7, column: 5, mount: true }])] });
    expect(verdicts(rows)).toEqual(["loading-on-accessor@7:placed", "loading-on-accessor@21:unaccounted(elsewhere)"]);
  });
});

describe("a code Chrome cannot place", () => {
  // `async-outside-loading-boundary` claims ASYNC_OUTSIDE_LOADING_BOUNDARY, raised from the scheduler: its stack holds no frame of the read.
  const rule = "async-outside-loading-boundary";

  it("keeps a single violation: one diagnostic anywhere covers it", () => {
    const rows = accountViolations({ violations: [v(rule, 9)], diagnostics: [d("ASYNC_OUTSIDE_LOADING_BOUNDARY", [{ line: 99, column: 1 }])] });
    expect(verdicts(rows)).toEqual([`${rule}@9:counted`]);
  });

  it("keeps two violations when Chrome raised two diagnostics", () => {
    const rows = accountViolations({ violations: [v(rule, 9), v(rule, 20)], diagnostics: [d("ASYNC_OUTSIDE_LOADING_BOUNDARY"), d("ASYNC_OUTSIDE_LOADING_BOUNDARY")] });
    expect(verdicts(rows)).toEqual([`${rule}@9:counted`, `${rule}@20:counted`]);
  });

  it("flags more distinct sites than diagnostics (the sibling read outside plus the read a hand-called Loading covers)", () => {
    const rows = accountViolations({ violations: [v(rule, 9), v(rule, 20)], diagnostics: [d("ASYNC_OUTSIDE_LOADING_BOUNDARY")] });
    expect(verdicts(rows)).toEqual([`${rule}@9:unaccounted(excess)`, `${rule}@20:unaccounted(excess)`]);
    expect(rows[0]).toMatchObject({ code: "ASYNC_OUTSIDE_LOADING_BOUNDARY", sites: 2, occurrences: 1 });
  });

  it("counts a site once however many findings stand on it", () => {
    const rows = accountViolations({ violations: [v(rule, 9), v(rule, 9)], diagnostics: [d("ASYNC_OUTSIDE_LOADING_BOUNDARY")] });
    expect(verdicts(rows)).toEqual([`${rule}@9:counted`, `${rule}@9:counted`]);
  });

  it("does not count a site known never to have run", () => {
    const rows = accountViolations({ violations: [v(rule, 9), v(rule, 20)], diagnostics: [d("ASYNC_OUTSIDE_LOADING_BOUNDARY")], ran: (violation) => violation.line !== 20 });
    expect(rows.map((row) => row.verdict)).toEqual(["counted", "counted"]);
  });

  it("counts a site the caller judged itself (alsoCounted)", () => {
    const rows = accountViolations({ violations: [v(rule, 20)], alsoCounted: [v(rule, 9)], diagnostics: [d("ASYNC_OUTSIDE_LOADING_BOUNDARY")] });
    expect(verdicts(rows)).toEqual([`${rule}@20:unaccounted(excess)`]);
  });

  it("budgets per file, and a diagnostic with no file counts for every file", () => {
    const here = [v(rule, 9), v(rule, 4, { file: "helpers.ts" })];
    expect(placelessExcess(here, [{ code: "ASYNC_OUTSIDE_LOADING_BOUNDARY", file: "App.tsx" }]).size).toBe(1);
    expect(placelessExcess(here, [{ code: "ASYNC_OUTSIDE_LOADING_BOUNDARY" }]).size).toBe(0);
    expect(placelessExcess(here, [{ code: "ASYNC_OUTSIDE_LOADING_BOUNDARY", file: "App.tsx" }, { code: "ASYNC_OUTSIDE_LOADING_BOUNDARY", file: "helpers.ts" }]).size).toBe(0);
  });

  it("counts only the codes asked for (the oracle judges CAUGHT_REACTIVE_READ by file, the others by line)", () => {
    const sites = [v(rule, 9), v(rule, 20), v("caught-reactive-read", 4), v("caught-reactive-read", 14)];
    const raised = [{ code: "ASYNC_OUTSIDE_LOADING_BOUNDARY" }, { code: "CAUGHT_REACTIVE_READ" }];
    expect([...placelessExcess(sites, raised).keys()].sort()).toEqual(["ASYNC_OUTSIDE_LOADING_BOUNDARY|App.tsx", "CAUGHT_REACTIVE_READ|App.tsx"]);
    expect([...placelessExcess(sites, raised, { only: ["CAUGHT_REACTIVE_READ"] }).keys()]).toEqual(["CAUGHT_REACTIVE_READ|App.tsx"]);
  });

  it("flags a violation with no diagnostic of its code at all", () => {
    const rows = accountViolations({ violations: [v(rule, 9)], diagnostics: [d("STRICT_READ_UNTRACKED", [{ line: 9, column: 1 }])] });
    expect(verdicts(rows)).toEqual([`${rule}@9:unaccounted(none)`]);
  });

  it("does not spend the budget on a violation another code placed", () => {
    // `missing-owner` claims NO_OWNER_EFFECT (placed) and SETTLED_CLEANUP_UNOWNED (placeless): the first violation is placed by the effect's frame,
    // so the one SETTLED_CLEANUP_UNOWNED diagnostic is the second's.
    const rows = accountViolations({
      violations: [v("missing-owner", 5), v("missing-owner", 12)],
      diagnostics: [d("NO_OWNER_EFFECT", [{ line: 5, column: 5 }]), d("SETTLED_CLEANUP_UNOWNED")],
    });
    expect(verdicts(rows)).toEqual(["missing-owner@5:placed", "missing-owner@12:counted"]);
  });

  it("judges CAUGHT_REACTIVE_READ the same way", () => {
    const rows = accountViolations({ violations: [v("caught-reactive-read", 4), v("caught-reactive-read", 14)], diagnostics: [d("CAUGHT_REACTIVE_READ")] });
    expect(verdicts(rows)).toEqual(["caught-reactive-read@4:unaccounted(excess)", "caught-reactive-read@14:unaccounted(excess)"]);
  });
});

describe("the pattern lane judges violations by site (ADR 0979)", () => {
  const at = (rule, line, column = 5, kind = "violation", path = "/w/p/App.tsx") => ({ id: "SC", rule, kind, primaryLocation: { path, line, column, endLine: line } });
  const reading = (...stack) => ({ codes: ["STRICT_READ_UNTRACKED"], stacks: { STRICT_READ_UNTRACKED: [stack] } });

  it("passes a violation Chrome places at it", () => {
    expect(judgePattern(reading({ file: "App.tsx", line: 7, column: 5 }), [at("strict-read-untracked", 7)]).outcome).toBe("fine");
  });

  it("flags the extra violation of a code Chrome reported at another site of the file", () => {
    const verdict = judgePattern(reading({ file: "App.tsx", line: 7, column: 5 }), [at("strict-read-untracked", 7), at("strict-read-untracked", 15)]);
    expect(verdict).toMatchObject({ outcome: "false-positive", rules: ["strict-read-untracked"], unaccounted: ["strict-read-untracked@15(elsewhere)"] });
  });

  it("keeps a lone violation of a placeless code Chrome reported once", () => {
    const run = { codes: ["ASYNC_OUTSIDE_LOADING_BOUNDARY"], counts: { ASYNC_OUTSIDE_LOADING_BOUNDARY: 1 }, stacks: { ASYNC_OUTSIDE_LOADING_BOUNDARY: [[]] } };
    expect(judgePattern(run, [at("async-outside-loading-boundary", 9)]).outcome).toBe("fine");
  });

  it("flags two violation sites of a placeless code Chrome reported once, the hidden false positive of the 8 programs", () => {
    const run = { codes: ["ASYNC_OUTSIDE_LOADING_BOUNDARY"], counts: { ASYNC_OUTSIDE_LOADING_BOUNDARY: 1 }, stacks: { ASYNC_OUTSIDE_LOADING_BOUNDARY: [[]] } };
    const verdict = judgePattern(run, [at("async-outside-loading-boundary", 9), at("async-outside-loading-boundary", 20)]);
    expect(verdict).toMatchObject({ outcome: "false-positive", rules: ["async-outside-loading-boundary"] });
    expect(verdict.unaccounted).toEqual(["async-outside-loading-boundary@9(excess ASYNC_OUTSIDE_LOADING_BOUNDARY: 2 sites, 1 raised)", "async-outside-loading-boundary@20(excess ASYNC_OUTSIDE_LOADING_BOUNDARY: 2 sites, 1 raised)"]);
  });

  it("keeps two sites when the run raised the code twice", () => {
    const run = { codes: ["ASYNC_OUTSIDE_LOADING_BOUNDARY"], counts: { ASYNC_OUTSIDE_LOADING_BOUNDARY: 2 }, stacks: { ASYNC_OUTSIDE_LOADING_BOUNDARY: [[]] } };
    expect(judgePattern(run, [at("async-outside-loading-boundary", 9), at("async-outside-loading-boundary", 20)]).outcome).toBe("fine");
  });

  it("still reports a violation as misplaced, not a false positive, when it is the only claim of a code reported elsewhere", () => {
    expect(judgePattern(reading({ file: "App.tsx", line: 7, column: 5 }), [at("strict-read-untracked", 12)]).outcome).toBe("misplaced");
  });

  it("keeps the file test for the witness rows, which kept no frames (`positions: false`)", () => {
    expect(judgePattern({ ...reading(), positions: false }, [at("strict-read-untracked", 15)]).outcome).toBe("fine");
  });
});

describe("the pattern lane's entry is the static entry the checker models (ADR 0979)", () => {
  it("imports App statically, after the recording module, never through an await gap", () => {
    for (const clickAll of [false, true]) {
      const entry = ENTRY(clickAll);
      expect(entry).toContain('import * as mod from "./App";');
      expect(entry).not.toMatch(/import\(/);
      // The recorder is the first import of the program's own code, so it is subscribed before App's module evaluates.
      expect(entry.indexOf('from "./entry-record"')).toBeLessThan(entry.indexOf('from "./App"'));
    }
    expect(RECORD).toContain("OBSERVE!.diagnostics.subscribe");
    expect(RECORD).not.toMatch(/import\(/);
  });
});
