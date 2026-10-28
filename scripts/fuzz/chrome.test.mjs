import { describe, expect, it } from "vitest";
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { EntryCache } from "./cache.mjs";
import { CLEAN_ID, cacheableChrome, chromeChecker, chromeDiagnostics, chromeRuntime, chromeStatus, isCloseTimeout, runChrome } from "./chrome.mjs";
import { HALT_TEXT, haltable } from "./hits.mjs";
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

describe("the hit readout from Chrome's checkpoints", () => {
  const hitsText = '{"h":{"4":1,"7":3},"o":[4,7,7,7]}';
  it("reads the last #sc-hits checkpoint, and quiescence from two equal reads", () => {
    const still = chromeRuntime(row(), [{ selector: "#sc-hits", text: hitsText }, { selector: "#sc-hits", text: hitsText }], sites);
    expect(still.hits).toEqual({ 4: 1, 7: 3 });
    expect(still.order).toEqual([4, 7, 7, 7]);
    expect(still.quiescent).toBe(true);
    const moving = chromeRuntime(row(), [{ selector: "#sc-hits", text: "{}" }, { selector: "#sc-hits", text: hitsText }], sites);
    expect(moving.quiescent).toBe(false);
  });
  it("is null without a readout (a clean control run, or an unreadable checkpoint)", () => {
    expect(chromeRuntime(row(), [], sites)).toMatchObject({ hits: null, quiescent: null });
    expect(chromeRuntime(row(), [{ selector: "#sc-hits", text: "garbage" }], sites).hits).toBeNull();
  });
  it("the counter decides 'ran' in Chrome too: a void site that ran, with Chrome silent, is a candidate", () => {
    const void1 = [{ id: "s1", line: 8, kind: "write", hit: 4, witness: null }];
    const quiet = row({ diagnostics: [] });
    const cps = [{ selector: "#sc-hits", text: hitsText }, { selector: "#sc-hits", text: hitsText }];
    expect(classify({ sites: void1, checker: chromeChecker(quiet), runtime: chromeRuntime(quiet, cps, void1), plan: { clicks: [] } }).verdict).toBe("candidate-false-positive");
  });
});

describe("the checker on the instrumented text must give the clean findings", () => {
  const finding = (rule, line, kind = "violation") => ({ file: "App.tsx", rule, id: "SC", kind, line, endLine: line });
  it("equal findings are not perturbed; a different (rule, kind, line) set is", () => {
    const r = row({ diagnostics: [], findings: [finding("no-write-in-pure-computation", 8)] });
    expect(chromeChecker(r, [finding("no-write-in-pure-computation", 8)]).perturbed).toEqual([]);
    expect(chromeChecker(r).perturbed).toEqual([]);
    const moved = chromeChecker(r, [finding("no-write-in-pure-computation", 8), finding("strict-read-untracked", 12)]);
    expect(moved.perturbed).toEqual(["missing on the instrumented text: strict-read-untracked|violation|12"]);
  });
  it("a missing clean reference (null) is a perturbation too; none given (undefined) compares nothing", () => {
    const r = row({ diagnostics: [], findings: [finding("no-write-in-pure-computation", 8)] });
    expect(chromeChecker(r, null).perturbed).toHaveLength(1);
    expect(chromeChecker(r, undefined).perturbed).toEqual([]);
  });
  it("a perturbed checker makes the candidate inconclusive, never confirmed", () => {
    const r = row({ diagnostics: [], findings: [finding("no-write-in-pure-computation", 8)] });
    const checker = chromeChecker(r, [finding("no-write-in-pure-computation", 8), finding("strict-read-untracked", 12, "uncertifiable")]);
    const judged = classify({ sites, checker, runtime: chromeRuntime(r, shown, sites), plan: { clicks: [] } });
    expect(judged.verdict).toBe("inconclusive");
    expect(judged.reasons[0]).toMatch(/instrumentation-perturbed-checker/);
    expect(chromeStatus(judged)).toBe("chrome-inconclusive");
  });
});

