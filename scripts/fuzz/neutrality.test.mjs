import { describe, expect, it } from "vitest";
import { BLIND, blindFindingStats, compareRuns } from "./neutrality.mjs";

const row = (id, verdict, items = [], extra = {}) => ({ id, verdict, items, ...extra });
const blind = (rule, line) => ({ kind: "inconclusive", reason: BLIND, finding: { rule, line } });
const resolved = (kind, rule, line) => ({ kind, reason: "x", finding: { rule, line } });

describe("blindFindingStats", () => {
  it("counts the findings that used to span no site and now map to one", () => {
    const base = [row("a", "inconclusive", [blind("strict-read-untracked", 12)]), row("b", "inconclusive", [blind("strict-read-untracked", 7), blind("no-destructure", 9)]), row("c", "agree")];
    const instr = [
      row("a", "candidate-false-positive", [resolved("candidate-false-positive", "strict-read-untracked", 12)]),
      row("b", "inconclusive", [resolved("inconclusive", "strict-read-untracked", 7), blind("no-destructure", 9)]),
      row("c", "agree"),
    ];
    expect(blindFindingStats(base, instr)).toEqual({ before: 3, stillBlind: 1, mapped: 2, mappedTo: { "candidate-false-positive": 1, inconclusive: 1 } });
  });
  it("a finding the instrumented run does not mention is not counted as mapped", () => {
    expect(blindFindingStats([row("a", "inconclusive", [blind("r", 3)])], [row("a", "agree")])).toMatchObject({ before: 1, mapped: 0, stillBlind: 0 });
  });
});

describe("compareRuns", () => {
  it("is equal when only inconclusives resolve, and reports the control", () => {
    const base = [row("a", "inconclusive"), row("b", "agree")];
    const instr = [row("a", "agree", [], { control: { matched: true, diffs: [] } }), row("b", "agree", [], { control: { matched: false, diffs: ["diagnostics: x"] } })];
    const report = compareRuns(base, instr);
    expect(report).toMatchObject({ programs: 2, sameVerdict: 1, equal: true, control: { programs: 2, matched: 1, mismatches: [{ id: "b", diffs: ["diagnostics: x"] }] } });
    expect(report.resolved).toEqual([{ id: "a", from: "inconclusive", to: "agree" }]);
  });
  it("fails on any other movement", () => {
    const report = compareRuns([row("a", "agree"), row("b", "candidate-false-positive")], [row("a", "inconclusive"), row("b", "agree")]);
    expect(report.equal).toBe(false);
    expect(report.failures.map((f) => f.id)).toEqual(["a", "b"]);
  });
});

// The round's mechanics with a fake pipeline: caching, ordering and stopping change what is run, never a verdict.
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { BaselineCache, MismatchHistory } from "./cache.mjs";
import { runProfile } from "./neutrality.mjs";

describe("runProfile", () => {
  const options = { seed: 1, count: 6, chunk: 2 };
  const fakeEvaluate = (log, { moveIn } = {}) => async (entries, config) => {
    log.push({ instrument: config.instrument !== false, ids: entries.map((e) => e.built.id) });
    return new Map(entries.map(({ program, built }) => [built.id, {
      program, built, classification: { verdict: config.instrument === false ? "inconclusive" : (moveIn === built.id ? "agree" : "inconclusive"), reasons: [], items: [] },
    }]));
  };
  const scratch = () => mkdtempSync(join(tmpdir(), "fuzz-round-"));

  it("a second round reuses the cached baseline and runs only the instrumented pass, with the same verdicts", async () => {
    const dir = scratch();
    const run = async () => {
      const log = [];
      const cache = new BaselineCache(dir, "e".repeat(64));
      const result = await runProfile({ options, profile: "body-call", config: { concurrency: 1 }, cache, history: null, out: dir, evaluateFn: fakeEvaluate(log) });
      return { log, result, cache };
    };
    const first = await run();
    expect(first.log.filter((c) => !c.instrument).flatMap((c) => c.ids)).toHaveLength(6);
    const second = await run();
    expect(second.log.filter((c) => !c.instrument)).toEqual([]);
    expect(second.log.filter((c) => c.instrument).flatMap((c) => c.ids)).toHaveLength(6);
    expect(second.cache.hits).toBe(6);
    expect(second.result.report).toEqual(first.result.report);
    expect(second.result.baseRows.map((r) => r.verdict)).toEqual(first.result.baseRows.map((r) => r.verdict));
  });

  it("a cache from another environment key is not used", async () => {
    const dir = scratch();
    const log = [];
    await runProfile({ options, profile: "mixed", config: {}, cache: new BaselineCache(dir, "a".repeat(64)), history: null, out: dir, evaluateFn: fakeEvaluate([]) });
    await runProfile({ options, profile: "mixed", config: {}, cache: new BaselineCache(dir, "b".repeat(64)), history: null, out: dir, evaluateFn: fakeEvaluate(log) });
    expect(log.filter((c) => !c.instrument).flatMap((c) => c.ids)).toHaveLength(6);
  });

  it("failing-first runs the programs that mismatched last time in the first chunk; stop-on-mismatch ends the round there", async () => {
    const dir = scratch();
    const history = new MismatchHistory(dir);
    // Program seeds as the generator makes them; pretend the third mismatched last time.
    const { generateEntries } = await import("./run.mjs");
    const entries = generateEntries({ ...options, profile: "async" });
    const bad = entries[3];
    history.record("programs", `async|1|${bad.program.seed}`, true);
    // The baseline says agree; the instrumented pass moves `bad` to inconclusive: not an allowed move.
    const evaluateFn = async (list, config) => new Map(list.map(({ program, built }) => [built.id, {
      program, built, classification: { verdict: config.instrument === false ? "agree" : (built.id === bad.built.id ? "inconclusive" : "agree"), reasons: [], items: [] },
    }]));
    const result = await runProfile({ options: { ...options, stopOnMismatch: true }, profile: "async", config: {}, cache: null, history, out: dir, evaluateFn });
    expect(result.stopped).toBe(true);
    expect(result.ranPrograms).toBe(2);
    expect(result.instrRows[0].id).toBe(bad.built.id);
    expect(result.report.failures.map((f) => f.id)).toEqual([bad.built.id]);
  });
});
