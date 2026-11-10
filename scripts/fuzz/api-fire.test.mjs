// Run: bun packages/cli/node_modules/vitest/vitest.mjs run --config packages/cli/vitest.config.mjs scripts/fuzz/api-fire.test.mjs
// The rc.17 API variants, the lazy-memo / deferred-apply contexts (misuse.mjs API_VARIANTS, API_CONTEXTS), and the later-fire,
// named-handler, never-run and attached-callback-prop contexts (FIRE_CONTEXTS, HANDLER_CONTEXTS, DEAD_CONTEXTS) do what their cells say on
// the fast runtime: a misuse cell reports its code at the site, a legal cell reports no defect code (and its site ran, or, for a site
// rc.17 never runs, did not run).
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { runtimeEvents } from "./classify.mjs";
import { policyOf } from "./codes.mjs";
import { buildCase, writeCase } from "./emit.mjs";
import { DEFAULT_FAST_INSTALL, runFast } from "./fast.mjs";
import { generateProgram } from "./generate.mjs";
import { hitCount } from "./hits.mjs";
import { legalCells } from "./legal.mjs";
import { cellList, judge, recallShapes, siteOf } from "./matrix.mjs";
import { API_CONTEXTS, API_VARIANTS, APPLY_VOID_UNRUN, BASE_ONLY_CONTEXTS, CBP_CONTEXTS, DEAD_CONTEXTS, SPEC, recallVariantsOf } from "./misuse.mjs";
import { recallLegalShapes } from "./legal.mjs";
import { WRAP_CONTEXT_NAMES } from "./boundary-shapes.mjs";
import { ARM_CONTEXT_NAMES } from "./arm-shapes.mjs";
import { TABLE_CONTEXT_NAMES } from "./table-shapes.mjs";

// ADR 0927: an apply that only names the compute's returned function never runs it.
const NEVER_RUN = ["memo-lazy-unread", "effect-apply-defer", ...DEAD_CONTEXTS, ...APPLY_VOID_UNRUN];
/** The contexts of the later-fire, named-handler, never-run and attached-callback-prop cells: one program a (code, context). */
// ADR 0935: the table family, the boundary wrappers and the arm shapes are base-only contexts too, but their legal twins run nothing by design (a scheduler's
// callback, an observer's, a generator nothing steps): the Chrome host judges them (focus.mjs) and boundary-scan.mjs screens them, not this fast-runtime test.
const EQUIVALENT = new Set([...BASE_ONLY_CONTEXTS().filter((context) => !API_CONTEXTS.includes(context) && !TABLE_CONTEXT_NAMES.includes(context) && !WRAP_CONTEXT_NAMES.includes(context) && !ARM_CONTEXT_NAMES.includes(context)),
  ...CBP_CONTEXTS.filter((context) => context.startsWith("cbp-attached"))]);

/** One program per (code, context, variant) the recall grids hold for the API variants and contexts, direct, no layer. */
function plans() {
  const out = [];
  for (const [kind, cells, shapesOf] of [["misuse", cellList({ recall: true }), recallShapes], ["legal", legalCells({ recall: true }), recallLegalShapes]]) {
    for (const cell of cells) {
      const variants = new Set((API_VARIANTS[cell.code] ?? []).filter((variant) => kind === "legal" ? recallVariantsOf(cell.code, { legal: true }).includes(variant) : recallVariantsOf(cell.code).includes(variant)));
      const shapes = shapesOf(cell.code, cell.context).filter((shape) => !shape.mount && !shape.source && !shape.callback && shape.layer === "none" && shape.binding === "direct");
      const wanted = shapes.filter((shape) => variants.has(shape.variant) || API_CONTEXTS.includes(cell.context) || EQUIVALENT.has(cell.context)).slice(0, EQUIVALENT.has(cell.context) ? 1 : Infinity);
      const seen = new Set();
      for (const shape of wanted) if (!seen.has(shape.variant)) { seen.add(shape.variant); out.push({ kind, ...cell, variant: shape.variant ?? null }); }
    }
  }
  return out;
}

describe("the API variants and contexts on the fast runtime", () => {
  it("fire in every misuse cell, and report nothing in every legal one", async () => {
    const root = mkdtempSync(join(tmpdir(), "fuzz-api-"));
    try {
      const entries = plans().map((plan, index) => {
        const program = generateProgram(5000 + index, { profile: "misuse", misusePlan: { code: plan.code, context: plan.context, variant: plan.variant, binding: "direct", layer: "none", legal: plan.kind === "legal" } });
        return { plan, built: buildCase(program, { id: `api-${index}` }) };
      });
      expect(entries.length).toBeGreaterThan(450);
      const records = await runFast(entries.map(({ built }) => ({ id: built.id, dir: writeCase(join(root, built.id), built), variant: "run" })), { install: DEFAULT_FAST_INSTALL, workers: 4 });
      const wrong = [];
      for (const { plan, built } of entries) {
        const runtime = records.get(built.id);
        const site = siteOf(built);
        const label = `${plan.kind} ${plan.code}/${plan.context}/${plan.variant}`;
        if (plan.kind === "misuse") {
          const verdict = judge({ code: plan.code, site, typecheck: { ok: true }, checker: { ok: true, findings: [] }, runtime });
          if (verdict.outcome !== "false-clean") wrong.push(`${label}: ${verdict.outcome} ${JSON.stringify(verdict.detail)}`);
          continue;
        }
        // A defect code (codes.mjs): an advisory one (ASYNC_OUTSIDE_LOADING_BOUNDARY, a pending read with no boundary) is legal.
        const reported = runtimeEvents(runtime ?? {}).map((event) => event.code).filter((code) => policyOf(code)?.disposition?.startsWith("defect"));
        const ran = hitCount(runtime?.hits ?? null, site.hit);
        if (reported.length) wrong.push(`${label}: reported ${reported.join(",")}`);
        if (NEVER_RUN.includes(plan.context) ? ran !== 0 : !(ran > 0)) wrong.push(`${label}: ran ${ran}`);
      }
      expect(wrong).toEqual([]);
    } finally { rmSync(root, { recursive: true, force: true }); }
  }, 300000);
});

describe("the API cells' shapes", () => {
  it("a lazy-memo or deferred-apply cell takes its base shapes only", () => {
    const shapes = recallShapes("STRICT_READ_UNTRACKED", "effect-apply-defer-rerun");
    expect(shapes.length).toBeGreaterThan(0);
    expect(shapes.every((shape) => !shape.mount && !shape.callback && !shape.source && ["none", "closure", "helper", "iife", "closure+helper", "module-helper"].includes(shape.layer))).toBe(true);
  });
  it("a legal cell only some variants take (an owned-write signal in a body) takes only those", () => {
    expect(SPEC.REACTIVE_WRITE_IN_OWNED_SCOPE.variantOnlyLegal.body.sort()).toEqual(["optimistic", "owned-write"]);
    const shapes = recallLegalShapes("REACTIVE_WRITE_IN_OWNED_SCOPE", "body");
    expect(new Set(shapes.map((shape) => shape.variant))).toEqual(new Set(["optimistic", "owned-write"]));
    expect(SPEC.REACTIVE_WRITE_IN_OWNED_SCOPE.legal).not.toContain("body");
    expect(SPEC.STRICT_READ_UNTRACKED.variantOnlyLegal.settled.sort()).toEqual(["is-pending", "latest"]);
  });
});