// runChrome against a fake oracle (a script with the oracle's command line that writes rows from a plan in the environment).
describe("runChrome: one call for the twin and the clean control, and one retry for a close timeout", () => {
  const fakeOracle = (dir) => {
    const path = join(dir, "oracle.mjs");
    writeFileSync(path, `import { appendFileSync, readFileSync, readdirSync, writeFileSync, existsSync } from "node:fs";
import { join } from "node:path";
const arg = (name) => process.argv[process.argv.indexOf(name) + 1];
const cases = arg("--cases"), only = process.argv.includes("--only") ? arg("--only").split(",") : null;
const plan = JSON.parse(readFileSync(process.env.FAKE_PLAN, "utf8"));
const ids = readdirSync(cases).filter((id) => !only || only.includes(id)).sort();
appendFileSync(process.env.FAKE_LOG, JSON.stringify({ ids, twin: ids.map((id) => readFileSync(join(cases, id, "App.tsx"), "utf8").trim()) }) + "\\n");
const attempt = existsSync(process.env.FAKE_LOG) ? readFileSync(process.env.FAKE_LOG, "utf8").trim().split("\\n").length : 1;
const results = ids.map((id) => {
  const row = (plan[id] ?? {})[Math.min(attempt, (plan[id] ?? [{}]).length) - 1] ?? {};
  return { id, verdict: "static-only", findings: [], diagnostics: [], pageErrors: [], ...row };
});
writeFileSync(arg("--out"), JSON.stringify({ results }));
`);
    return path;
  };
  const setup = (plan) => {
    const dir = mkdtempSync(join(tmpdir(), "fuzz-chrome-"));
    const caseDir = join(dir, "case");
    mkdirSync(caseDir);
    writeFileSync(join(caseDir, "App.tsx"), "clean");
    writeFileSync(join(caseDir, "App.run.txt"), "twin");
    writeFileSync(join(caseDir, "oracle.json"), "{}");
    writeFileSync(join(caseDir, "oracle.run.json"), "{}");
    writeFileSync(join(dir, "plan.json"), JSON.stringify(plan));
    const log = join(dir, "log.jsonl");
    return { dir, caseDir, log, env: { ...process.env, FAKE_PLAN: join(dir, "plan.json"), FAKE_LOG: log }, oracle: fakeOracle(dir), install: join(dir, "install") };
  };
  const finding = { file: "App.tsx", rule: "r", id: "SC1", kind: "violation", line: 3, endLine: 3 };
  const closeTimeout = { verdict: "harness-error", error: "Feedback resource cleanup failed: browser close timed out after 2000 ms" };

  it("recognizes a close timeout and nothing else", () => {
    expect(isCloseTimeout(closeTimeout)).toBe(true);
    expect(isCloseTimeout({ verdict: "harness-error", error: "boom" })).toBe(false);
    expect(isCloseTimeout({ verdict: "static-only", error: "browser close timed out after 2000 ms" })).toBe(false);
  });

  it("runs the twin and the clean case in one oracle call, and compares the twin's findings with the clean case's", async () => {
    const t = setup({ "fuzz-1": [{ findings: [finding] }], [CLEAN_ID("fuzz-1")]: [{ findings: [{ ...finding, line: 4, endLine: 4 }] }] });
    const records = await runChrome([{ id: "fuzz-1", dir: t.caseDir, sites: [] }], { out: t.dir, install: t.install, bin: "b", typefacts: "t", withClean: true, oracle: t.oracle, env: t.env });
    const calls = readFileSync(t.log, "utf8").trim().split("\n").map((line) => JSON.parse(line));
    expect(calls).toHaveLength(1);
    expect(calls[0]).toEqual({ ids: ["fuzz-1", CLEAN_ID("fuzz-1")], twin: ["twin", "clean"] });
    expect(records.get("fuzz-1").checker.perturbed.join()).toMatch(/./);
    const same = setup({ "fuzz-1": [{ findings: [finding] }], [CLEAN_ID("fuzz-1")]: [{ findings: [finding] }] });
    const equal = await runChrome([{ id: "fuzz-1", dir: same.caseDir, sites: [] }], { out: same.dir, install: same.install, bin: "b", typefacts: "t", withClean: true, oracle: same.oracle, env: same.env });
    expect(equal.get("fuzz-1").checker).toMatchObject({ ok: true, perturbed: [] });
  });

  it("runs a case that ended in a close timeout once more, alone, and keeps the second row", async () => {
    const t = setup({ "fuzz-1": [closeTimeout, { findings: [finding] }], "fuzz-2": [{}] });
    const records = await runChrome([{ id: "fuzz-1", dir: t.caseDir, sites: [] }, { id: "fuzz-2", dir: t.caseDir, sites: [] }], { out: t.dir, install: t.install, bin: "b", typefacts: "t", oracle: t.oracle, env: t.env });
    const calls = readFileSync(t.log, "utf8").trim().split("\n").map((line) => JSON.parse(line));
    expect(calls.map((call) => call.ids)).toEqual([["fuzz-1", "fuzz-2"], ["fuzz-1"]]);
    expect(records.get("fuzz-1")).toMatchObject({ retries: 1, checker: { ok: true, findings: [finding] } });
    expect(records.get("fuzz-2").retries).toBeUndefined();
  });

  it("does not retry more than asked, and a case that keeps failing keeps its row", async () => {
    const t = setup({ "fuzz-1": [closeTimeout] });
    const records = await runChrome([{ id: "fuzz-1", dir: t.caseDir, sites: [] }], { out: t.dir, install: t.install, bin: "b", typefacts: "t", oracle: t.oracle, env: t.env });
    expect(readFileSync(t.log, "utf8").trim().split("\n")).toHaveLength(2);
    expect(records.get("fuzz-1").checker.ok).toBe(false);
    const none = setup({ "fuzz-1": [closeTimeout] });
    await runChrome([{ id: "fuzz-1", dir: none.caseDir, sites: [] }], { out: none.dir, install: none.install, bin: "b", typefacts: "t", closeRetries: 0, oracle: none.oracle, env: none.env });
    expect(readFileSync(none.log, "utf8").trim().split("\n")).toHaveLength(1);
  });
});

