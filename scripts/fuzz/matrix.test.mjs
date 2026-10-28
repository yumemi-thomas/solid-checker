import { describe, expect, it } from "vitest";
import { buildMatrix, codeStatuses, gate, generateShape, gridShapes, judge, markdown, regionOf, shapeSeed } from "./matrix.mjs";
import { LAYERS, SPEC, TEMPLATE_CODES, bindingsOf, variantsOf } from "./misuse.mjs";

const site = { line: 10, aux: [4] };
const ok = { ok: true, findings: [] };
const event = (code, line, extra = {}) => ({ code, message: "m", positions: line === null ? [] : [{ file: "App.tsx", line }], ...extra });
const run = (events, extra = {}) => ({ diagnostics: events, errors: [], witnesses: [], ...extra });
const finding = (rule, kind, line = 10, endLine = line) => ({ rule, kind, line, endLine, id: "SC", file: "App.tsx" });
const verdict = (over) => judge({ code: "FLUSH_IN_EFFECT_CALLBACK", site, typecheck: { ok: true }, checker: ok, runtime: run([event("FLUSH_IN_EFFECT_CALLBACK", 10)]), ...over });

describe("judge", () => {
  it("regions are the site line and the wrapper's line", () => expect(regionOf(site)).toEqual([10, 4]));
  it("agree: a violation of a rule that claims the code covers the site", () => {
    expect(verdict({ checker: { ok: true, findings: [finding("flush-in-effect-callback", "violation", 9, 11)] } }).outcome).toBe("agree");
  });
  it("agree when the claiming finding sits at the call that runs the misuse, away from its line (remote)", () => {
    const result = verdict({ checker: { ok: true, findings: [finding("flush-in-effect-callback", "violation", 30)] } });
    expect(result).toMatchObject({ outcome: "agree", detail: { remote: "flush-in-effect-callback@30" } });
  });
  it("recall-gap when only an uncertifiable claiming finding sits away from the site", () => {
    expect(verdict({ checker: { ok: true, findings: [finding("flush-in-effect-callback", "uncertifiable", 30)] } }).outcome).toBe("recall-gap");
  });
  it("a coded error the page threw is an event even when the table has no pattern for it (Chrome)", () => {
    const thrown = run([], { errors: [{ kind: "page-error", message: "[SETTLED_CLEANUP_UNOWNED] onSettled returned a cleanup in an unowned scope", positions: [] }] });
    expect(judge({ code: "SETTLED_CLEANUP_UNOWNED", site, typecheck: { ok: true }, checker: ok, runtime: thrown }).outcome).toBe("false-clean");
  });
  it("an unattributed code counts as an unplaced diagnostic", () => {
    expect(judge({ code: "SETTLED_CLEANUP_UNOWNED", site, typecheck: { ok: true }, checker: ok, runtime: run([], { unattributed: ["SETTLED_CLEANUP_UNOWNED"] }) }).outcome).toBe("false-clean");
  });
  it("agree through the wrapper's line", () => {
    expect(verdict({ runtime: run([event("FLUSH_IN_EFFECT_CALLBACK", 4)]), checker: { ok: true, findings: [finding("flush-in-effect-callback", "violation", 10)] } }).outcome).toBe("agree");
  });
  it("wrong-rule: only another rule's violation covers the site", () => {
    expect(verdict({ checker: { ok: true, findings: [finding("missing-owner", "violation")] } }).outcome).toBe("wrong-rule");
  });
  it("another rule's uncertifiable finding on the site is not a recall gap for this code: listed, and the program falls through", () => {
    const result = verdict({ checker: { ok: true, findings: [finding("no-write-in-pure-computation", "uncertifiable")] } });
    expect(result).toMatchObject({ outcome: "false-clean", detail: { unrelated: ["no-write-in-pure-computation/uncertifiable@10"] } });
    // A withholding (dispatch unresolved) declines every rule at its site.
    expect(verdict({ checker: { ok: true, findings: [finding("reactive-dispatch-unresolved", "uncertifiable")] } }).outcome).toBe("recall-gap");
    // A claiming finding elsewhere in the file still makes it a gap.
    expect(verdict({ checker: { ok: true, findings: [finding("no-write-in-pure-computation", "uncertifiable"), finding("flush-in-effect-callback", "uncertifiable", 30)] } }).outcome).toBe("recall-gap");
  });
  it("recall-gap: only an uncertifiable finding covers the site", () => {
    expect(verdict({ checker: { ok: true, findings: [finding("flush-in-effect-callback", "uncertifiable")] } }).outcome).toBe("recall-gap");
  });
  it("false-clean: no claiming finding anywhere, and none covers the site", () => {
    expect(verdict({ checker: { ok: true, findings: [finding("missing-owner", "violation", 30)] } }).outcome).toBe("false-clean");
    expect(verdict().outcome).toBe("false-clean");
  });
  it("a style rule the oracle ignores does not hide a false clean", () => {
    expect(verdict({ checker: { ok: true, findings: [finding("prefer-show", "violation")] } }).outcome).toBe("false-clean");
  });
  it("not-fired: the runtime does not report the code at the site (a generator problem)", () => {
    expect(verdict({ runtime: run([]) }).outcome).toBe("not-fired");
    expect(verdict({ runtime: run([event("FLUSH_IN_EFFECT_CALLBACK", 99)]) }).outcome).toBe("not-fired");
    expect(verdict({ runtime: run([event("NO_OWNER_CLEANUP", 10)]) }).outcome).toBe("not-fired");
  });
  it("an unplaced diagnostic is confirmed, and covered by a claiming finding anywhere", () => {
    expect(verdict({ runtime: run([event("FLUSH_IN_EFFECT_CALLBACK", null)]) }).outcome).toBe("false-clean");
    expect(verdict({ runtime: run([event("FLUSH_IN_EFFECT_CALLBACK", null)]), checker: { ok: true, findings: [finding("flush-in-effect-callback", "uncertifiable", 50)] } }).outcome).toBe("recall-gap");
  });
  it("a thrown coded error is a runtime event", () => {
    const thrown = run([], { errors: [{ kind: "page-error", message: "[FLUSH_IN_ACTION] flush() inside an action body is not allowed.", positions: [{ file: "App.tsx", line: 10 }] }] });
    expect(judge({ code: "FLUSH_IN_ACTION", site, typecheck: { ok: true }, checker: { ok: true, findings: [finding("flush-in-action", "violation")] }, runtime: thrown }).outcome).toBe("agree");
  });
  it("typed-away and harness failures are not checker evidence", () => {
    expect(verdict({ typecheck: { ok: false, errors: ["TS2554"] } }).outcome).toBe("typed-away");
    expect(verdict({ checker: { ok: false, error: "boom", findings: [] } }).outcome).toBe("harness");
    expect(verdict({ runtime: run([], { failure: { phase: "render", message: "x" } }) }).outcome).toBe("harness");
  });
});

