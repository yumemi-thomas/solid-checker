import { describe, expect, it } from "vitest";
import { classify, disturbances, runtimeEvents } from "./classify.mjs";

// Hand-written records. Sites: s1 is a write on line 8 (witness shown when it ran), s2 a read on line 20.
const sites = [
  { id: "s1", line: 8, kind: "write", witness: { selector: '[data-probe="w1"]', text: "17" } },
  { id: "s2", line: 20, kind: "read", witness: { selector: '[data-probe="k1"]', text: "1007" } },
];
const plan = { clicks: ["b1"] };
const violation = (rule, line, endLine = line, id = "SC2001") => ({ rule, id, kind: "violation", line, endLine });
const uncertifiable = (rule, line, endLine = line) => ({ rule, id: "SC2001", kind: "uncertifiable", line, endLine });
const runtime = (over = {}) => ({ failure: null, diagnostics: [], errors: [], witnesses: [{ siteId: "s1", reached: true }, { siteId: "s2", reached: true }], ...over });
const checker = (findings) => ({ ok: true, findings });
const diag = (code, ...lines) => ({ code, message: code, positions: lines.map((line) => ({ file: "App.tsx", line })) });

describe("classify: the checker claims a violation", () => {
  it("is a candidate false positive when the runtime ran the site and said nothing", () => {
    const result = classify({ sites, checker: checker([violation("no-write-in-pure-computation", 8)]), runtime: runtime(), plan });
    expect(result.verdict).toBe("candidate-false-positive");
    expect(result.items[0]).toMatchObject({ kind: "candidate-false-positive", siteIds: ["s1"], finding: { rule: "no-write-in-pure-computation", line: 8 } });
    expect(result.reasons[0]).toMatch(/no diagnostic and no throw/);
  });

  it("agrees when a diagnostic whose code claims the rule has the finding's line on its stack", () => {
    const result = classify({ sites, checker: checker([violation("no-write-in-pure-computation", 8)]), runtime: runtime({ diagnostics: [diag("REACTIVE_WRITE_IN_OWNED_SCOPE", 12, 8)] }), plan });
    expect(result.verdict).toBe("agree");
  });

  it("is inconclusive when the runtime is silent but the site's witness is absent (it never ran)", () => {
    const result = classify({ sites, checker: checker([violation("no-write-in-pure-computation", 8)]), runtime: runtime({ witnesses: [{ siteId: "s1", reached: false }] }), plan });
    expect(result.verdict).toBe("inconclusive");
    expect(result.reasons[0]).toMatch(/witness is absent/);
  });

  it("is inconclusive when nothing shows the site ran (no witness)", () => {
    const result = classify({ sites, checker: checker([violation("no-write-in-pure-computation", 8)]), runtime: runtime({ witnesses: [{ siteId: "s1", reached: null }] }), plan });
    expect(result.verdict).toBe("inconclusive");
    expect(result.reasons[0]).toMatch(/no witness/);
  });

  it("is inconclusive when the program threw anywhere, even elsewhere", () => {
    const result = classify({ sites, checker: checker([violation("no-write-in-pure-computation", 8)]), runtime: runtime({ errors: [{ kind: "window-error", message: "boom", positions: [{ file: "App.tsx", line: 30 }] }] }), plan });
    expect(result.verdict).toBe("inconclusive");
    expect(result.reasons[0]).toMatch(/disturbed.*threw/);
  });

  it("is inconclusive when reactivity halted", () => {
    const result = classify({ sites, checker: checker([violation("no-write-in-pure-computation", 8)]), runtime: runtime({ diagnostics: [diag("REACTIVITY_HALTED")] }), plan });
    expect(result.verdict).toBe("inconclusive");
  });

  it("does not call the silence a contradiction when another code is reported at the finding", () => {
    // The write rule's violation is not contradicted (inconclusive); the read diagnostic has no finding of a rule that claims it.
    const result = classify({ sites, checker: checker([violation("no-write-in-pure-computation", 8)]), runtime: runtime({ diagnostics: [diag("STRICT_READ_UNTRACKED", 8)] }), plan });
    expect(result.items.map((item) => item.kind).sort()).toEqual(["candidate-false-clean", "inconclusive"]);
    expect(result.items.find((item) => item.kind === "inconclusive").reason).toMatch(/STRICT_READ_UNTRACKED.*not a code that claims/);
    expect(result.verdict).toBe("candidate-false-clean");
  });

  it("does not hold a rule with no runtime counterpart against the runtime", () => {
    const result = classify({ sites, checker: checker([violation("jsx-no-duplicate-props", 8)]), runtime: runtime(), plan });
    expect(result.verdict).toBe("inconclusive");
    expect(result.reasons[0]).toMatch(/no runtime diagnostic claims rule/);
  });

  it("ignores the style rules the oracle ignores", () => {
    const result = classify({ sites, checker: checker([violation("prefer-show", 8)]), runtime: runtime(), plan });
    expect(result.verdict).toBe("agree");
  });

  it("is inconclusive when the finding spans no site", () => {
    const result = classify({ sites, checker: checker([violation("strict-read-untracked", 40)]), runtime: runtime(), plan });
    expect(result.verdict).toBe("inconclusive");
  });

  it("uses a finding that spans several lines: any reached site in the span suffices", () => {
    const result = classify({ sites, checker: checker([violation("strict-read-untracked", 5, 22, "SC1001")]), runtime: runtime({ witnesses: [{ siteId: "s1", reached: false }, { siteId: "s2", reached: true }] }), plan });
    expect(result.verdict).toBe("candidate-false-positive");
    expect(result.items[0].siteIds).toEqual(["s2"]);
  });

  it("is not a candidate when a coded thrown error sits at the finding", () => {
    const result = classify({ sites, checker: checker([violation("flush-in-action", 8, 8, "SC")]),
      runtime: runtime({ errors: [{ kind: "window-error", message: "[FLUSH_IN_ACTION] flush called", positions: [{ file: "App.tsx", line: 8 }] }] }), plan });
    expect(result.items[0].kind).toBe("agree");
  });
});

