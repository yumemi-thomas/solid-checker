import { describe, expect, it } from "vitest";
import { evaluatedDead, generateLegal, judgeLegal, legalCells, legalShapes, recallLegalShapes } from "./legal.mjs";
import { RECALL_TEMPLATE_CODES, SPEC, TEMPLATE_CODES } from "./misuse.mjs";
import { siteOf } from "./matrix.mjs";

const site = { line: 10, aux: [], hit: 4 };
const finding = (rule, kind, line = 10) => ({ rule, kind, line, endLine: line, id: "SC", file: "App.tsx" });
const runtime = (over = {}) => ({ diagnostics: [], errors: [], witnesses: [], hits: { 4: 1 }, ...over });
const judge = (over) => judgeLegal({ code: "STORE_WRITE_OUTSIDE_SETTER", site, typecheck: { ok: true }, checker: { ok: true, findings: [] }, runtime: runtime(), ...over });

describe("judgeLegal", () => {
  it("is clean with no finding at the site, uncertifiable when the checker declined there", () => {
    expect(judge({}).outcome).toBe("clean");
    expect(judge({ checker: { ok: true, findings: [finding("no-store-mutation-outside-setter", "uncertifiable")] } }).outcome).toBe("uncertifiable");
  });
  it("is a false positive: a violation at a site that ran, with none of its rule's codes reported", () => {
    const result = judge({ checker: { ok: true, findings: [finding("no-store-mutation-outside-setter", "violation")] } });
    expect(result).toMatchObject({ outcome: "false-positive", detail: { rules: ["no-store-mutation-outside-setter"] } });
  });
  it("is unexecuted at a site that never ran, and disturbed when the program threw", () => {
    const violation = { ok: true, findings: [finding("no-store-mutation-outside-setter", "violation")] };
    expect(judge({ checker: violation, runtime: runtime({ hits: { 4: 0 } }) }).outcome).toBe("unexecuted");
    expect(judge({ checker: violation, runtime: runtime({ errors: [{ kind: "window-error", message: "boom", positions: [] }] }) }).outcome).toBe("disturbed");
  });
  // Review 4c: a violation means "wrong if it runs", and a branch the checker cannot evaluate counts as reachable, so a run-claiming
  // rule's violation at a site whose counter is 0 (`xs.forEach(hf)` over `JSON.parse("[]")`, lg-pending-async-forbidden-scope-zero-
  // settled-var) is unexecuted, never a false positive or a pass. ADR 0504's review had scored it a false positive.
  it("stays a false positive where the text says the site never runs (R21)", () => {
    const built = (line) => ({ ledger: [], files: { "App.tsx": line } });
    expect(evaluatedDead(null, built("  [].forEach(hf);"), { line: 1, aux: [] })).toBe(true);
    expect(evaluatedDead(null, built("  0 && hf();"), { line: 1, aux: [] })).toBe(true);
    expect(evaluatedDead(null, built("  JSON.parse('[]').forEach(hf);"), { line: 1, aux: [] })).toBe(false);
  });
  it("a closed program is evaluated, unless a value in it is opaque (R22; zero-*-var keeps the owner's ruling)", () => {
    const dead = (context) => {
      const cell = legalCells({ recall: true }).find((candidate) => candidate.context === context);
      const { program, built } = generateLegal({ seed: 1 }, cell, { shape: recallLegalShapes(cell.code, cell.context)[0] });
      return evaluatedDead(program, built, built.sites.find((candidate) => candidate.code) ?? built.sites[0]);
    };
    expect(dead("zero-settled-empty")).toBe(true);
    expect(dead("zero-settled-var")).toBe(false);
    expect(dead("zero-body-var")).toBe(false);
  });
  it("is unexecuted at a site that never ran, a run-claiming rule's violation too; a false positive where it ran elsewhere", () => {
    const findings = [finding("pending-async-unsuspendable-read", "violation")];
    const verdict = judgeLegal({ code: "PENDING_ASYNC_UNTRACKED_READ", site, typecheck: { ok: true }, checker: { ok: true, findings }, runtime: runtime({ hits: { 4: 0 } }) });
    expect(verdict.outcome).toBe("unexecuted");
    expect(judgeLegal({ code: "PENDING_ASYNC_UNTRACKED_READ", site, typecheck: { ok: true }, checker: { ok: true, findings }, runtime: runtime() }).outcome).toBe("false-positive");
    const elsewhere = [finding("pending-async-unsuspendable-read", "violation", 30)];
    expect(judgeLegal({ code: "PENDING_ASYNC_UNTRACKED_READ", site, typecheck: { ok: true }, checker: { ok: true, findings: elsewhere }, runtime: runtime() }).outcome).toBe("false-positive");
  });

  it("gives a -guarded-rerun apply a later run: its compute reads a signal a button sets to 2", () => {
    for (const context of ["effect-apply-guarded-rerun", "render-apply-guarded-rerun"]) {
      expect(SPEC.PENDING_ASYNC_UNTRACKED_READ.legal).toContain(context);
      const { built } = generateLegal({ seed: 1 }, { code: "PENDING_ASYNC_UNTRACKED_READ", context }, { shape: legalShapes("PENDING_ASYNC_UNTRACKED_READ")[0] });
      expect(built.text).toMatch(/return t1\(\);\n\s+\}, \(v\) => \{\n\s+if \(v === 1\) return;/);
      expect(built.text).toMatch(/onClick=\{\(\) => \{\n\s+setT1\(2\);/);
      expect(built.plan.clicks).toEqual(["b1"]);
    }
  });

  it("puts a -guarded apply's site behind a return its first run takes", () => {
    for (const context of ["effect-apply-guarded", "render-apply-guarded", "effect-apply-guarded-closure", "render-apply-guarded-closure"]) {
      expect(SPEC.PENDING_ASYNC_UNTRACKED_READ.legal).toContain(context);
      const { built } = generateLegal({ seed: 1 }, { code: "PENDING_ASYNC_UNTRACKED_READ", context }, { shape: legalShapes("PENDING_ASYNC_UNTRACKED_READ")[0] });
      expect(built.files["App.tsx"]).toMatch(/\n\s+if \(v === 1\) return;\n\s+\{/);
    }
  });

  it("is a harness failure when the runtime failed before the site ran: a module that would not load says nothing", () => {
    const failed = runtime({ failure: { phase: "import", message: "Failed to resolve import \"./helpers\"" }, hits: null });
    expect(judge({ runtime: failed })).toMatchObject({ outcome: "harness" });
    expect(judge({ runtime: runtime({ failure: { phase: "plan", message: "late" } }) }).outcome).toBe("clean");
  });
  it("withholds clean from a disturbed run that never showed the site ran: its legality went unobserved", () => {
    const thrown = { errors: [{ kind: "page-error", message: "boom", positions: [] }] };
    expect(judge({ runtime: runtime({ ...thrown, hits: { 4: 0 } }) })).toMatchObject({ outcome: "disturbed", detail: { checker: "clean" } });
    expect(judge({ runtime: runtime({ ...thrown, hits: null }) }).outcome).toBe("disturbed");
    // The site ran before the throw: observed.
    expect(judge({ runtime: runtime(thrown) }).outcome).toBe("clean");
  });
  it("is not legal when the runtime reports the code: the template is wrong there", () => {
    const fired = runtime({ diagnostics: [{ code: "STORE_WRITE_OUTSIDE_SETTER", positions: [{ file: "App.tsx", line: 10 }] }] });
    expect(judge({ runtime: fired }).outcome).toBe("not-legal");
  });
  // ADR 0979: this assertion said the code reported at another line excused a violation at the site (`not.toBe("false-positive")`). A violation at the site is
  // accounted for at the site; the code reported elsewhere accounts for nothing here, so the row is a false-positive candidate.
  it("is a false-positive candidate when the runtime reports one of the rule's codes only elsewhere (was: no false positive, ADR 0979)", () => {
    const elsewhere = runtime({ diagnostics: [{ code: "STORE_WRITE_OUTSIDE_SETTER", positions: [{ file: "App.tsx", line: 30 }] }] });
    expect(judgeLegal({ code: "REACTIVE_WRITE_IN_OWNED_SCOPE", site, typecheck: { ok: true }, checker: { ok: true, findings: [finding("no-store-mutation-outside-setter", "violation")] }, runtime: elsewhere }).outcome).toBe("false-positive");
  });
  it("is no false positive when the runtime reports one of the rule's codes at the site, or a frame of its stack is on the site", () => {
    const at = (positions) => runtime({ diagnostics: [{ code: "STORE_WRITE_OUTSIDE_SETTER", positions }] });
    const violation = { ok: true, findings: [finding("no-store-mutation-outside-setter", "violation")] };
    for (const positions of [[{ file: "App.tsx", line: 10 }], [{ file: "App.tsx", line: 30 }, { file: "App.tsx", line: 10 }]]) {
      expect(judgeLegal({ code: "REACTIVE_WRITE_IN_OWNED_SCOPE", site, typecheck: { ok: true }, checker: violation, runtime: at(positions) }).outcome).not.toBe("false-positive");
    }
  });
  it("accounts for a lone violation of a placeless code by the run's one diagnostic, and flags two sites against one", () => {
    const flush = runtime({ diagnostics: [{ code: "ASYNC_OUTSIDE_LOADING_BOUNDARY", positions: [{ file: "App.tsx", line: 99 }] }] });
    const rule = "async-outside-loading-boundary";
    expect(judgeLegal({ code: "PENDING_ASYNC_UNTRACKED_READ", site, typecheck: { ok: true }, checker: { ok: true, findings: [finding(rule, "violation")] }, runtime: flush }).outcome).not.toBe("false-positive");
    expect(judgeLegal({ code: "PENDING_ASYNC_UNTRACKED_READ", site, typecheck: { ok: true }, checker: { ok: true, findings: [finding(rule, "violation"), finding(rule, "violation", 20)] }, runtime: flush }).outcome).toBe("false-positive");
  });
});

describe("the legal cells", () => {
  it("lists every code with legal contexts, and builds a program of the shape asked in each", () => {
    const cells = legalCells();
    expect(cells.length).toBeGreaterThan(20);
    for (const { code, context } of cells) expect(SPEC[code].legal).toContain(context);
    const { program, built } = generateLegal({ seed: 1 }, { code: "STORE_WRITE_OUTSIDE_SETTER", context: "handler" }, { shape: { variant: "draft", binding: "direct", layer: "none" } });
    expect(program.components[0].misuse).toMatchObject({ code: "STORE_WRITE_OUTSIDE_SETTER", context: "handler", variant: "draft" });
    expect(built.text).toContain("setS1((draft) => { draft.v = 7; })");
    expect(siteOf(built).code).toBe("STORE_WRITE_OUTSIDE_SETTER");
    expect(legalShapes("STORE_WRITE_OUTSIDE_SETTER").every((shape) => ["draft", "in-setter"].includes(shape.variant))).toBe(true);
  });
});

describe("a legal program whose JSX reads a pending value (misuse.mjs legalLoading)", () => {
  it("renders its view under <Loading>, so rc.17 reports no placeless ASYNC_OUTSIDE_LOADING_BOUNDARY that would leave it unjudged", () => {
    const memo = generateLegal({ seed: 1 }, { code: "PENDING_ASYNC_UNTRACKED_READ", context: "memo" }, { shape: { variant: null, binding: "direct", layer: "none", callback: "const" } }).built.text;
    expect(memo).toContain("<Loading fallback={0}>");
    const guarded = generateLegal({ seed: 1 }, { code: "PENDING_ASYNC_UNTRACKED_READ", context: "effect-apply-guarded" }, { shape: { variant: null, binding: "direct", layer: "none" } }).built.text;
    expect(guarded).not.toContain("<Loading");
    expect(generateLegal({ seed: 1 }, { code: "STRICT_READ_UNTRACKED", context: "jsx" }, { shape: { variant: "latest", binding: "direct", layer: "none" } }).built.text).toContain("<Loading fallback={0}>");
    expect(generateLegal({ seed: 1 }, { code: "STRICT_READ_UNTRACKED", context: "jsx" }, { shape: { variant: "signal", binding: "direct", layer: "none" } }).built.text).not.toContain("<Loading");
    expect(SPEC.STRICT_READ_UNTRACKED.variantContexts.latest).not.toContain("render-compute");
  });
});

// `legalCells` once took TEMPLATE_CODES alone: a recall-only code's legal cells were declared and never generated, a silent blind spot.
describe("the legal grids' reach", () => {
  it("gives every context a code declares legal programs, a recall-only code too", () => {
    const empty = [];
    const declared = (code) => [...(SPEC[code].legal ?? []), ...(SPEC[code].recallLegal ?? [])];
    const measured = [...TEMPLATE_CODES, ...RECALL_TEMPLATE_CODES];
    // An advisory code is measured only as recall-only (misuse.mjs MISUSE_CODES, ADR 0668): none is left out with legal cells.
    expect(Object.keys(SPEC).filter((code) => !SPEC[code].typedAway && !measured.includes(code) && declared(code).length)).toEqual([]);
    for (const code of measured) {
      for (const context of declared(code)) {
        const cell = legalCells({ codes: [code], recall: true }).find((candidate) => candidate.context === context);
        const shapes = [...legalShapes(code, ["root"], ["local"], context), ...recallLegalShapes(code, context)];
        if (!cell || !shapes.length) empty.push(`${code}/${context}`);
      }
    }
    expect(empty).toEqual([]);
  });
  it("generates a program in every legal cell of a recall-only code", () => {
    for (const code of RECALL_TEMPLATE_CODES) {
      for (const cell of legalCells({ codes: [code], recall: true })) {
        const [shape] = recallLegalShapes(code, cell.context);
        expect(shape, `${code}/${cell.context}`).toBeDefined();
        expect(generateLegal({ seed: 1 }, cell, { shape }).built.files["App.tsx"]).toContain("export function");
      }
    }
  });
});