const row = (code, context, seed, outcome, extra = {}) => ({ code, context, seed, outcome, binding: "direct", layer: "none", ...extra });
const matrixOf = (rows) => buildMatrix(rows, { options: { seed: 1, perCell: 4 }, header: { label: "x" }, typedAway: {} });

describe("buildMatrix", () => {
  const rows = [row("A", "body", 1, "agree"), row("A", "body", 2, "false-clean"), row("A", "body", 3, "not-fired"), row("A", "body", 4, "wrong-rule"), row("A", "memo", 5, "recall-gap")];
  it("counts programs per cell and only runtime-confirmed ones as checker columns", () => {
    const { cells } = matrixOf(rows);
    expect(cells["A|body"]).toMatchObject({ programs: 4, agree: 1, "false-clean": 1, "not-fired": 1, "wrong-rule": 1, confirmed: 3 });
    expect(cells["A|memo"]).toMatchObject({ programs: 1, "recall-gap": 1, confirmed: 1 });
  });
  it("breaks a cell down by binding and layer", () => {
    const { cells } = matrixOf([row("A", "body", 1, "agree", { binding: "alias" }), row("A", "body", 2, "false-clean", { binding: "alias", layer: "helper" })]);
    expect(cells["A|body"].byBinding.alias).toMatchObject({ agree: 1, "false-clean": 1 });
    expect(cells["A|body"].byLayer.helper["false-clean"]).toBe(1);
  });
  it("renders a markdown table with the cell counts", () => {
    const m = matrixOf(rows);
    const text = markdown(m, codeStatuses(m));
    expect(text).toContain("| A | 1/3 W1 F1 n1 | 0/1 G1 |");
    expect(text).toContain("## False-clean cells");
  });
  it("classifies each code: proven, gaps, no-recall, untriggered, typed-away", () => {
    const m = matrixOf([row("FLUSH_IN_ACTION", "action", 1, "agree"), row("NO_OWNER_CLEANUP", "timer", 1, "agree"), row("NO_OWNER_CLEANUP", "timer", 2, "false-clean"), row("NO_OWNER_EFFECT", "timer", 1, "false-clean"), row("NO_OWNER_CONTEXT", "timer", 1, "not-fired")]);
    const by = Object.fromEntries(codeStatuses(m).map((s) => [s.code, s.status]));
    expect(by).toMatchObject({ FLUSH_IN_ACTION: "proven", NO_OWNER_CLEANUP: "gaps", NO_OWNER_EFFECT: "no-recall", NO_OWNER_CONTEXT: "untriggered", MISSING_EFFECT_FN: "typed-away", DYNAMIC_STATIC_PROMISE: "not-run" });
  });
});