describe("runChrome: the Chrome cache", () => {
  const fakeOracle = (dir) => {
    const path = join(dir, "oracle.mjs");
    writeFileSync(path, `import { appendFileSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
const arg = (name) => process.argv[process.argv.indexOf(name) + 1];
const ids = readdirSync(arg("--cases")).sort();
appendFileSync(process.env.FAKE_LOG, JSON.stringify(ids) + "\\n");
const plan = JSON.parse(readFileSync(process.env.FAKE_PLAN, "utf8"));
writeFileSync(arg("--out"), JSON.stringify({ results: ids.map((id) => ({ id, verdict: "static-only", findings: [], diagnostics: [], pageErrors: [], ...(plan[id] ?? {}) })) }));
`);
    return path;
  };
  const setup = (plan = {}) => {
    const dir = mkdtempSync(join(tmpdir(), "fuzz-chrome-cache-"));
    const caseDir = join(dir, "case");
    mkdirSync(caseDir);
    writeFileSync(join(caseDir, "App.tsx"), "clean");
    writeFileSync(join(dir, "plan.json"), JSON.stringify(plan));
    const log = join(dir, "log.jsonl");
    writeFileSync(log, "");
    const cache = new EntryCache(join(dir, "cache"), "chrome", "e".repeat(64));
    const options = { out: dir, install: join(dir, "install"), bin: "b", typefacts: "t", control: true, oracle: fakeOracle(dir), env: { ...process.env, FAKE_PLAN: join(dir, "plan.json"), FAKE_LOG: log }, cache };
    const calls = () => readFileSync(log, "utf8").trim().split("\n").filter(Boolean).map((line) => JSON.parse(line));
    return { dir, caseDir, options, calls, cache };
  };

  it("answers a case Chrome already ran from the cache, says so, and runs a changed case", async () => {
    const t = setup({ "fuzz-1": { diagnostics: [{ code: "NO_OWNER_CLEANUP", file: "App.tsx", line: 2 }] } });
    const first = await runChrome([{ id: "fuzz-1", dir: t.caseDir, sites: [] }], t.options);
    expect(first.get("fuzz-1").cached).toBe(false);
    const second = await runChrome([{ id: "fuzz-1", dir: t.caseDir, sites: [] }], t.options);
    expect(t.calls()).toEqual([["fuzz-1"]]);
    expect(second.get("fuzz-1")).toMatchObject({ cached: true, runtime: first.get("fuzz-1").runtime });
    writeFileSync(join(t.caseDir, "App.tsx"), "clean, edited");
    const third = await runChrome([{ id: "fuzz-1", dir: t.caseDir, sites: [] }], t.options);
    expect(third.get("fuzz-1").cached).toBe(false);
    expect(t.calls()).toEqual([["fuzz-1"], ["fuzz-1"]]);
    // Another mode of the same files is another key.
    await runChrome([{ id: "fuzz-1", dir: t.caseDir, sites: [] }], { ...t.options, control: false });
    expect(t.calls()).toHaveLength(3);
  });

  it("does not store a harness error or a page that never loaded; without a cache every call runs", async () => {
    const t = setup({ "fuzz-1": { verdict: "harness-error", error: "boom" }, "fuzz-2": { failure: { step: 0, message: "page.goto: Timeout 12000ms exceeded" } } });
    const cases = [{ id: "fuzz-1", dir: t.caseDir, sites: [] }, { id: "fuzz-2", dir: t.caseDir, sites: [] }];
    await runChrome(cases, t.options);
    await runChrome(cases, t.options);
    expect(t.calls()).toEqual([["fuzz-1", "fuzz-2"], ["fuzz-1", "fuzz-2"]]);
    expect(t.cache.stats().stored).toBe(0);
    const plain = setup();
    const { cache, ...options } = plain.options;
    const records = await runChrome([{ id: "fuzz-1", dir: plain.caseDir, sites: [] }], options);
    await runChrome([{ id: "fuzz-1", dir: plain.caseDir, sites: [] }], options);
    expect(plain.calls()).toHaveLength(2);
    expect(records.get("fuzz-1").cached).toBeUndefined();
    expect(cache).toBeTruthy();
  });
  it("a cacheable record is a row that is no harness error and a page that loaded", () => {
    expect(cacheableChrome({ row: { verdict: "static-only" }, runtime: { failure: null } })).toBe(true);
    expect(cacheableChrome({ row: { verdict: "static-only" }, runtime: { failure: { message: "locator.click: Timeout 12000ms exceeded" } } })).toBe(true);
    expect(cacheableChrome({ row: null, runtime: {} })).toBe(false);
    expect(cacheableChrome({ row: { verdict: "harness-error" }, runtime: {} })).toBe(false);
    expect(cacheableChrome({ row: { verdict: "static-only" }, runtime: { failure: { message: "Browser feedback bootstrap did not bind the loaded document within 12000 ms" } } })).toBe(false);
  });
});

