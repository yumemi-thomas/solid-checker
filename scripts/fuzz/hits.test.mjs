import { describe, expect, it } from "vitest";
import { buildCase } from "./emit.mjs";
import { generateProgram } from "./generate.mjs";
import {
  INSTRUMENT_PRELUDE, ORDER_LIMIT, PRELUDE_MARK, compareVerdictRuns, counterIds, findingsDiff, hitRecord, hitSummary, normalizeHits, outcomeDiff, outcomeOf, parseHitsText, stripInstrumentation,
} from "./hits.mjs";
import { printProgram } from "./print.mjs";

const SEEDS = Array.from({ length: 400 }, (_, i) => i + 1);
const PROFILES = [undefined, "body-call", "default-param", "helper-closure", "async", "in-place"];

describe("the instrumented twin (neutrality, part a)", () => {
  it("strip(runText) equals the clean text, line for line, on every seed and profile", () => {
    for (const profile of PROFILES) {
      for (const seed of SEEDS) {
        const { text, runText } = printProgram(generateProgram(seed, { profile }));
        expect(stripInstrumentation(runText), `seed ${seed} ${profile}`).toBe(text);
      }
    }
  });

  it("moves no line: the prelude is appended after the last line", () => {
    for (const seed of SEEDS.slice(0, 100)) {
      const { text, runText } = printProgram(generateProgram(seed));
      expect(runText.startsWith(text)).toBe(false); // counters are inline
      const body = runText.slice(0, runText.indexOf(PRELUDE_MARK));
      expect(body.split("\n").length).toBe(text.split("\n").length);
      expect(runText.endsWith(INSTRUMENT_PRELUDE)).toBe(true);
    }
  });

  it("is deterministic and clean text is the one the printer always produced", () => {
    for (const seed of [1, 7, 99]) {
      const program = generateProgram(seed);
      expect(printProgram(program)).toEqual(printProgram(structuredClone(program)));
      expect(printProgram(program, { instrument: false }).text).toBe(printProgram(program).text);
      expect(printProgram(program, { instrument: false }).runText).toBe(printProgram(program).text);
    }
  });

  it("a counter left behind is caught: a stripped text with a residue differs from the clean one", () => {
    const { text, runText } = printProgram(generateProgram(5));
    expect(stripInstrumentation(runText.replace("{ __h(1);", "{ __h(1); __h(1);"))).not.toBe(text);
    expect(stripInstrumentation(runText.replace("\n  return", "\n  __h(1);\n  return"))).not.toBe(text);
  });

  it("every ledger id appears once, on the line the ledger says", () => {
    for (const seed of SEEDS.slice(0, 120)) {
      const { runText, ledger, sites, glue } = printProgram(generateProgram(seed));
      const ids = counterIds(runText);
      expect(new Set(ids).size, `seed ${seed}`).toBe(ids.length);
      expect([...ids].sort((a, b) => a - b)).toEqual(ledger.map((row) => row.hit));
      const lines = runText.split("\n");
      for (const row of ledger) expect(lines[row.line - 1], `seed ${seed} hit ${row.hit}`).toContain(`__h(${row.hit})`);
      // Payload sites keep the line the clean print gave them, now with a counter on it.
      for (const site of sites) expect(lines[site.line - 1]).toMatch(new RegExp(`__h\\(${site.hit}\\)(?:, |; )`));
      expect(glue.map((g) => g.id)).toEqual(glue.map((_, i) => `a${i + 1}`));
    }
  });
});