describe("gate", () => {
  const base = matrixOf([row("A", "body", 1, "agree"), row("A", "body", 2, "agree"), row("A", "body", 3, "false-clean")]);
  it("passes the same matrix", () => expect(gate(base, matrixOf([row("A", "body", 1, "agree"), row("A", "body", 2, "agree"), row("A", "body", 3, "false-clean")])).ok).toBe(true));
  it("fails when a cell's agree count drops, naming the program", () => {
    const result = gate(base, matrixOf([row("A", "body", 1, "agree"), row("A", "body", 2, "recall-gap"), row("A", "body", 3, "false-clean")]));
    expect(result.ok).toBe(false);
    expect(result.regressions[0]).toMatchObject({ cell: "A|body", why: "agree 2 -> 1" });
    expect(result.regressions[0].programs).toEqual(["2: agree -> recall-gap"]);
  });
  it("fails when a cell's false-clean count rises", () => {
    const result = gate(base, matrixOf([row("A", "body", 1, "agree"), row("A", "body", 2, "agree"), row("A", "body", 3, "false-clean"), row("A", "body", 4, "false-clean")]));
    expect(result.regressions.map((r) => r.why)).toEqual(["false-clean 1 -> 2"]);
  });
  it("passes and reports an improvement", () => {
    const result = gate(base, matrixOf([row("A", "body", 1, "agree"), row("A", "body", 2, "agree"), row("A", "body", 3, "agree")]));
    expect(result.ok).toBe(true);
    expect(result.improvements.map((i) => i.why)).toEqual(["agree 2 -> 3", "false-clean 1 -> 0"]);
  });
  it("never counts a move out of a typed-away or harness baseline as a regression, and reports it apart", () => {
    const loaded = matrixOf([row("A", "body", 1, "agree"), row("A", "body", 2, "typed-away"), row("A", "body", 3, "harness"), row("A", "body", 4, "agree")]);
    const later = matrixOf([row("A", "body", 1, "agree"), row("A", "body", 2, "false-clean"), row("A", "body", 3, "false-clean"), row("A", "body", 4, "agree")]);
    const result = gate(loaded, later);
    expect(result.ok).toBe(true);
    expect(result.regressions).toEqual([]);
    expect(result.baselineUnjudged).toEqual([{ cell: "A|body", programs: ["2: typed-away -> false-clean", "3: harness -> false-clean"] }]);
  });
  it("still fails a judged program that becomes false-clean in the same cell", () => {
    const loaded = matrixOf([row("A", "body", 1, "agree"), row("A", "body", 2, "typed-away")]);
    const result = gate(loaded, matrixOf([row("A", "body", 1, "false-clean"), row("A", "body", 2, "false-clean")]));
    expect(result.ok).toBe(false);
    expect(result.regressions.map((r) => r.programs)).toEqual([["1: agree -> false-clean"], ["1: agree -> false-clean"]]);
  });
  it("reports a move into harness or typed-away as an infrastructure warning, not a regression", () => {
    const wide = matrixOf([1, 2, 3, 4, 5, 6].map((seed) => row("A", "body", seed, seed === 3 ? "false-clean" : "agree")));
    const result = gate(wide, matrixOf([1, 2, 3, 4, 5, 6].map((seed) => row("A", "body", seed, seed === 2 ? "harness" : seed === 3 ? "typed-away" : "agree"))));
    expect(result.ok).toBe(true);
    expect(result.infrastructure).toEqual([{ cell: "A|body", programs: ["2: agree -> harness", "3: false-clean -> typed-away"] }]);
  });
  it("refuses a current run that is mostly typed-away or harness", () => {
    const result = gate(base, matrixOf([row("A", "body", 1, "harness"), row("A", "body", 2, "typed-away"), row("A", "body", 3, "typed-away")]));
    expect(result.ok).toBe(false);
    expect(result.refused).toMatch(/judged nothing/);
  });
  it("fails a baseline cell that vanished and refuses different seeds", () => {
    expect(gate(base, matrixOf([row("B", "body", 1, "agree")])).regressions[0].why).toMatch(/missing/);
    expect(gate(base, { ...base, seed: 2 }).refused).toMatch(/differ in seed/);
  });
  it("retires a baseline cell whose code the policy table no longer counts as a defect, and still fails an unknown code's", () => {
    const async = { code: "ASYNC_OUTSIDE_LOADING_BOUNDARY", context: "jsx", agree: 0, "false-clean": 1, verdicts: { 1: "false-clean" } };
    const result = gate({ ...base, cells: { ...base.cells, "ASYNC_OUTSIDE_LOADING_BOUNDARY|jsx": async } }, base);
    expect(result.ok).toBe(true);
    expect(result.retired).toEqual([{ cell: "ASYNC_OUTSIDE_LOADING_BOUNDARY|jsx", why: "ASYNC_OUTSIDE_LOADING_BOUNDARY is advisory now, not measured" }]);
    expect(gate({ ...base, cells: { ...base.cells, "NOT_A_CODE|jsx": { ...async, code: "NOT_A_CODE" } } }, base).regressions[0].why).toMatch(/missing/);
  });
});

