import { describe, expect, it } from "vitest";
import { chromeChecker, chromeDiagnostics, chromeRuntime } from "./chrome.mjs";
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