describe("what is counted", () => {
  it("counts every payload site, every emitted call (glue) and every function entry", () => {
    const { runText, sites, glue, ledger } = printProgram(generateProgram(3668339988 % 100000, { profile: "body-call" }));
    expect(sites.length).toBeGreaterThan(0);
    expect(glue.length).toBeGreaterThan(0);
    // The plumbing line the old comparator was blind to is a glue site now: `m1()()` is a call in the component body.
    const plumbing = glue.filter((g) => /__h\(\d+\); (?:const n\d = )?m\d\(\)\(\);/.test(runText.split("\n")[g.line - 1]));
    expect(plumbing.length).toBeGreaterThan(0);
    // ...counted at the chain's top, so the chain itself is left as the checker would see it unmarked.
    expect(runText).not.toMatch(/\(__h\(\d+\), m\d\(\)\)\(\)/);
    expect(runText).not.toMatch(/\(__h\(\d+\), m\d\(\)\)\.f/);
    const kinds = new Set(ledger.map((row) => row.kind));
    expect([...kinds].sort()).toEqual(["auto", "entry", "site"]);
    expect(ledger.some((row) => row.kind === "entry" && row.node === "component")).toBe(true);
    expect(ledger.some((row) => row.kind === "entry" && row.node === "fn")).toBe(true);
  });

  it("across seeds, counts handlers and defaults, and marks a counter in a parameter default", () => {
    const rows = SEEDS.flatMap((seed) => printProgram(generateProgram(seed)).ledger);
    expect(rows.some((row) => row.kind === "entry" && row.node === "handler")).toBe(true);
    expect(rows.some((row) => row.inDefault)).toBe(true);
    // A component's entry has no enclosing function; everything else has one.
    expect(rows.filter((row) => row.enclosing === null).every((row) => row.node === "component")).toBe(true);
  });

  it("the type-relevant shape stays: the twin declares __h and nothing else new at the top level", () => {
    const { runText } = printProgram(generateProgram(2));
    expect(runText.slice(runText.indexOf(PRELUDE_MARK))).toMatch(/^\/\/ __sc_hits__\nfunction __h\(id: number\): void/);
    expect(runText.match(/^function |^const |^import /gm).length).toBeGreaterThan(2);
  });
});

describe("the readout", () => {
  it("parses #sc-hits text and rejects what is not one", () => {
    expect(parseHitsText('{"h":{"1":2,"7":1},"o":[1,7,1]}')).toEqual({ hits: { 1: 2, 7: 1 }, order: [1, 7, 1] });
    expect(parseHitsText("{}")).toEqual({ hits: {}, order: [] });
    for (const bad of ["", "  ", "nope", "[1]", null, undefined, '{"h":3}']) expect(parseHitsText(bad)).toBeNull();
    expect(normalizeHits({ 3: 1, x: 2, 4: "z" })).toEqual({ 3: 1 });
  });

  it("the counter itself is plain DOM and globalThis: no Solid import, bounded order log, a visible 1px readout", () => {
    expect(INSTRUMENT_PRELUDE).not.toMatch(/solid|createSignal|getOwner|getObserver/);
    expect(INSTRUMENT_PRELUDE).toContain(`order.length < ${ORDER_LIMIT}`);
    expect(INSTRUMENT_PRELUDE).toContain('id = "sc-hits"');
    expect(INSTRUMENT_PRELUDE).not.toMatch(/setAttribute\("hidden"|display:\s*none|visibility/);
  });

  it("the counter, run for real, counts and mirrors into the element", () => {
    const store = { text: "" };
    const element = { id: "", setAttribute() {}, get textContent() { return store.text; }, set textContent(v) { store.text = v; } };
    const body = { append(e) { store.element = e; } };
    const document = { getElementById: () => store.element ?? null, createElement: () => element, body };
    const js = INSTRUMENT_PRELUDE.slice(PRELUDE_MARK.length).replace(/globalThis as unknown as \{[^}]*\}/g, "globalThis").replace(/: number|: void/g, "");
    // eslint-disable-next-line no-new-func
    const run = new Function("document", "globalThis", `${js}; return __h;`);
    const g = {};
    const h = run(document, g);
    h(4); h(4); h(9);
    expect(g.__scHits).toEqual({ 4: 2, 9: 1 });
    expect(parseHitsText(store.text)).toEqual({ hits: { 4: 2, 9: 1 }, order: [4, 4, 9] });
  });

  it("records ran and landed separately", () => {
    const ledger = [{ hit: 1, ref: "s1", kind: "site", node: "write", line: 5 }, { hit: 2, ref: "a1", kind: "auto", node: "call", line: 6 }, { hit: 3, ref: "e3", kind: "entry", node: "fn", line: 4 }];
    const record = hitRecord(ledger, { 1: 2, 3: 1 }, [{ siteId: "s1", reached: false }]);
    expect(record).toEqual([
      { hit: 1, ref: "s1", kind: "site", node: "write", line: 5, hits: 2, ran: true, landed: false },
      { hit: 2, ref: "a1", kind: "auto", node: "call", line: 6, hits: 0, ran: false },
      { hit: 3, ref: "e3", kind: "entry", node: "fn", line: 4, hits: 1, ran: true },
    ]);
    expect(hitRecord(ledger, null)[0]).toMatchObject({ hits: null, ran: null });
    expect(hitSummary(ledger, { 1: 1 }, [], true)).toMatchObject({ instrumented: true, quiescent: true, glue: { ran: 0, total: 1 }, entries: { ran: 0, total: 1 } });
  });
});

