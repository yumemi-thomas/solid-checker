import { describe, expect, it } from "vitest";
import { VERDICTS, classify, disturbances, runtimeEvents, unattributedCodes } from "./classify.mjs";

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
    expect(covered.verdict).toBe("recall-gap");
    expect(covered.items[0].reason).toMatch(/uncertifiable/);
  });

  it("a runtime diagnostic no finding covers is a candidate false clean, with its severity", () => {
    const result = classify({ sites, checker: checker([]), runtime: runtime({ diagnostics: [diag("REACTIVE_WRITE_IN_OWNED_SCOPE", 8)] }), plan });
    expect(result.verdict).toBe("candidate-false-clean");
    expect(result.items[0]).toMatchObject({ kind: "candidate-false-clean", diagnostic: { code: "REACTIVE_WRITE_IN_OWNED_SCOPE", line: 8, severity: "error" } });
  });

  // `const cb1 = () => { ...line 8... }; onSettled(cb1);` registered on line 30: the callback runs later, so line 30 is not on the stack.
  describe("a finding on the line where the callback was registered", () => {
    const registrations = (line) => (line >= 7 && line <= 9 ? [30] : []);
    const withRegistration = (findings, code = "CLEANUP_IN_FORBIDDEN_SCOPE", lines = [8, 9]) => classify({ sites: [], checker: checker(findings), runtime: runtime({ diagnostics: [diag(code, ...lines)] }), plan, evidence: { registrationLines: registrations } });
    it("covers the diagnostic: a withholding finding is a recall gap, a violation an agree", () => {
      const declined = withRegistration([uncertifiable("reactive-dispatch-unresolved", 30)]);
      expect(declined.verdict).toBe("recall-gap");
      expect(declined.items.find((item) => item.kind === "recall-gap")).toMatchObject({ diagnostic: { code: "CLEANUP_IN_FORBIDDEN_SCOPE", line: 8, rule: "reactive-dispatch-unresolved" } });
      const proven = withRegistration([violation("leaf-owner-forbidden-call", 30, 30, "SC3001")]);
      expect(proven.items.map((item) => item.kind)).not.toContain("candidate-false-clean");
      expect(proven.items.find((item) => item.kind === "agree")).toMatchObject({ diagnostic: { code: "CLEANUP_IN_FORBIDDEN_SCOPE" } });
    });
    it("is still a false clean without the registration link, or when the finding's rule does not claim the code", () => {
      expect(classify({ sites: [], checker: checker([uncertifiable("reactive-dispatch-unresolved", 30)]), runtime: runtime({ diagnostics: [diag("CLEANUP_IN_FORBIDDEN_SCOPE", 8, 9)] }), plan }).verdict).toBe("candidate-false-clean");
      expect(withRegistration([uncertifiable("no-write-in-pure-computation", 30)]).verdict).toBe("candidate-false-clean");
    });
    it("is still a false clean when the stack is in another function (no link to line 30) or the finding is on another line", () => {
      expect(withRegistration([uncertifiable("reactive-dispatch-unresolved", 30)], "CLEANUP_IN_FORBIDDEN_SCOPE", [40]).verdict).toBe("candidate-false-clean");
      expect(withRegistration([uncertifiable("reactive-dispatch-unresolved", 31)]).verdict).toBe("candidate-false-clean");
    });
  });

  // ASYNC_OUTSIDE_LOADING_BOUNDARY is reported by the scheduler: whatever frames the fast path attaches (here line 3, the mount), the
  // read the checker answers on (line 8) is not among them. It is judged by file, as the misuse matrix judges it.
  describe("a placeless code is judged by file, not by line", () => {
    const placeless = (findings, lines = [3]) => classify({ sites, checker: checker(findings), runtime: runtime({ diagnostics: [diag("ASYNC_OUTSIDE_LOADING_BOUNDARY", ...lines)] }), plan });
    it("is agreed with by a claiming violation anywhere in the file, and is never a false positive for it", () => {
      const result = placeless([violation("async-outside-loading-boundary", 8, 8, "SC5003")]);
      expect(result.items.map((item) => item.kind)).toEqual(["agree"]);
      expect(result.verdict).toBe("agree");
    });
    // The one placeless code is advisory (Solid documents the boundary-less mount as legal), so silence on it is no miss.
    it("is neither a recall gap nor a false clean: the code is advisory", () => {
      expect(placeless([uncertifiable("async-outside-loading-boundary", 8)]).verdict).toBe("agree");
      expect(placeless([]).verdict).toBe("agree");
      expect(placeless([]).items.map((item) => item.kind)).not.toContain("candidate-false-clean");
      expect(placeless([violation("no-write-in-pure-computation", 8)]).items.map((item) => item.kind)).not.toContain("candidate-false-clean");
    });
    it("leaves a code reported at its call to the line", () => {
      const result = classify({ sites, checker: checker([uncertifiable("reactive-dispatch-unresolved", 8)]), runtime: runtime({ diagnostics: [diag("STRICT_READ_UNTRACKED", 3)] }), plan });
      expect(result.verdict).toBe("candidate-false-clean");
    });
  });

  it("a withholding finding at the diagnostic is a recall gap: the checker declined", () => {
    const result = classify({ sites, checker: checker([uncertifiable("reactive-dispatch-unresolved", 8)]), runtime: runtime({ diagnostics: [diag("STRICT_READ_UNTRACKED", 8)] }), plan });
    expect(result.verdict).toBe("recall-gap");
    expect(result.items[0]).toMatchObject({ kind: "recall-gap", diagnostic: { code: "STRICT_READ_UNTRACKED", line: 8, rule: "reactive-dispatch-unresolved" } });
  });

  it("a checker violation at the diagnostic is an agree, never a recall gap or a false clean", () => {
    const result = classify({ sites, checker: checker([violation("no-write-in-pure-computation", 8)]), runtime: runtime({ diagnostics: [diag("REACTIVE_WRITE_IN_OWNED_SCOPE", 8)] }), plan });
    expect(result.items.map((item) => item.kind)).toEqual(["agree"]);
  });

  it("a defect code no rule claims, with no finding, is a false clean; a consequence, advisory or SSR code never is", () => {
    const unclaimed = classify({ sites, checker: checker([]), runtime: runtime({ diagnostics: [diag("RUN_WITH_DISPOSED_OWNER", 8)] }), plan });
    expect(unclaimed.verdict).toBe("candidate-false-clean");
    expect(unclaimed.items[0].diagnostic).toMatchObject({ code: "RUN_WITH_DISPOSED_OWNER", severity: "warn" });
    for (const code of ["REACTIVITY_HALTED", "HUGE_FAN_OUT", "SSR_BOUNDARY_WATERFALL", "NOT_A_CODE"]) {
      expect(classify({ sites, checker: checker([]), runtime: runtime({ diagnostics: [diag(code, 8)] }), plan }).items.map((item) => item.kind), code).not.toContain("candidate-false-clean");
    }
  });

  it("a diagnostic with no line is a false clean only when no claiming finding exists anywhere", () => {
    const unplaced = runtime({ diagnostics: [diag("STRICT_READ_UNTRACKED")] });
    const none = classify({ sites, checker: checker([]), runtime: unplaced, plan });
    expect(none.items.find((item) => item.kind === "candidate-false-clean")?.diagnostic).toMatchObject({ code: "STRICT_READ_UNTRACKED", line: null, unplaced: true });
    const elsewhere = classify({ sites, checker: checker([violation("strict-read-untracked", 20, 20, "SC1001")]), runtime: unplaced, plan });
    expect(elsewhere.items.map((item) => item.kind)).not.toContain("candidate-false-clean");
    expect(classify({ sites, checker: checker([]), runtime: runtime({ diagnostics: [diag("REACTIVITY_HALTED", 8)] }), plan }).verdict).toBe("agree");
  });

  it("ranks the tiers: false clean, wrong rule, known open, recall gap, unexecuted, inconclusive, agree", () => {
    expect(VERDICTS).toEqual(["candidate-false-positive", "candidate-false-clean", "candidate-wrong-rule", "known-open", "recall-gap", "candidate-unexecuted", "inconclusive", "agree"]);
    const both = classify({ sites, checker: checker([uncertifiable("reactive-dispatch-unresolved", 8), violation("no-write-in-pure-computation", 20, 20)]),
      runtime: runtime({ diagnostics: [diag("STRICT_READ_UNTRACKED", 8)], witnesses: [{ siteId: "s1", reached: true }, { siteId: "s2", reached: false }] }), plan });
    expect(both.items.map((item) => item.kind).sort()).toEqual(["inconclusive", "recall-gap"]);
    expect(both.verdict).toBe("recall-gap");
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

  it("a diagnostic no line can be given for disturbs the run: silence at a finding is not evidence", () => {
    const unplaced = runtime({ diagnostics: [{ code: "REACTIVE_WRITE_IN_OWNED_SCOPE", message: "m", positions: [] }] });
    expect(unattributedCodes(unplaced)).toEqual(["REACTIVE_WRITE_IN_OWNED_SCOPE"]);
    expect(unattributedCodes(runtime({ unattributed: ["A", "A", "B"] }))).toEqual(["A", "B"]);
    expect(unattributedCodes(runtime({ diagnostics: [diag("REACTIVITY_HALTED")] }))).toEqual(["REACTIVITY_HALTED"]);
    expect(disturbances(unplaced, plan).join("|")).toMatch(/could not be placed on a line \(REACTIVE_WRITE_IN_OWNED_SCOPE\)/);
    // Halting is reported once, as halting.
    expect(disturbances(runtime({ diagnostics: [diag("REACTIVITY_HALTED")] }), plan)).toEqual(["reactivity halted"]);
    const result = classify({ sites, checker: checker([violation("no-write-in-pure-computation", 8)]), runtime: unplaced, plan });
    expect(result.verdict).toBe("inconclusive");
  });

  it("a placed diagnostic is not unattributed", () => {
    expect(unattributedCodes(runtime({ diagnostics: [diag("STRICT_READ_UNTRACKED", 20)] }))).toEqual([]);
  });
});

