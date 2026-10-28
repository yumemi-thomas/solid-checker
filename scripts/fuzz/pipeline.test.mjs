import { existsSync, mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { buildCase } from "./emit.mjs";
import { generateProgram } from "./generate.mjs";
import { evaluate, judgeEntries, lazyControl, signaturesOf } from "./pipeline.mjs";

// The wiring with fakes for the three expensive steps: the type checker, the checker, the Happy DOM batch.
const entry = (seed) => { const program = generateProgram(seed, { profile: "in-place" }); return { program, built: buildCase(program, { id: `fuzz-${seed}` }) }; };
const config = () => ({ install: mkdtempSync(join(tmpdir(), "fuzz-pipe-")), casesDir: mkdtempSync(join(tmpdir(), "fuzz-cases-")), concurrency: 2, workers: 1 });
const okTypes = async () => ({ ok: true, errors: [] });

describe("evaluate", () => {
  it("drops a program that does not type-check, before the checker and the runtime see it", async () => {
    const e = entry(1);
    let ran = false;
    const results = await evaluate([e], config(), {
      typecheck: async () => ({ ok: false, errors: ["App.tsx(1,1): error TS2322"] }),
      runChecker: async () => { ran = true; return { ok: true, findings: [] }; },
      runFast: async () => { ran = true; return new Map(); },
    });
    expect(ran).toBe(false);
    expect(results.get(e.built.id).classification).toMatchObject({ verdict: "type-error" });
    expect(existsSync(join(config().casesDir))).toBe(true);
  });

  it("passes only the cases the checker answered to the runtime batch, and classifies a checker failure as inconclusive", async () => {
    const [a, b] = [entry(2), entry(3)];
    let batch = [];
    const results = await evaluate([a, b], config(), {
      typecheck: okTypes,
      runChecker: async (dir) => (dir.endsWith(a.built.id) ? { ok: true, findings: [] } : { ok: false, findings: [], error: "timeout" }),
      runFast: async (cases) => { batch = cases.map((c) => c.id); return new Map(cases.map((c) => [c.id, { diagnostics: [], errors: [], witnesses: [] }])); },
    });
    expect(batch).toEqual([a.built.id]);
    expect(results.get(a.built.id).classification.verdict).toBe("agree");
    expect(results.get(b.built.id).classification).toMatchObject({ verdict: "inconclusive", reasons: [expect.stringMatching(/checker failed: timeout/)] });
  });

  it("finds a candidate false positive end to end, and gives it a signature", async () => {
    const e = entry(4);
    const [site] = e.built.sites;
    const results = await evaluate([e], config(), {
      typecheck: okTypes,
      runChecker: async () => ({ ok: true, findings: [{ file: "App.tsx", rule: site.kind === "write" ? "no-write-in-pure-computation" : "strict-read-untracked", id: "SC", kind: "violation", line: site.line, endLine: site.line }] }),
      runFast: async (cases) => new Map(cases.map((c) => [c.id, { diagnostics: [], errors: [], witnesses: e.built.sites.map((s) => ({ siteId: s.id, reached: true })) }])),
    });
    const result = results.get(e.built.id);
    expect(result.classification.verdict).toBe("candidate-false-positive");
    expect(result.verdicts[0]).toMatchObject({ siteId: site.id, verdict: "violation" });
    const [found] = signaturesOf(e.program, e.built, result.classification);
    expect(found.signature).toMatch(/^(no-write-in-pure-computation|strict-read-untracked)\|candidate-false-positive\|silent\|component\.body/);
  });
});

// The hit ledger in the wiring: the twin is type-checked and run, the checker reads the clean text, the controls compare.
describe("evaluate: the instrumented twin", () => {
  const records = (e, over = {}) => async (cases) => new Map(cases.map((c) => [c.id, {
    diagnostics: [], errors: [], witnesses: e.built.sites.map((s) => ({ siteId: s.id, reached: true, actual: s.witness?.text })), hits: Object.fromEntries(e.built.ledger.map((r) => [r.hit, 1])), quiescent: true, ...over,
  }]));

  it("runs the twin in the fast batch, type-checks both texts, and gives the checker only the clean text", async () => {
    const e = entry(6);
    const typed = []; const checked = []; let batch;
    await evaluate([e], config(), {
      typecheck: async (dir) => { typed.push(dir); return { ok: true, errors: [] }; },
      runChecker: async (dir) => { checked.push(dir); return { ok: true, findings: [] }; },
      runFast: async (cases) => { batch = cases; return records(e)(cases); },
    });
    expect(typed.map((d) => d.split("/").at(-1))).toEqual([e.built.id, `${e.built.id}-run`]);
    expect(checked.map((d) => d.split("/").at(-1))).toEqual([e.built.id]);
    expect(batch).toEqual([{ id: e.built.id, dir: expect.any(String), variant: "run" }]);
  });

  it("a twin that does not type-check is inconclusive, never a verdict", async () => {
    const e = entry(7);
    const results = await evaluate([e], config(), {
      typecheck: async (dir) => (dir.endsWith("-run") ? { ok: false, errors: ["App.tsx(3,1): error TS2695"] } : { ok: true, errors: [] }),
      runChecker: async () => ({ ok: true, findings: [] }),
      runFast: async () => { throw new Error("must not run"); },
    });
    expect(results.get(e.built.id).classification.items[0]).toMatchObject({ kind: "inconclusive", why: "twin-type-error" });
  });

  it("a non-neutral twin is refused before it runs", async () => {
    const e = entry(8);
    e.built.neutral = false;
    const results = await evaluate([e], config(), { typecheck: okTypes, runChecker: async () => ({ ok: true, findings: [] }), runFast: async () => { throw new Error("must not run"); } });
    expect(results.get(e.built.id).classification.items[0]).toMatchObject({ why: "not-neutral" });
  });

  it("the baseline mode runs the clean text with no ledger and classifies as before", async () => {
    const e = entry(9);
    const [site] = e.built.sites;
    let batch;
    const results = await evaluate([e], { ...config(), instrument: false }, {
      typecheck: okTypes,
      runChecker: async () => ({ ok: true, findings: [{ file: "App.tsx", rule: "strict-read-untracked", id: "SC", kind: "violation", line: site.line, endLine: site.line }] }),
      runFast: async (cases) => { batch = cases; return new Map(cases.map((c) => [c.id, { diagnostics: [], errors: [], witnesses: e.built.sites.map((s) => ({ siteId: s.id, reached: true })) }])); },
    });
    expect(batch[0].variant).toBe("clean");
    expect(results.get(e.built.id).classification.verdict).toBe("candidate-false-positive");
  });

  it("the control run compares the clean twin; a moved outcome makes the program inconclusive", async () => {
    const e = entry(10);
    const same = records(e);
    const ids = [];
    const ok = await evaluate([e], { ...config(), control: true }, {
      typecheck: okTypes, runChecker: async () => ({ ok: true, findings: [] }),
      runFast: async (cases) => { ids.push(cases.map((c) => `${c.id}:${c.variant}`)); return same(cases); },
    });
    expect(ids).toEqual([[`${e.built.id}:run`], [`${e.built.id}-clean:clean`]]);
    expect(ok.get(e.built.id).control).toMatchObject({ matched: true, diffs: [] });
    const moved = await evaluate([e], { ...config(), control: true }, {
      typecheck: okTypes, runChecker: async () => ({ ok: true, findings: [] }),
      runFast: async (cases) => records(e, cases[0].id.endsWith("-clean") ? { diagnostics: [{ code: "STRICT_READ_UNTRACKED", positions: [{ line: 5 }] }] } : {})(cases),
    });
    expect(moved.get(e.built.id).classification.items[0]).toMatchObject({ kind: "inconclusive", why: "perturbed-run" });
  });

  it("the control also compares the checker on the twin's text with the clean findings", async () => {
    const e = entry(11);
    const f = (rule, line) => ({ file: "App.tsx", rule, id: "SC", kind: "violation", line, endLine: line });
    const results = await evaluate([e], { ...config(), control: true }, {
      typecheck: okTypes,
      runChecker: async (dir) => ({ ok: true, findings: dir.endsWith("-run") ? [f("strict-read-untracked", 9)] : [f("strict-read-untracked", 8)] }),
      runFast: records(e),
    });
    expect(results.get(e.built.id).control.diffs.join()).toMatch(/checker missing on the instrumented text/);
    expect(results.get(e.built.id).classification.items[0].why).toBe("perturbed-run");
  });

  it("a never-run payload site the ledger places is a candidate-unexecuted with a signature, and no readout keeps it inconclusive", async () => {
    const e = entry(14);
    const [site] = e.built.sites;
    const finding = { file: "App.tsx", rule: site.kind === "write" ? "no-write-in-pure-computation" : "strict-read-untracked", id: "SC", kind: "violation", line: site.line, endLine: site.line };
    const entries = Object.fromEntries(e.built.ledger.filter((row) => row.kind === "entry").map((row) => [row.hit, 1]));
    const record = (extra) => async (cases) => new Map(cases.map((c) => [c.id, { diagnostics: [], errors: [], witnesses: e.built.sites.map((s) => ({ siteId: s.id, reached: null })), quiescent: true, hits: entries, ...extra }]));
    const run = (runFast) => evaluate([e], config(), { typecheck: okTypes, runChecker: async () => ({ ok: true, findings: [finding] }), runFast });
    const result = (await run(record())).get(e.built.id);
    const item = result.classification.items.find((i) => i.finding);
    expect(item).toMatchObject({ kind: "candidate-unexecuted", status: "candidate-false-positive: unexecuted" });
    expect(result.classification.verdict).toBe("candidate-unexecuted");
    const [found] = signaturesOf(e.program, e.built, result.classification);
    expect(found.signature).toMatch(/candidate-unexecuted/);
    expect(found.site.id).toBe(site.id);
    // the page not shown quiet, or no component entered: the old inconclusive stays
    expect((await run(record({ quiescent: false }))).get(e.built.id).classification.items.find((i) => i.finding)).toMatchObject({ kind: "inconclusive", why: "never-ran" });
    expect((await run(record({ hits: {} }))).get(e.built.id).classification.items.find((i) => i.finding)).toMatchObject({ kind: "inconclusive", why: "never-ran" });
  });

  it("signatures: a glue-only candidate has one too", async () => {
    const e = entry(13);
    const glueLine = e.built.glue[0];
    const results = await evaluate([e], config(), {
      typecheck: okTypes,
      runChecker: async () => ({ ok: true, findings: [{ file: "App.tsx", rule: "strict-read-untracked", id: "SC", kind: "violation", line: glueLine.line, endLine: glueLine.line }] }),
      runFast: records(e, { witnesses: [] }),
    });
    const result = results.get(e.built.id);
    const candidates = result.classification.items.filter((i) => i.kind === "candidate-false-positive");
    if (!e.built.sites.some((s) => s.line === glueLine.line)) {
      expect(candidates.length).toBe(1);
      expect(signaturesOf(e.program, e.built, result.classification)).toHaveLength(1);
    }
  });
});

// The shrink's control: asked of the candidates the plain pass accepts, and the answers equal the every-round control's.
describe("judgeEntries: the control of a shrink", () => {
  const records = (e, over = {}) => async (cases) => new Map(cases.map((c) => [c.id, {
    diagnostics: [], errors: [], witnesses: e.built.sites.map((s) => ({ siteId: s.id, reached: true, actual: s.witness?.text })), hits: Object.fromEntries(e.built.ledger.map((r) => [r.hit, 1])), quiescent: true, ...over,
  }]));
  // A fake that reports a violation at the first site of every program and, on the clean twin of chosen ids, a diagnostic that moves the outcome.
  const world = (entries, perturbed) => {
    const calls = { fast: [] };
    const deps = {
      typecheck: okTypes,
      runChecker: async (dir) => {
        const e = entries.find(({ built }) => dir.endsWith(built.id) || dir.endsWith(`${built.id}-run`));
        const [site] = e.built.sites;
        return { ok: true, findings: [{ file: "App.tsx", rule: "strict-read-untracked", id: "SC", kind: "violation", line: site.line, endLine: site.line }] };
      },
      runFast: async (cases) => {
        calls.fast.push(cases.map((c) => `${c.id}:${c.variant}`));
        const out = new Map();
        for (const c of cases) {
          const e = entries.find(({ built }) => c.id.startsWith(built.id));
          const moved = c.variant === "clean" && perturbed.has(e.built.id);
          for (const [id, rec] of await records(e, moved ? { diagnostics: [{ code: "STRICT_READ_UNTRACKED", positions: [{ line: 5 }] }] } : {})([c])) out.set(id, rec);
        }
        return out;
      },
    };
    return { deps, calls };
  };
  const holds = (_entry, result) => result.classification.items.some((item) => item.kind === "candidate-false-positive");

  it("answers as the every-round control does, whichever candidates the control refuses", async () => {
    const entries = [entry(21), entry(22), entry(23), entry(24)];
    for (const refused of [[], [entries[0].built.id], [entries[0].built.id, entries[1].built.id], entries.map((e) => e.built.id)]) {
      const each = world(entries, new Set(refused));
      const lazy = world(entries, new Set(refused));
      const old = await judgeEntries(entries, { ...config(), control: true, shrinkControl: "each" }, holds, each.deps);
      const now = await judgeEntries(entries, { ...config(), control: true }, holds, lazy.deps);
      // The answers agree up to the first true: the candidates after the one taken are unreached.
      const taken = old.indexOf(true);
      expect(now.slice(0, taken < 0 ? undefined : taken + 1)).toEqual(old.slice(0, taken < 0 ? undefined : taken + 1));
      expect(now.filter(Boolean).length).toBe(taken < 0 ? 0 : 1);
    }
  });

  it("runs the control only for the candidates the plain pass accepts, and not past the first that passes it", async () => {
    const entries = [entry(25), entry(26), entry(27)];
    const { deps, calls } = world(entries, new Set([entries[0].built.id]));
    const answers = await judgeEntries(entries, { ...config(), control: true }, holds, deps);
    expect(answers).toEqual([false, true, false]);
    // One plain batch (the twins), then the controlled run of candidate 0 (refused) and of candidate 1 (taken); candidate 2 is never run again.
    expect(calls.fast.filter((batch) => batch.some((call) => call.endsWith(":clean"))).map((batch) => batch.map((call) => call.split(":")[0]))).toEqual([[`${entries[0].built.id}-clean`], [`${entries[1].built.id}-clean`]]);
  });

  it("keeps the control in every round without a control, or when asked", () => {
    expect(lazyControl({ control: true })).toBe(true);
    expect(lazyControl({ control: false })).toBe(false);
    expect(lazyControl({ control: true, shrinkControl: "each" })).toBe(false);
  });
});