describe("outcomes and findings the twin must not move (parts b and chrome)", () => {
  const runtime = (over = {}) => ({ failure: null, diagnostics: [{ code: "STRICT_READ_UNTRACKED", positions: [{ line: 8 }] }], errors: [], witnesses: [{ siteId: "s1", actual: "17" }], ...over });
  it("equal outcomes have no diff, whatever the order of diagnostics or the extra fields", () => {
    expect(outcomeDiff(runtime(), runtime({ hits: { 1: 1 }, quiescent: true }))).toEqual([]);
    const two = [{ code: "A", positions: [{ line: 1 }] }, { code: "B", positions: [{ line: 2 }] }];
    expect(outcomeDiff(runtime({ diagnostics: two }), runtime({ diagnostics: [...two].reverse() }))).toEqual([]);
  });
  it("names a moved diagnostic, throw, witness or failure", () => {
    expect(outcomeDiff(runtime(), runtime({ diagnostics: [] })).join()).toMatch(/diagnostics/);
    expect(outcomeDiff(runtime(), runtime({ diagnostics: [{ code: "STRICT_READ_UNTRACKED", positions: [{ line: 9 }] }] })).join()).toMatch(/diagnostics/);
    expect(outcomeDiff(runtime({ errors: [{ kind: "handler", message: "x" }] }), runtime()).join()).toMatch(/errors/);
    expect(outcomeDiff(runtime(), runtime({ witnesses: [{ siteId: "s1", actual: null }] })).join()).toMatch(/witness s1/);
    expect(outcomeDiff(runtime({ failure: { phase: "render" } }), runtime()).join()).toMatch(/failure/);
    expect(outcomeOf(runtime()).diagnostics).toEqual({ "STRICT_READ_UNTRACKED@8": 1 });
  });
  it("compares findings as (rule, kind, line) sets", () => {
    const f = (rule, line, kind = "violation") => ({ rule, kind, line, endLine: line + 3 });
    expect(findingsDiff([f("a", 1), f("b", 2)], [f("b", 2), f("a", 1)])).toEqual([]);
    expect(findingsDiff([f("a", 1)], [f("a", 1, "uncertifiable")])).toHaveLength(2);
    expect(findingsDiff([f("a", 1)], [])).toEqual(["missing on the instrumented text: a|violation|1"]);
    expect(findingsDiff([], [f("a", 1)])).toEqual(["new on the instrumented text: a|violation|1"]);
  });
});

describe("compareVerdictRuns (the regression harness's rule)", () => {
  it("lets an inconclusive resolve and nothing else move", () => {
    const base = { a: "agree", b: "inconclusive", c: "inconclusive", d: "candidate-false-positive", e: "agree" };
    const instr = { a: "agree", b: "agree", c: "inconclusive", d: "agree", e: "inconclusive" };
    const result = compareVerdictRuns(base, instr);
    expect(result.same).toBe(2);
    expect(result.resolved).toEqual([{ id: "b", from: "inconclusive", to: "agree" }]);
    expect(result.failures).toEqual([{ id: "d", from: "candidate-false-positive", to: "agree" }, { id: "e", from: "agree", to: "inconclusive" }]);
    expect(result.equal).toBe(false);
  });
  it("is equal when only inconclusives resolve, and treats a missing program as a failure", () => {
    expect(compareVerdictRuns(new Map([["a", "inconclusive"]]), new Map([["a", "candidate-false-positive"]])).equal).toBe(true);
    expect(compareVerdictRuns({ a: "agree" }, {}).missing).toEqual(["a"]);
    expect(compareVerdictRuns({ a: "agree" }, {}).equal).toBe(false);
  });
});

describe("a built case", () => {
  it("is neutral, and its files are the clean text, the twin and both step lists", () => {
    const built = buildCase(generateProgram(11), { id: "fuzz-11" });
    expect(built.neutral).toBe(true);
    expect(built.files["App.tsx"]).toBe(built.text);
    expect(built.files["App.run.txt"]).toBe(built.runText);
    const clean = JSON.parse(built.files["oracle.json"]).steps;
    const run = JSON.parse(built.files["oracle.run.json"]).steps;
    expect(clean.some((s) => s.selector === "#sc-hits")).toBe(false);
    expect(run.slice(0, clean.length)).toEqual(clean);
    expect(run.slice(clean.length)).toEqual([{ action: "wait-for-selector", selector: "#sc-hits" }, { action: "wait-for-selector", selector: "#sc-hits" }]);
    const sitesJson = JSON.parse(built.files["sites.json"]);
    expect(sitesJson.glue).toEqual(built.glue);
    expect(sitesJson.ledger).toEqual(built.ledger);
  });
});