// The hit ledger: `ran` (the counter) and `landed` (the witness text) are separate facts.
describe("classify: the hit ledger", () => {
  const witnessed = [{ id: "s1", line: 8, kind: "write", hit: 4, witness: { selector: '[data-probe="w1"]', text: "17" } }];
  const void1 = [{ id: "s1", line: 8, kind: "write", hit: 4, witness: null }];
  const glue = [{ id: "a1", hit: 9, kind: "call", line: 30 }, { id: "a2", hit: 10, kind: "call", line: 30 }];
  const finding = (line = 8) => violation("no-write-in-pure-computation", line);
  const run = (over) => classify({ sites: witnessed, glue, checker: checker([finding()]), plan, runtime: runtime({ witnesses: [{ siteId: "s1", reached: null }], hits: {}, ...over }) });

  it("a site that ran and landed is a candidate, as before", () => {
    const result = run({ witnesses: [{ siteId: "s1", reached: true }], hits: { 4: 1 } });
    expect(result.verdict).toBe("candidate-false-positive");
    expect(result.reasons[0]).toMatch(/witness shown/);
  });

  it("a void payload (no witness) that ran is classifiable: judged by its counter", () => {
    const result = classify({ sites: void1, checker: checker([finding()]), plan, runtime: runtime({ witnesses: [{ siteId: "s1", reached: null }], hits: { 4: 2 } }) });
    expect(result.verdict).toBe("candidate-false-positive");
    expect(result.reasons[0]).toMatch(/its counter ran/);
    expect(classify({ sites: void1, checker: checker([finding()]), plan, runtime: runtime({ witnesses: [{ siteId: "s1", reached: null }], hits: { 4: 0 } }) }).items[0]).toMatchObject({ kind: "inconclusive", why: "never-ran" });
  });

  it("a site with a witness that ran but did not land is inconclusive, not silence", () => {
    for (const reached of [false, null]) {
      const result = run({ witnesses: [{ siteId: "s1", reached }], hits: { 4: 1 } });
      expect(result.items[0]).toMatchObject({ kind: "inconclusive", why: "ran-not-landed" });
    }
  });

  it("a site that never ran is inconclusive 'never-ran' (dead-site is off)", () => {
    expect(run({ hits: { 1: 1 } }).items[0]).toMatchObject({ kind: "inconclusive", why: "never-ran" });
  });

  it("a witness shown while the counter says zero distrusts the readout", () => {
    expect(run({ witnesses: [{ siteId: "s1", reached: true }], hits: { 1: 1 } }).items[0]).toMatchObject({ kind: "inconclusive", why: "readout-mismatch" });
  });

  it("without a readout (hits null) the old rules hold exactly", () => {
    expect(run({ witnesses: [{ siteId: "s1", reached: null }], hits: null }).items[0].why).toBe("no-witness");
    expect(run({ witnesses: [{ siteId: "s1", reached: false }], hits: null }).items[0].why).toBe("witness-absent");
    expect(run({ witnesses: [{ siteId: "s1", reached: true }], hits: null }).verdict).toBe("candidate-false-positive");
  });

  it("a runtime diagnostic at the finding still agrees, hits or no hits", () => {
    expect(run({ diagnostics: [diag("REACTIVE_WRITE_IN_OWNED_SCOPE", 8)], hits: { 4: 1 } }).verdict).toBe("agree");
  });

  it("a finding on a plumbing line maps to the glue calls there instead of 'spans no site'", () => {
    const onGlue = (hits) => classify({ sites: witnessed, glue, checker: checker([violation("strict-read-untracked", 30)]), plan, runtime: runtime({ witnesses: [{ siteId: "s1", reached: null }], hits }) });
    expect(onGlue({ 9: 1, 10: 1 }).verdict).toBe("candidate-false-positive");
    expect(onGlue({ 9: 1, 10: 1 }).items[0].siteIds).toEqual(["a1", "a2"]);
    expect(onGlue({ 9: 0, 10: 0 }).items[0]).toMatchObject({ kind: "inconclusive", why: "never-ran" });
    // with no readout the glue proves nothing; and with no glue given the finding is blind, as before
    expect(onGlue(null).items[0]).toMatchObject({ kind: "inconclusive", why: "no-witness" });
    expect(classify({ sites: witnessed, checker: checker([violation("strict-read-untracked", 30)]), plan, runtime: runtime() }).items[0].reason).toBe("the finding spans no site of interest");
  });

  it("payload sites win over glue: a finding covering a payload site is judged by it alone", () => {
    const both = classify({ sites: witnessed, glue: [{ id: "a3", hit: 11, kind: "call", line: 8 }], checker: checker([finding()]), plan, runtime: runtime({ witnesses: [{ siteId: "s1", reached: null }], hits: { 11: 5 } }) });
    expect(both.items[0]).toMatchObject({ kind: "inconclusive", why: "never-ran" });
  });

  it("an instrumentation-perturbed checker makes the program inconclusive", () => {
    const result = classify({ sites, checker: { ...checker([violation("no-write-in-pure-computation", 8)]), perturbed: ["new on the instrumented text: x|violation|9"] }, runtime: runtime(), plan });
    expect(result.verdict).toBe("inconclusive");
    expect(result.items[0]).toMatchObject({ why: "perturbed-checker" });
  });
});