describe("classify: the checker is quiet, uncertifiable or failed", () => {
  it("both silent is agree", () => {
    expect(classify({ sites, checker: checker([]), runtime: runtime(), plan })).toMatchObject({ verdict: "agree", reasons: ["the checker and the runtime are both silent"] });
  });

  it("an uncertifiable finding is never a claim, and covers a runtime diagnostic", () => {
    const silent = classify({ sites, checker: checker([uncertifiable("no-write-in-pure-computation", 8)]), runtime: runtime(), plan });
    expect(silent.verdict).toBe("agree");
    const covered = classify({ sites, checker: checker([uncertifiable("no-write-in-pure-computation", 8)]), runtime: runtime({ diagnostics: [diag("REACTIVE_WRITE_IN_OWNED_SCOPE", 8)] }), plan });
    expect(covered.verdict).toBe("agree");
    expect(covered.items[0].reason).toMatch(/uncertifiable/);
  });

  it("a runtime diagnostic no finding covers is a candidate false clean (phase 4, provisional)", () => {
    const result = classify({ sites, checker: checker([]), runtime: runtime({ diagnostics: [diag("REACTIVE_WRITE_IN_OWNED_SCOPE", 8)] }), plan });
    expect(result.verdict).toBe("candidate-false-clean");
    expect(result.items[0]).toMatchObject({ provisional: true, diagnostic: { code: "REACTIVE_WRITE_IN_OWNED_SCOPE", line: 8 } });
  });

  it("a withholding finding covers any diagnostic at its site", () => {
    const result = classify({ sites, checker: checker([uncertifiable("reactive-dispatch-unresolved", 8)]), runtime: runtime({ diagnostics: [diag("STRICT_READ_UNTRACKED", 8)] }), plan });
    expect(result.verdict).toBe("agree");
  });

  it("a diagnostic with no application position, or with no claiming rule, is not held against the checker", () => {
    const result = classify({ sites, checker: checker([]), runtime: runtime({ diagnostics: [diag("STRICT_READ_UNTRACKED"), diag("REACTIVITY_HALTED", 8)] }), plan });
    expect(result.verdict).toBe("agree");
  });

  it("a checker failure is inconclusive, never agree", () => {
    const result = classify({ sites, checker: { ok: false, error: "timeout", findings: [] }, runtime: runtime(), plan });
    expect(result).toMatchObject({ verdict: "inconclusive", reasons: [expect.stringMatching(/checker failed: timeout/)] });
  });

  it("a runtime failure with no witnesses is inconclusive", () => {
    const result = classify({ sites, checker: checker([violation("no-write-in-pure-computation", 8)]), runtime: { failure: { phase: "import", message: "bad" }, diagnostics: [], errors: [], witnesses: [] }, plan });
    expect(result.verdict).toBe("inconclusive");
  });

  it("ranks candidate-false-positive over everything and inconclusive over agree", () => {
    const mixed = classify({ sites, checker: checker([violation("no-write-in-pure-computation", 8), violation("strict-read-untracked", 20, 20, "SC1001")]), runtime: runtime({ witnesses: [{ siteId: "s1", reached: true }, { siteId: "s2", reached: false }] }), plan });
    expect(mixed.verdict).toBe("candidate-false-positive");
    expect(mixed.items.map((i) => i.kind).sort()).toEqual(["candidate-false-positive", "inconclusive"]);
  });
});

describe("helpers", () => {
  it("runtimeEvents merges the two event passes and adds coded thrown errors", () => {
    const events = runtimeEvents({ diagnostics: [diag("X", 8), diag("X", 8)], errors: [{ kind: "x", message: "Cannot call resolve inside a reactive scope", positions: [{ file: "App.tsx", line: 9 }] }] });
    expect(events.map((e) => `${e.source}:${e.code}`)).toEqual(["diagnostic:X", "thrown:RESOLVE_IN_REACTIVE_SCOPE"]);
  });
  it("disturbances name runner failures, throws, halts and plan misses", () => {
    expect(disturbances(runtime(), plan)).toEqual([]);
    const reasons = disturbances(runtime({ failure: { phase: "render", message: "m" }, errors: [{ kind: "plan", message: "no button b9" }, { kind: "handler", message: "h" }], diagnostics: [diag("REACTIVITY_HALTED")] }), plan);
    expect(reasons.join("|")).toMatch(/runner failure.*threw.*halted.*driving plan/);
  });
});