describe("the grid", () => {
  it("lists every shape of a code once, in a fixed order", () => {
    for (const code of TEMPLATE_CODES) {
      const shapes = gridShapes(code);
      const keys = shapes.map((shape) => `${shape.variant}|${shape.binding}|${shape.layer}`);
      expect(new Set(keys).size).toBe(keys.length);
      expect(shapes.length).toBe(variantsOf(code).reduce((n, variant) => n + bindingsOf(code, variant).length * LAYERS.length, 0));
    }
  });

  it("builds the program of the shape asked, with a seed and id of the shape alone", () => {
    const cell = { code: "NO_OWNER_CLEANUP", context: SPEC.NO_OWNER_CLEANUP.contexts[0] };
    const shape = { variant: null, binding: "namespace", layer: "closure+helper" };
    const { program, built } = generateShape({ seed: 1 }, cell, shape, 7);
    expect(program.components[0].misuse).toMatchObject({ code: cell.code, context: cell.context, ...shape });
    expect(built.id).toMatch(/^[A-Za-z0-9_-]+$/);
    expect(generateShape({ seed: 1 }, cell, shape, 3).built.text).toBe(built.text);              // the index does not move it
    expect(shapeSeed(1, cell.code, cell.context, shape)).not.toBe(shapeSeed(2, cell.code, cell.context, shape));
    expect(shapeSeed(1, cell.code, cell.context, shape)).not.toBe(shapeSeed(1, cell.code, cell.context, { ...shape, layer: "iife" }));
  });

  it("the gate refuses to compare a grid matrix with a random one", () => {
    const random = { seed: 1, perCell: 20, generator: "g", total: {}, cells: {} };
    const grid = { seed: 1, perCell: null, sampling: "grid", generator: "g", total: {}, cells: {} };
    expect(gate(random, grid).refused).toMatch(/perCell|sampling/);
    expect(gate(grid, grid).ok).toBe(true);
    expect(gate(random, { ...random }).ok).toBe(true);
  });
});