describe("classify: a real defect named by the wrong rule", () => {
  it("is candidate-wrong-rule when the runtime's code belongs to the finding's defect family", () => {
    // The checker says the cleanup has no owner (missing-owner); the runtime says the scope forbids it (leaf-owner-forbidden-call).
    const result = classify({ sites, checker: checker([violation("missing-owner", 8, 8, "SC3001")]), runtime: runtime({ diagnostics: [diag("CLEANUP_IN_FORBIDDEN_SCOPE", 8)] }), plan });
    expect(result.verdict).toBe("candidate-wrong-rule");
    expect(result.items[0]).toMatchObject({ kind: "candidate-wrong-rule", diagnostic: { code: "CLEANUP_IN_FORBIDDEN_SCOPE", line: 8 }, runtimeRules: ["leaf-owner-forbidden-call"] });
    expect(result.reasons[0]).toMatch(/missing-owner/);
  });
  it("is not claimed across families: a read diagnostic at a write finding stays inconclusive", () => {
    const result = classify({ sites, checker: checker([violation("no-write-in-pure-computation", 8)]), runtime: runtime({ diagnostics: [diag("STRICT_READ_UNTRACKED", 8)] }), plan });
    expect(result.items.some((item) => item.kind === "candidate-wrong-rule")).toBe(false);
  });
  it("a diagnostic whose rule the finding has agrees, and a consequence code at the finding is ignored", () => {
    expect(classify({ sites, checker: checker([violation("missing-owner", 8)]), runtime: runtime({ diagnostics: [diag("NO_OWNER_CLEANUP", 8)] }), plan }).verdict).toBe("agree");
    const halted = classify({ sites, checker: checker([violation("missing-owner", 8)]), runtime: runtime({ diagnostics: [diag("NO_OWNER_CLEANUP", 8), diag("REACTIVITY_HALTED", 8)] }), plan });
    expect(halted.verdict).toBe("agree");
  });
});