describe("chromeRuntime: the halt marker", () => {
  const sites = [{ id: "s1", witness: { selector: "#w1", text: "w1:7" } }, { id: "s2", witness: { selector: "#w2", text: "w2:3" } }];
  it("reads a step that resolved to the marker as absent and the run as cut short; a run with no marker is unchanged", () => {
    const row = { id: "fuzz-1", verdict: "static-only", diagnostics: [], pageErrors: [] };
    const halted = chromeRuntime(row, [{ selector: haltable("#w1"), text: "w1:7" }, { selector: haltable("#w2"), text: HALT_TEXT }], sites);
    expect(halted.failure).toMatchObject({ phase: "halted" });
    expect(halted.witnesses).toEqual([
      { siteId: "s1", witness: sites[0].witness, present: true, actual: "w1:7", reached: true },
      { siteId: "s2", witness: sites[1].witness, present: false, actual: null, reached: null },
    ]);
    const plain = chromeRuntime(row, [{ selector: haltable("#w1"), text: "w1:7" }, { selector: "#w2", text: "w2:3" }], sites);
    expect(plain.failure).toBeNull();
    expect(plain.witnesses.map((w) => w.reached)).toEqual([true, true]);
    // A failure the oracle recorded stays the failure.
    expect(chromeRuntime({ ...row, failure: { message: "locator.click: Timeout" } }, [{ selector: haltable("#w2"), text: HALT_TEXT }], sites).failure.phase).toBe("execution");
  });
});
