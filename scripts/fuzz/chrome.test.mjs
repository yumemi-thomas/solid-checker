import { describe, expect, it } from "vitest";
import { chromeChecker, chromeDiagnostics, chromeRuntime, chromeStatus } from "./chrome.mjs";
import { classify } from "./classify.mjs";

// A row as scripts/runtime-oracle.mjs writes it (the fields the fuzzer reads).
const row = (over = {}) => ({
  id: "fuzz-1", verdict: "static-only", failure: null, pageErrors: [],
  diagnostics: [{ code: "REACTIVE_WRITE_IN_OWNED_SCOPE", file: "App.tsx", line: 12, column: 5, message: "write", verdict: "covered-violation", stack: ["App.tsx:8", "App.tsx:20"] }],
  findings: [{ file: "App.tsx", rule: "no-write-in-pure-computation", id: "SC2001", kind: "violation", line: 8, endLine: 8 }],
  ...over,
});
const sites = [{ id: "s1", line: 8, kind: "write", witness: { selector: '[data-probe="w1"]', text: "17" } }, { id: "s2", line: 30, kind: "write", witness: null }];

describe("Chrome records", () => {
  it("turns an oracle diagnostic into positions: its site, then each application frame", () => {
    expect(chromeDiagnostics(row())[0].positions).toEqual([
      { file: "App.tsx", line: 12 }, { file: "App.tsx", line: 8 }, { file: "App.tsx", line: 20 },
    ]);
  });

  it("reads witnesses from the report's checkpoints (the last reading of a selector wins)", () => {
    const runtime = chromeRuntime(row(), [{ selector: '[data-probe="w1"]', text: "0" }, { selector: '[data-probe="w1"]', text: " 17 " }], sites);
    expect(runtime.witnesses).toEqual([
      { siteId: "s1", witness: sites[0].witness, present: true, actual: "17", reached: true },
      { siteId: "s2", witness: null, present: false, actual: null, reached: null },
    ]);
    expect(chromeRuntime(row(), [], sites).witnesses[0]).toMatchObject({ present: false, reached: null });
  });

  it("carries page errors and harness failures", () => {
    const runtime = chromeRuntime(row({ verdict: "harness-error", error: "boom", pageErrors: ["x is not a function"] }), [], sites);
    expect(runtime.failure).toMatchObject({ phase: "harness-error" });
    expect(runtime.errors).toEqual([{ kind: "page-error", message: "x is not a function", positions: [] }]);
    expect(chromeChecker(row({ verdict: "type-error" })).ok).toBe(false);
  });

  it("feeds the comparator: a violation with no Chrome diagnostic and the witness shown is a candidate", () => {
    const quiet = row({ diagnostics: [] });
    const result = classify({ sites, checker: chromeChecker(quiet), runtime: chromeRuntime(quiet, [{ selector: '[data-probe="w1"]', text: "17" }], sites), plan: { clicks: [] } });
    expect(result.verdict).toBe("candidate-false-positive");
  });

  it("and Chrome's own diagnostic at the finding settles it as agree", () => {
    const result = classify({ sites, checker: chromeChecker(row()), runtime: chromeRuntime(row(), [{ selector: '[data-probe="w1"]', text: "17" }], sites), plan: { clicks: [] } });
    expect(result.verdict).toBe("agree");
  });
});

// A candidate the fast path raised and Chrome re-ran: the checker proves a write (line 8), the page shows its witness.
const shown = [{ selector: '[data-probe="w1"]', text: "17" }];
const judge = (over) => {
  const r = row({ diagnostics: [], ...over });
  return classify({ sites, checker: chromeChecker(r), runtime: chromeRuntime(r, shown, sites), plan: { clicks: [] } });
};

describe("a diagnostic Chrome could not place", () => {
  it("is carried from the oracle row's `unattributed` list", () => {
    expect(chromeRuntime(row({ unattributed: ["REACTIVE_WRITE_IN_OWNED_SCOPE"] }), [], sites).unattributed).toEqual(["REACTIVE_WRITE_IN_OWNED_SCOPE"]);
    expect(chromeRuntime(row(), [], sites).unattributed).toEqual([]);
  });

  it("makes the candidate inconclusive, never a confirmed false positive", () => {
    expect(judge({}).verdict).toBe("candidate-false-positive");
    const placed = judge({ unattributed: ["REACTIVE_WRITE_IN_OWNED_SCOPE"] });
    expect(placed.verdict).toBe("inconclusive");
    expect(placed.reasons.join(" ")).toMatch(/could not be placed.*REACTIVE_WRITE_IN_OWNED_SCOPE/);
    expect(chromeStatus(placed)).toBe("chrome-inconclusive");
  });

  it("holds for any code, not only the finding's", () => {
    expect(judge({ unattributed: ["STRICT_READ_UNTRACKED"] }).verdict).toBe("inconclusive");
    expect(judge({ unattributed: ["HUGE_FAN_OUT"] }).verdict).toBe("inconclusive");
  });

  it("does not hide a diagnostic Chrome did place at the finding (that still agrees)", () => {
    const r = row({ unattributed: ["HUGE_FAN_OUT"] });
    expect(classify({ sites, checker: chromeChecker(r), runtime: chromeRuntime(r, shown, sites), plan: { clicks: [] } }).verdict).toBe("agree");
  });
});

describe("a page error or a failed execution in Chrome", () => {
  it("makes the candidate inconclusive", () => {
    expect(judge({ pageErrors: ["Uncaught TypeError: x is not a function"] }).verdict).toBe("inconclusive");
    expect(judge({ verdict: "unproven-clean", failure: "the feedback run did not finish" }).verdict).toBe("inconclusive");
    expect(judge({ verdict: "unproven-clean", failure: { message: "timeout" } }).verdict).toBe("inconclusive");
  });
});

describe("chromeStatus", () => {
  it("confirms only on a Chrome false-positive candidate; agree refutes; anything undecided stays open", () => {
    expect(chromeStatus(null)).toBe("unconfirmed");
    expect(chromeStatus({ verdict: "candidate-false-positive" })).toBe("confirmed");
    expect(chromeStatus({ verdict: "agree" })).toBe("refuted-by-chrome");
    expect(chromeStatus({ verdict: "inconclusive" })).toBe("chrome-inconclusive");
  });
});
