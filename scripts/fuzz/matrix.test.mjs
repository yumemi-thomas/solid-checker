import { describe, expect, it } from "vitest";
import { buildMatrix, codeStatuses, featureTable, gate, generateShape, gridShapes, judge, markdown, regionOf, shapeSeed } from "./matrix.mjs";
import { LAYERS, SPEC, TEMPLATE_CODES, VALUE_BINDINGS, bindingsOf, gridBindingsOf, gridVariantsOf, variantsOf } from "./misuse.mjs";

const site = { line: 10, aux: [4] };
const ok = { ok: true, findings: [] };
const event = (code, line, extra = {}) => ({ code, message: "m", positions: line === null ? [] : [{ file: "App.tsx", line, column: 1 }], ...extra });
const run = (events, extra = {}) => ({ diagnostics: events, errors: [], witnesses: [], ...extra });
const finding = (rule, kind, line = 10, endLine = line) => ({ rule, kind, line, column: 1, endLine, id: "SC", file: "App.tsx" });
const verdict = (over) => judge({ code: "FLUSH_IN_EFFECT_CALLBACK", site, typecheck: { ok: true }, checker: ok, runtime: run([event("FLUSH_IN_EFFECT_CALLBACK", 10)]), ...over });

describe("judge", () => {
  it("judges a thrown code an <Errored> boundary logged instead of rethrowing (unplaced)", () => {
    const logged = run([], { consoleErrors: ["Error: Context must either be created with a default value or a value must be provided before accessing it."] });
    // Review 4c, R8: a diagnostic of a code with a place and no position cannot be judged: `unplaced`, never a pass or a false clean.
    expect(judge({ code: "CONTEXT_NOT_FOUND", site, typecheck: { ok: true }, checker: ok, runtime: logged }).outcome).toBe("unplaced");
    expect(judge({ code: "CONTEXT_NOT_FOUND", site, typecheck: { ok: true }, checker: { ok: true, findings: [finding("missing-context-provider", "violation")] }, runtime: logged }).outcome).toBe("unplaced");
    expect(judge({ code: "CONTEXT_NOT_FOUND", site, typecheck: { ok: true }, checker: ok, runtime: run([], { consoleErrors: ["unrelated"] }) }).outcome).toBe("not-fired");
  });
  it("places a logged code by the logged error's own frames (ADR 0907)", () => {
    const at = (line) => run([], { consoleErrors: ["ContextNotFoundError: Context has no provider above this read and no default value."], consoleErrorPositions: [line === null ? [] : [{ file: "App.tsx", line, column: 1 }]] });
    const claim = { ok: true, findings: [finding("missing-context-provider", "violation")] };
    expect(judge({ code: "CONTEXT_NOT_FOUND", site, typecheck: { ok: true }, checker: claim, runtime: at(10) }).outcome).toBe("agree");
    expect(judge({ code: "CONTEXT_NOT_FOUND", site, typecheck: { ok: true }, checker: ok, runtime: at(10) }).outcome).toBe("false-clean");
    // A cause that is another code thrown at the site does not place it there (review of ADR 0907); the same code's cause does.
    const caused = (cause) => run([], { consoleErrors: ["NoOwnerError: Context can only be accessed under a reactive root."], consoleErrorPositions: [[{ file: "App.tsx", line: 20, column: 1 }]], consoleErrorCauses: [{ message: cause, positions: [{ file: "App.tsx", line: 10, column: 1 }] }] });
    const owner = { ok: true, findings: [finding("missing-owner", "violation")] };
    expect(judge({ code: "NO_OWNER_CONTEXT", site, typecheck: { ok: true }, checker: owner, runtime: caused("Cannot call resolve inside a reactive scope") }).outcome).toBe("not-fired");
    expect(judge({ code: "NO_OWNER_CONTEXT", site, typecheck: { ok: true }, checker: owner, runtime: caused("Context can only be accessed under a reactive root.") }).outcome).not.toBe("not-fired");
    // Review R66: an own frame in another module of the program (helpers.ts, recorded with no line) is the program's: the other code's cause
    // does not place it, and with no placed frame it is unplaced; with no own frame at all the cause places it.
    const own = (frames) => run([], { consoleErrors: ["NoOwnerError: Context can only be accessed under a reactive root."], consoleErrorPositions: [frames], consoleErrorCauses: [{ message: "Cannot call resolve inside a reactive scope", positions: [{ file: "App.tsx", line: 10, column: 1 }] }] });
    expect(judge({ code: "NO_OWNER_CONTEXT", site, typecheck: { ok: true }, checker: owner, runtime: own([{ file: "helpers.ts" }]) }).outcome).toBe("unplaced");
    expect(judge({ code: "NO_OWNER_CONTEXT", site, typecheck: { ok: true }, checker: owner, runtime: own([]) }).outcome).toBe("agree");
    // No frame: unplaced, as before.
    expect(judge({ code: "CONTEXT_NOT_FOUND", site, typecheck: { ok: true }, checker: claim, runtime: at(null) }).outcome).toBe("unplaced");
  });
  it("regions are the site line and the wrapper's line", () => expect(regionOf(site)).toEqual([10, 4]));
  it("agree: a violation of a rule that claims the code covers it at the diagnostic's position, never by a line range (review 4c, R8)", () => {
    expect(verdict({ checker: { ok: true, findings: [finding("flush-in-effect-callback", "violation")] } }).outcome).toBe("agree");
    // The claiming rule's own violation starting elsewhere: the warning in the wrong place (`misplaced`).
    expect(verdict({ checker: { ok: true, findings: [finding("flush-in-effect-callback", "violation", 9, 11)] } }).outcome).toBe("misplaced");
  });
  // The call that runs the misuse is a deeper frame of the code's stack: its exact position covers, on a line that mounts nothing.
  const callAt30 = run([{ code: "FLUSH_IN_EFFECT_CALLBACK", message: "m", positions: [{ file: "App.tsx", line: 10, column: 1 }, { file: "App.tsx", line: 30, column: 1 }] }]);
  const text30 = { files: { "App.tsx": Array.from({ length: 40 }, (_, index) => (index === 29 ? "  helper();" : "x();")).join("\n") } };
  it("agree when the claiming finding sits at the call that runs the misuse, a deeper frame of its stack (remote)", () => {
    const result = verdict({ runtime: callAt30, built: text30, checker: { ok: true, findings: [finding("flush-in-effect-callback", "violation", 30)] } });
    expect(result).toMatchObject({ outcome: "agree", detail: { remote: "flush-in-effect-callback@30" } });
    expect(verdict({ checker: { ok: true, findings: [finding("flush-in-effect-callback", "violation", 30)] } }).outcome).toBe("misplaced");
  });
  it("recall-gap when only an uncertifiable claiming finding sits at a deeper frame's call; off the stack it covers nothing (R8)", () => {
    expect(verdict({ runtime: callAt30, built: text30, checker: { ok: true, findings: [finding("flush-in-effect-callback", "uncertifiable", 30)] } }).outcome).toBe("recall-gap");
    expect(verdict({ checker: { ok: true, findings: [finding("flush-in-effect-callback", "uncertifiable", 30)] } }).outcome).toBe("false-clean");
  });
  it("a coded error the page threw is an event even when the table has no pattern for it (Chrome)", () => {
    const thrown = run([], { errors: [{ kind: "page-error", message: "[SETTLED_CLEANUP_UNOWNED] onSettled returned a cleanup in an unowned scope", positions: [] }] });
    expect(judge({ code: "SETTLED_CLEANUP_UNOWNED", site, typecheck: { ok: true }, checker: ok, runtime: thrown }).outcome).toBe("false-clean");
  });
  it("an unattributed placeless code is judged by file (ADR 0815)", () => {
    expect(judge({ code: "SETTLED_CLEANUP_UNOWNED", site, typecheck: { ok: true }, checker: ok, runtime: run([], { unattributed: ["SETTLED_CLEANUP_UNOWNED"] }) }).outcome).toBe("false-clean");
  });
  it("agree through the wrapper's line only at the diagnostic's position (R8)", () => {
    expect(verdict({ runtime: run([event("FLUSH_IN_EFFECT_CALLBACK", 4)]), checker: { ok: true, findings: [finding("flush-in-effect-callback", "violation", 4)] } }).outcome).toBe("agree");
    expect(verdict({ runtime: run([event("FLUSH_IN_EFFECT_CALLBACK", 4)]), checker: { ok: true, findings: [finding("flush-in-effect-callback", "violation", 10)] } }).outcome).toBe("misplaced");
  });
  it("wrong-rule: only another rule's violation covers the site", () => {
    expect(verdict({ checker: { ok: true, findings: [finding("missing-owner", "violation")] } }).outcome).toBe("wrong-rule");
  });
  it("another rule's violation the runtime substantiates on the site is a second defect, not a wrong rule", () => {
    const both = run([event("STORE_WRITE_OUTSIDE_SETTER", 10), event("STRICT_READ_UNTRACKED", 10)]);
    const findings = [finding("strict-read-untracked", "violation"), finding("no-store-mutation-outside-setter", "uncertifiable")];
    expect(judge({ code: "STORE_WRITE_OUTSIDE_SETTER", site, typecheck: { ok: true }, checker: { ok: true, findings }, runtime: both }))
      .toMatchObject({ outcome: "recall-gap", detail: { corroborated: ["strict-read-untracked"] } });
    expect(judge({ code: "STORE_WRITE_OUTSIDE_SETTER", site, typecheck: { ok: true }, checker: { ok: true, findings }, runtime: run([event("STORE_WRITE_OUTSIDE_SETTER", 10)]) }).outcome).toBe("wrong-rule");
  });
  it("a placeless code corroborates another rule's violation through its own `loc` at a node enclosing the site, counted on its own", () => {
    // ASYNC_OUTSIDE_LOADING_BOUNDARY names the `{` of the JSX hole (line 8) that holds the site (line 10); its stack names no App.tsx line.
    const built = { lineTable: [{ line: 1, endLine: 20, path: ["body"] }, { line: 8, endLine: 11, path: ["body", 0] }, { line: 10, endLine: 10, path: ["body", 0, "x"] }] };
    const at = { ...site, path: ["body", 0, "x"] };
    const findings = [finding("async-outside-loading-boundary", "violation"), finding("loading-on-outside-hold", "uncertifiable")];
    const hold = (loc, others = []) => judge({ code: "LOADING_ON_OUTSIDE_HOLD", site: at, typecheck: { ok: true }, checker: { ok: true, findings }, built, others,
      runtime: run([event("LOADING_ON_OUTSIDE_HOLD", null), event("ASYNC_OUTSIDE_LOADING_BOUNDARY", null, { loc })]) });
    expect(hold("cases/x/App.tsx:8:25")).toMatchObject({ outcome: "recall-gap", detail: { corroborated: ["async-outside-loading-boundary"], holeCorroborated: true } });
    // A `loc` on no enclosing node, or none at all, corroborates nothing.
    expect(hold("cases/x/App.tsx:12:3").outcome).toBe("wrong-rule");
    expect(hold(null).outcome).toBe("wrong-rule");
    // A node that also holds another use site names neither: only the hole itself (line 8) is still this site's.
    expect(hold("cases/x/App.tsx:1:1", [[15]]).outcome).toBe("wrong-rule");
    expect(hold("cases/x/App.tsx:8:25", [[15]]).outcome).toBe("recall-gap");
  });
  it("another rule's uncertifiable finding on the site is not a recall gap for this code: listed, and the program falls through", () => {
    const result = verdict({ checker: { ok: true, findings: [finding("no-write-in-pure-computation", "uncertifiable")] } });
    expect(result).toMatchObject({ outcome: "false-clean", detail: { unrelated: ["no-write-in-pure-computation/uncertifiable@10"] } });
    // A withholding (dispatch unresolved) declines every rule at its site.
    expect(verdict({ checker: { ok: true, findings: [finding("reactive-dispatch-unresolved", "uncertifiable")] } }).outcome).toBe("recall-gap");
    // Review 4c: elsewhere it declines nothing; at the stack's top frame, or a deeper frame's exact call position, it does, unless that
    // position mounts the program (a component's tag, a render call) or the program's text is unknown.
    expect(verdict({ checker: { ok: true, findings: [finding("reactive-dispatch-unresolved", "uncertifiable", 30)] } })).toMatchObject({ outcome: "false-clean", detail: { certified: false } });
    const deep = run([{ code: "FLUSH_IN_EFFECT_CALLBACK", message: "m", positions: [{ file: "App.tsx", line: 10, column: 1 }, { file: "App.tsx", line: 30, column: 1 }] }]);
    const text = (line30) => ({ files: { "App.tsx": Array.from({ length: 40 }, (_, index) => (index === 29 ? line30 : "x();")).join("\n") } });
    const withheld30 = { ok: true, findings: [finding("reactive-dispatch-unresolved", "uncertifiable", 30)] };
    expect(verdict({ runtime: deep, checker: withheld30, built: text("  helper(n);") }).outcome).toBe("recall-gap");
    expect(verdict({ runtime: deep, checker: withheld30, built: text("<C1 />;") })).toMatchObject({ outcome: "false-clean", detail: { withheldNear: "reactive-dispatch-unresolved@30:1" } });
    expect(verdict({ runtime: deep, checker: withheld30 }).outcome).toBe("false-clean");
    const top = run([{ code: "FLUSH_IN_EFFECT_CALLBACK", message: "m", positions: [{ file: "App.tsx", line: 30, column: 1 }, { file: "App.tsx", line: 10, column: 1 }] }]);
    expect(verdict({ runtime: top, checker: withheld30 }).outcome).toBe("recall-gap");
    // A claiming finding elsewhere in the file no longer makes it a gap (R8).
    expect(verdict({ checker: { ok: true, findings: [finding("no-write-in-pure-computation", "uncertifiable"), finding("flush-in-effect-callback", "uncertifiable", 30)] } }).outcome).toBe("false-clean");
  });
  it("a declining rule's uncertifiable finding is a recall gap; its violation stays a wrong rule (codes.mjs `declining`)", () => {
    const ctx = (findings) => judge({ code: "CONTEXT_NOT_FOUND", site, typecheck: { ok: true }, checker: { ok: true, findings }, runtime: run([event("CONTEXT_NOT_FOUND", 10)]) });
    expect(ctx([finding("missing-owner", "uncertifiable")]).outcome).toBe("recall-gap");
    expect(ctx([finding("missing-owner", "violation")]).outcome).toBe("wrong-rule");
    // Only for the code that lists it: SC4001's obligation does not decline a flush code.
    expect(verdict({ checker: { ok: true, findings: [finding("missing-owner", "uncertifiable")] } }).outcome).toBe("false-clean");
  });
  it("marks a false clean certified when the file has no uncertifiable finding at all (a fail-closed break)", () => {
    expect(verdict({}).detail.certified).toBe(true);
    expect(verdict({ checker: { ok: true, findings: [finding("no-write-in-pure-computation", "violation", 30)] } }).detail.certified).toBe(true);
    expect(verdict({ checker: { ok: true, findings: [finding("missing-owner", "uncertifiable", 30)] } }).detail.certified).toBe(false);
  });
  it("tables outcomes by binding and layer across codes", () => {
    const rows = [
      { code: "A", binding: "alias", layer: "none", outcome: "false-clean", detail: { certified: true } },
      { code: "B", binding: "alias", layer: "none", outcome: "false-clean", detail: { certified: false } },
      { code: "B", binding: "alias", layer: "none", outcome: "agree", detail: {} },
      { code: "A", binding: "direct", layer: "none", outcome: "agree", detail: {} },
    ];
    const table = featureTable(rows);
    expect(table["alias|none"]).toMatchObject({ programs: 3, agree: 1, "false-clean": 2, falseCleanCertified: 1, codes: ["A", "B"] });
    expect(table["direct|none"]).toMatchObject({ programs: 1, agree: 1, "false-clean": 0, codes: [] });
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
  it("an unplaced diagnostic cannot be judged: no finding has its position (R8, `unplaced`)", () => {
    expect(verdict({ runtime: run([event("FLUSH_IN_EFFECT_CALLBACK", null)]) }).outcome).toBe("unplaced");
    expect(verdict({ runtime: run([event("FLUSH_IN_EFFECT_CALLBACK", null)]), checker: { ok: true, findings: [finding("flush-in-effect-callback", "uncertifiable", 50)] } }).outcome).toBe("unplaced");
  });
  it("a thrown coded error is a runtime event", () => {
    const thrown = run([], { errors: [{ kind: "page-error", message: "[FLUSH_IN_ACTION] flush() inside an action body is not allowed.", positions: [{ file: "App.tsx", line: 10, column: 1 }] }] });
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
    expect(by).toMatchObject({ FLUSH_IN_ACTION: "proven", NO_OWNER_CLEANUP: "gaps", NO_OWNER_EFFECT: "no-recall", NO_OWNER_CONTEXT: "untriggered", ASYNC_STORE_SETTER: "typed-away", DYNAMIC_STATIC_PROMISE: "not-run" });
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
  it("fails when a baseline program enters false-clean", () => {
    const result = gate(base, matrixOf([row("A", "body", 1, "agree"), row("A", "body", 2, "false-clean"), row("A", "body", 3, "false-clean")]));
    expect(result.regressions.map((r) => r.why)).toEqual(["agree 2 -> 1", "false-clean 1 -> 2"]);
    expect(result.regressions[1].programs).toEqual(["2: agree -> false-clean"]);
  });
  it("reports a program the baseline does not have (a grid shape added since) as new, never as a regression", () => {
    const result = gate(base, matrixOf([row("A", "body", 1, "agree"), row("A", "body", 2, "agree"), row("A", "body", 3, "false-clean"), row("A", "body", 4, "false-clean")]));
    expect(result.ok).toBe(true);
    expect(result.newPrograms).toEqual([{ cell: "A|body", programs: 1 }]);
    // A baseline program the current run lacks still fails.
    expect(gate(base, matrixOf([row("A", "body", 1, "agree"), row("A", "body", 3, "false-clean")])).ok).toBe(false);
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

describe("the grid's store variants", () => {
  it("adds the past-root and deferred writes to the grid only, each deferred one only where calling the setter is legal", () => {
    expect(variantsOf("STORE_WRITE_OUTSIDE_SETTER")).toEqual(["assign", "delete"]);
    expect(gridVariantsOf("STORE_WRITE_OUTSIDE_SETTER")).toEqual(["assign", "delete", "nested-assign", "nested-push", "deferred-micro", "deferred-timer", "deferred-then"]);
    const variants = (context) => [...new Set(gridShapes("STORE_WRITE_OUTSIDE_SETTER", context).map((shape) => shape.variant))];
    expect(variants("handler")).toContain("deferred-micro");
    expect(variants("body")).not.toContain("deferred-micro");
    expect(variants("body")).toContain("nested-push");
  });
  it("prints a deferred write inside the store's own setter callback, on the site's line", () => {
    const { built } = generateShape({ seed: 1 }, { code: "STORE_WRITE_OUTSIDE_SETTER", context: "handler" }, { variant: "deferred-micro", binding: "direct", layer: "none" }, 0);
    const site = built.sites.find((s) => s.code);
    expect(built.text.split("\n")[site.line - 1]).toMatch(/setS1\(\(\) => \{ queueMicrotask\(\(\) => \{ s1\.v = 7; \}\); \}\)/);
    const nested = generateShape({ seed: 1 }, { code: "STORE_WRITE_OUTSIDE_SETTER", context: "body" }, { variant: "nested-assign", binding: "direct", layer: "none" }, 0).built.text;
    expect(nested).toContain("createStore({ v: 0, a: { v: 0 }, items: [0] })");
    expect(nested).toContain("s1.a.v = 7");
  });
});

describe("the grid", () => {
  it("adds the value bindings (hook, hook member, tuple slot) to values only, never to APIs and never to the random draw", () => {
    expect(gridBindingsOf("STRICT_READ_UNTRACKED", "signal")).toEqual([...bindingsOf("STRICT_READ_UNTRACKED", "signal"), ...VALUE_BINDINGS]);
    expect(gridBindingsOf("STORE_WRITE_OUTSIDE_SETTER", "assign")).toEqual([...bindingsOf("STORE_WRITE_OUTSIDE_SETTER", "assign"), ...VALUE_BINDINGS]);
    expect(gridBindingsOf("NO_OWNER_CLEANUP", null)).toEqual(bindingsOf("NO_OWNER_CLEANUP", null));
    expect(gridBindingsOf("REACTIVE_WRITE_IN_OWNED_SCOPE", "refresh")).toEqual(bindingsOf("REACTIVE_WRITE_IN_OWNED_SCOPE", "refresh"));   // refresh() is an API
    for (const code of TEMPLATE_CODES) for (const variant of variantsOf(code)) for (const binding of VALUE_BINDINGS) expect(bindingsOf(code, variant)).not.toContain(binding);
  });
  it("lists every shape of a code once, in a fixed order", () => {
    for (const code of TEMPLATE_CODES) {
      const shapes = gridShapes(code);
      const keys = shapes.map((shape) => `${shape.variant}|${shape.binding}|${shape.layer}`);
      expect(new Set(keys).size).toBe(keys.length);
      // A variant one source carries (`variantSources`) has no local shape.
      expect(shapes.length).toBe(gridVariantsOf(code).filter((variant) => !SPEC[code].variantSources?.[variant]).reduce((n, variant) => n + gridBindingsOf(code, variant).length * LAYERS.length, 0));
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

describe("the mount axis", () => {
  it("keeps a root program's id and seed, and renders every other mount from an exported parent", async () => {
    const { cellList, generateShape, gridShapes } = await import("./matrix.mjs");
    const cell = cellList().find((c) => c.code === "REACTIVE_WRITE_IN_OWNED_SCOPE" && c.context === "body");
    const [root] = gridShapes(cell.code, cell.context);
    expect(gridShapes(cell.code, cell.context, ["root", "child"]).length).toBe(2 * gridShapes(cell.code, cell.context).length);
    const plain = generateShape({ seed: 1 }, cell, root, 0);
    expect(plain.built.id).not.toMatch(/-m-/);
    expect(JSON.parse(plain.built.files["oracle.json"]).mount).toEqual(["C1"]);
    const forms = { child: /<div>\n\s+<C1 \/>/, show: /<Show when=\{true\}>\n\s+<C1 \/>/, for: /<For each=\{\[1\]\}>\n\s+\{\(\) => <C1 \/>\}/ };
    for (const [mount, form] of Object.entries(forms)) {
      const { built } = generateShape({ seed: 1 }, cell, { ...root, mount }, 0);
      expect(built.id).toBe(`${plain.built.id}-m-${mount}`);
      expect(built.text).toMatch(/^function C1\(/m);
      expect(built.text).toMatch(/^export function P1\(\)/m);
      expect(built.text).toMatch(form);
      expect(JSON.parse(built.files["oracle.json"]).mount).toEqual(["P1"]);
    }
  });
});

describe("the source axes", () => {
  const firstOf = async (code, context, source) => {
    const { gridShapes } = await import("./matrix.mjs");
    return gridShapes(code, context, ["root"], [source]);
  };
  it("adds no shape and moves no program without --sources", async () => {
    const { gridShapes, shapeSeed } = await import("./matrix.mjs");
    const local = gridShapes("REACTIVE_WRITE_IN_OWNED_SCOPE", "body");
    expect(gridShapes("REACTIVE_WRITE_IN_OWNED_SCOPE", "body", ["root"], ["local"])).toEqual(local);
    expect(local.every((shape) => !("source" in shape))).toBe(true);
    expect(shapeSeed(1, "REACTIVE_WRITE_IN_OWNED_SCOPE", "body", local[0])).toBe(shapeSeed(1, "REACTIVE_WRITE_IN_OWNED_SCOPE", "body", { ...local[0] }));
  });
  it("expands axis names and refuses an unknown source", async () => {
    const { SOURCE_AXES, sourcesOption } = await import("./misuse.mjs");
    expect(sourcesOption("props,module")).toEqual([...SOURCE_AXES.props, ...SOURCE_AXES.module]);
    expect(() => sourcesOption("nowhere")).toThrow(/unknown source/);
  });
  it("creates a prop source's values in the exported parent and names them through props", async () => {
    const { cellList, generateShape } = await import("./matrix.mjs");
    const cell = cellList().find((c) => c.code === "REACTIVE_WRITE_IN_OWNED_SCOPE" && c.context === "body");
    const [shape] = (await firstOf(cell.code, cell.context, "prop")).filter((s) => s.variant === "signal" && s.binding === "direct");
    const { built } = generateShape({ seed: 1 }, cell, shape, 0);
    expect(built.id).toMatch(/-s-prop$/);
    expect(built.text).toMatch(/^function C1\(props: C1Props\)/m);
    expect(built.text).toMatch(/^export function P1\(\) \{\n {2}const \[w1, setW1\] = createSignal\(0\);/m);
    expect(built.text).toContain("<C1 w1={w1} setW1={setW1} />");
    expect(built.text.split("\n")[built.sites[0].line - 1]).toContain("props.setW1(7)");
    expect(JSON.parse(built.files["oracle.json"]).mount).toEqual(["P1"]);
  });
  it("prints each source in its own form", async () => {
    const { cellList, generateShape } = await import("./matrix.mjs");
    const forms = {
      "prop-destructured": [/^function C1\(\{ w1, setW1 \}: C1Props\)/m, /setW1\(7\)/],
      "prop-merge": [/const merged = merge\(\{ extra: 1 \}, props\);/, /merged\.setW1\(7\)/],
      "prop-omit": [/const rest = omit\(props, "extra"\);/, /extra=\{1\}/],
      "prop-spread": [/^function M1\(props: C1Props\) \{\n {2}return <C1 \{\.\.\.props\} \/>;/m, /<M1 w1=\{w1\} setW1=\{setW1\} \/>/],
      context: [/const ValCtx = createContext<C1Vals>\(\);/, /const ctx = useContext\(ValCtx\);/, /<ValCtx value=\{\{ w1, setW1 \}\}>/],
      "context-destructured": [/const \{ w1, setW1 \} = useContext\(ValCtx\);/],
      "module-hook": [/const \{ w1, setW1 \} = useVals\(\);/, /import \{ useVals \} from "\.\/helpers";/],
      "module-hook-member": [/const vals = useVals\(\);/, /vals\.setW1\(7\)/],
    };
    const cell = cellList().find((c) => c.code === "REACTIVE_WRITE_IN_OWNED_SCOPE" && c.context === "body");
    for (const [source, patterns] of Object.entries(forms)) {
      const [shape] = (await firstOf(cell.code, cell.context, source)).filter((s) => s.variant === "signal" && s.binding === "direct");
      const { built } = generateShape({ seed: 1 }, cell, shape, 0);
      for (const pattern of patterns) expect(built.text, source).toMatch(pattern);
    }
    const [moduleShape] = (await firstOf(cell.code, cell.context, "module")).filter((s) => s.variant === "signal" && s.binding === "direct");
    const { built } = generateShape({ seed: 1 }, cell, moduleShape, 0);
    expect(built.files["helpers.ts"]).toBe('import { createSignal } from "solid-js";\n\nexport const [w1, setW1] = createSignal(0);\n');
    expect(built.text).toContain('import { w1, setW1 } from "./helpers";');
    expect(JSON.parse(built.files["oracle.json"]).mount).toEqual(["C1"]);
  });
  it("passes a read's value as a getter prop only where the site reads it", async () => {
    const { sourcesOf } = await import("./misuse.mjs");
    expect(sourcesOf("STRICT_READ_UNTRACKED", "signal", "direct")).toContain("prop-value");
    expect(sourcesOf("STRICT_READ_UNTRACKED", "signal", "alias")).not.toContain("prop-value");
    expect(sourcesOf("REACTIVE_WRITE_IN_OWNED_SCOPE", "signal", "direct")).not.toContain("prop-value");
    expect(sourcesOf("PENDING_ASYNC_UNTRACKED_READ", null, "direct")).toEqual(expect.arrayContaining(["async-prop", "async-value", "async-context", "async-loading", "module"]));
    expect(sourcesOf("PENDING_ASYNC_UNTRACKED_READ", null, "direct")).not.toContain("prop");
    expect(sourcesOf("FLUSH_IN_ACTION", null, "direct")).toEqual(["local"]);
    const { cellList, generateShape } = await import("./matrix.mjs");
    const cell = cellList().find((c) => c.code === "PENDING_ASYNC_UNTRACKED_READ" && c.context === "body");
    const [shape] = (await firstOf(cell.code, cell.context, "async-value")).filter((s) => s.binding === "direct");
    const { built } = generateShape({ seed: 1 }, cell, shape, 0);
    expect(built.text).toContain("<C1 a1={a1()} />");
    expect(built.text.split("\n")[built.sites[0].line - 1].trim()).toBe("props.a1;");
    const [loading] = (await firstOf(cell.code, cell.context, "async-loading")).filter((s) => s.binding === "direct");
    expect(generateShape({ seed: 1 }, cell, loading, 0).built.text).toMatch(/<Loading fallback=\{0\}>\n\s+<C1 a1=\{a1\} \/>\n\s+<\/Loading>/);
  });
});

describe("combination programs", () => {
  it("marks the cell's site primary among one or two legal uses of other codes, deterministically, with an id and seed of its own", async () => {
    const { cellList, combined, generateShape, gridShapes, shapeSeed, siteOf } = await import("./matrix.mjs");
    const { validate } = await import("./model.mjs");
    const cell = cellList().find((c) => c.code === "REACTIVE_WRITE_IN_OWNED_SCOPE" && c.context === "body");
    const [plain] = gridShapes(cell.code, cell.context);
    const [shape] = combined([plain], true);
    expect(shapeSeed(1, cell.code, cell.context, shape)).not.toBe(shapeSeed(1, cell.code, cell.context, plain));
    const a = generateShape({ seed: 1 }, cell, shape, 0);
    expect(a.built.id).toBe(`${generateShape({ seed: 1 }, cell, plain, 0).built.id}-c`);
    expect(generateShape({ seed: 1 }, cell, shape, 0).built.text).toBe(a.built.text);
    expect(validate(a.program)).toEqual([]);
    const uses = a.built.sites.filter((site) => site.code);
    expect(uses.length).toBeGreaterThanOrEqual(2);
    expect(uses.filter((site) => site.primary)).toHaveLength(1);
    expect(siteOf(a.built)).toMatchObject({ code: cell.code, primary: true });
    expect(uses.filter((site) => !site.primary).every((site) => site.code !== cell.code && site.bind === "direct")).toBe(true);
  });
  it("judges the primary site without the claims that cover another use", async () => {
    const { judge } = await import("./matrix.mjs");
    const site = { line: 10, aux: [] };
    // The finding at line 20 is the call that runs the site, a deeper frame of the code's stack (R8).
    const runtime = { diagnostics: [{ code: "REACTIVE_WRITE_IN_OWNED_SCOPE", positions: [{ file: "App.tsx", line: 10, column: 1 }, { file: "App.tsx", line: 20, column: 1 }] }], errors: [] };
    const elsewhere = { rule: "no-write-in-pure-computation", kind: "violation", line: 20, column: 1, endLine: 20 };
    const built = { files: { "App.tsx": Array.from({ length: 30 }, () => "x();").join("\n") } };
    const input = { code: "REACTIVE_WRITE_IN_OWNED_SCOPE", site, typecheck: { ok: true }, checker: { ok: true, findings: [elsewhere] }, runtime, built };
    expect(judge(input).outcome).toBe("agree");
    const { outcome, detail } = judge({ ...input, others: [[20]] });
    expect(outcome).toBe("false-clean");
    expect(detail.atOthers).toEqual(["no-write-in-pure-computation@20"]);
  });
  it("judges every use of a legal combination and takes the worst", async () => {
    const { judgeLegalProgram } = await import("./legal.mjs");
    const built = { sites: [{ code: "REACTIVE_WRITE_IN_OWNED_SCOPE", line: 5, aux: [], hit: 1, primary: true }, { code: "NO_OWNER_CLEANUP", line: 9, aux: [], hit: 2 }], plan: { clicks: [] } };
    const runtime = { diagnostics: [], errors: [], hits: { 1: 1, 2: 1 } };
    const clean = judgeLegalProgram({ code: "REACTIVE_WRITE_IN_OWNED_SCOPE", built, typecheck: { ok: true }, checker: { ok: true, findings: [] }, runtime });
    expect(clean.outcome).toBe("clean");
    expect(clean.detail.sites).toEqual(["REACTIVE_WRITE_IN_OWNED_SCOPE@5:clean", "NO_OWNER_CLEANUP@9:clean"]);
    const fp = judgeLegalProgram({ code: "REACTIVE_WRITE_IN_OWNED_SCOPE", built, typecheck: { ok: true }, checker: { ok: true, findings: [{ rule: "missing-owner", kind: "violation", line: 9, endLine: 9 }] }, runtime });
    expect(fp).toMatchObject({ outcome: "false-positive", detail: { at: 9, code: "NO_OWNER_CLEANUP" } });
    // Another use provoking a code on its own lines (a `-guarded-rerun` read) does not make a use of that code elsewhere not legal.
    const shared = { sites: [{ code: "STRICT_READ_UNTRACKED", line: 5, aux: [], hit: 1, primary: true }, { code: "PENDING_ASYNC_UNTRACKED_READ", line: 9, aux: [], hit: 2 }], plan: { clicks: [] } };
    const rerun = { ...runtime, diagnostics: [{ code: "STRICT_READ_UNTRACKED", positions: [{ file: "App.tsx", line: 9 }] }] };
    expect(judgeLegalProgram({ code: "STRICT_READ_UNTRACKED", built: shared, typecheck: { ok: true }, checker: { ok: true, findings: [] }, runtime: rerun }).detail.sites).toEqual(["STRICT_READ_UNTRACKED@5:clean", "PENDING_ASYNC_UNTRACKED_READ@9:clean"]);
  });
});

describe("a withholding at a call's argument (ADR 0815, R20)", () => {
  const withheld = (line, column) => ({ rule: "reactive-dispatch-unresolved", kind: "uncertifiable", line, column, endLine: line, id: "SC9012", file: "App.tsx" });
  const text = (lines) => ({ files: { "App.tsx": lines.join("\n") } });
  const judged = (code, positions, finding, lines, sites = { line: 1, aux: [] }) => judge({ code, site: sites, typecheck: { ok: true }, checker: { ok: true, findings: [finding] }, runtime: run([{ code, message: "m", positions }]), built: text(lines) }).outcome;

  it("covers a read a native callee did on what it was handed", () => {
    expect(judged("STRICT_READ_UNTRACKED", [{ file: "App.tsx", line: 1, column: 6 }], withheld(1, 16), ["JSON.stringify(st);", "x();"])).toBe("recall-gap");
  });
  it("covers no code the call raises itself: a setter's write, a primitive's own run", () => {
    const setter = ['import { createSignal } from "solid-js";', "const [, set] = createSignal(0);", "set(holder.c);"];
    expect(judged("REACTIVE_WRITE_IN_OWNED_SCOPE", [{ file: "App.tsx", line: 3, column: 1 }], withheld(3, 5), setter, { line: 3, aux: [] })).toBe("false-clean");
    const untrack = ['import { untrack } from "solid-js";', "untrack(holder.c);"];
    expect(judged("STRICT_READ_UNTRACKED", [{ file: "App.tsx", line: 2, column: 1 }], withheld(2, 9), untrack, { line: 2, aux: [] })).toBe("false-clean");
  });
  it("covers a store write only at a function's first argument: a member call writes its receiver (R22)", () => {
    const list = ["list.push(holder.c);", "x();"];
    expect(judged("STORE_WRITE_OUTSIDE_SETTER", [{ file: "App.tsx", line: 1, column: 6 }], withheld(1, 11), list)).toBe("false-clean");
    const second = ["mutate(a, holder.c);", "x();"];
    expect(judged("STORE_WRITE_OUTSIDE_SETTER", [{ file: "App.tsx", line: 1, column: 1 }], withheld(1, 11), second)).toBe("false-clean");
    const first = ["mutate(st);", "x();"];
    expect(judged("STORE_WRITE_OUTSIDE_SETTER", [{ file: "App.tsx", line: 1, column: 1 }], withheld(1, 8), first)).toBe("recall-gap");
    const assign = ["Object.assign(st, x);", "y();"];
    expect(judged("STORE_WRITE_OUTSIDE_SETTER", [{ file: "App.tsx", line: 1, column: 8 }], withheld(1, 15), assign)).toBe("recall-gap");
  });
  it("covers nothing at a deeper frame: another file's top frame keeps the App frame below depth 0", () => {
    const tag = ["const holder = { c: 0 };", "export function App() { return <Child v={holder.c} />; }"];
    const positions = [{ file: "helpers.ts" }, { file: "App.tsx", line: 2, column: 32 }];
    expect(judged("STRICT_READ_UNTRACKED", positions, withheld(2, 32), tag, { line: 2, aux: [] })).toBe("false-clean");
    const helper = ["const r = pick(holder.c);", "x();"];
    expect(judged("STRICT_READ_UNTRACKED", [{ file: "helpers.ts", line: 3, column: 1 }, { file: "App.tsx", line: 1, column: 11 }], withheld(1, 16), helper, { line: 1, aux: [] })).toBe("false-clean");
  });
});

describe("an uncaught NotReadyError (ADR 0815)", () => {
  const failure = { name: "NotReadyError", message: "", phase: "import", stack: "Error" };
  const code = "PENDING_ASYNC_UNTRACKED_READ";
  const judged = (findings) => judge({ code, site: { line: 6, aux: [] }, typecheck: { ok: true }, checker: { ok: true, findings }, runtime: run([], { failure }) }).outcome;
  it("is the program's result: a row at the read is judged, none is a false clean", () => {
    expect(judged([{ rule: "pending-async-unsuspendable-read", kind: "uncertifiable", line: 6, endLine: 6, column: 3 }])).toBe("pending-uncaught");
    expect(judged([{ rule: "pending-async-unsuspendable-read", kind: "uncertifiable", line: 2, endLine: 2, column: 3 }])).toBe("false-clean");
    expect(judged([])).toBe("false-clean");
  });
  it("leaves any other failure the harness's", () => {
    expect(judge({ code, site: { line: 6, aux: [] }, typecheck: { ok: true }, checker: { ok: true, findings: [] }, runtime: run([], { failure: { ...failure, name: "TypeError", message: "x" } }) }).outcome).toBe("harness");
  });
});
