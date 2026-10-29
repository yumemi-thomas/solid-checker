import { describe, expect, it } from "vitest";
import { generateLegal, judgeLegal, legalCells, legalShapes } from "./legal.mjs";
import { siteOf } from "./matrix.mjs";
import { SPEC } from "./misuse.mjs";

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
  it("is a false positive at a site that never ran when the rule claims it ran (SC5001, ADR 0504 review)", () => {
    const findings = [finding("pending-async-unsuspendable-read", "violation")];
    const verdict = judgeLegal({ code: "PENDING_ASYNC_UNTRACKED_READ", site, typecheck: { ok: true }, checker: { ok: true, findings }, runtime: runtime({ hits: { 4: 0 } }) });
    expect(verdict.outcome).toBe("false-positive");
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
  it("is not legal when the runtime reports the code: the template is wrong there", () => {
    const fired = runtime({ diagnostics: [{ code: "STORE_WRITE_OUTSIDE_SETTER", positions: [{ file: "App.tsx", line: 10 }] }] });
    expect(judge({ runtime: fired }).outcome).toBe("not-legal");
  });
  it("is no false positive when the runtime reports one of the rule's codes elsewhere", () => {
    const elsewhere = runtime({ diagnostics: [{ code: "STORE_WRITE_OUTSIDE_SETTER", positions: [{ file: "App.tsx", line: 30 }] }] });
    expect(judgeLegal({ code: "REACTIVE_WRITE_IN_OWNED_SCOPE", site, typecheck: { ok: true }, checker: { ok: true, findings: [finding("no-store-mutation-outside-setter", "violation")] }, runtime: elsewhere }).outcome).not.toBe("false-positive");
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
