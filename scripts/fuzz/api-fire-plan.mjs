// The plan of the API-fire test (api-fire-1of2.test.mjs, api-fire-2of2.test.mjs, api-fire-plan.test.mjs): one program per (code, context, variant) the recall grids hold for the
// rc.17 API variants, the lazy-memo / deferred-apply contexts (misuse.mjs API_VARIANTS, API_CONTEXTS), and the later-fire, named-handler, never-run and attached-callback-prop
// contexts (FIRE_CONTEXTS, HANDLER_CONTEXTS, DEAD_CONTEXTS): each does what its cell says on the fast runtime: a misuse cell reports its code at the site, a legal cell reports no
// defect code (and its site ran, or, for a site rc.17 never runs, did not run).
//
// Owner ruling (batch 10): the plan outgrew the 300 s limit of its one test (1,883 programs, 322 s alone on a quiet VM), and the limit is not raised: the plan is SPLIT into two
// halves, each its own test file with the same limit. `plans()` is the whole plan, as the one test built it; `partOf(0|1)` is a half: an entry goes to the half its cell's key hashes
// to (`halfOf`, a function of the entry alone, so adding a cell moves no other), and keeps its place `index` in the whole plan, from which its program's seed and id come, so every
// program is the program the one test ran. api-fire-plan.test.mjs holds that the halves do not overlap and their union is the plan.
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { runtimeEvents } from "./classify.mjs";
import { policyOf } from "./codes.mjs";
import { buildCase, writeCase } from "./emit.mjs";
import { DEFAULT_FAST_INSTALL, runFast } from "./fast.mjs";
import { generateProgram } from "./generate.mjs";
import { hitCount } from "./hits.mjs";
import { legalCells } from "./legal.mjs";
import { cellList, judge, recallShapes, siteOf } from "./matrix.mjs";
import { API_CONTEXTS, API_VARIANTS, APPLY_VOID_UNRUN, BASE_ONLY_CONTEXTS, CBP_CONTEXTS, DEAD_CONTEXTS, recallVariantsOf } from "./misuse.mjs";
import { recallLegalShapes } from "./legal.mjs";
import { WRAP_CONTEXT_NAMES } from "./boundary-shapes.mjs";
import { ARM_CONTEXT_NAMES } from "./arm-shapes.mjs";
import { SEAM_CONTEXT_NAMES } from "./seam-shapes.mjs";
import { REF_LAYER_CONTEXT_NAMES } from "./ref-layer-shapes.mjs";
import { ENTRY_CONTEXT_NAMES } from "./entry-shapes.mjs";
import { OB_CONTEXT_NAMES } from "./ob-shapes.mjs";
import { SF_CONTEXT_NAMES } from "./sf-shapes.mjs";
import { RV_CONTEXT_NAMES } from "./rv-shapes.mjs";
import { NM_CONTEXT_NAMES } from "./nm-shapes.mjs";
import { LA_CONTEXT_NAMES } from "./la-shapes.mjs";
import { TABLE_CONTEXT_NAMES } from "./table-shapes.mjs";
import { fnv } from "./rng.mjs";

export const PARTS = 2;

// ADR 0927: an apply that only names the compute's returned function never runs it.
export const NEVER_RUN = ["memo-lazy-unread", "effect-apply-defer", ...DEAD_CONTEXTS, ...APPLY_VOID_UNRUN];
/** The contexts of the later-fire, named-handler, never-run and attached-callback-prop cells: one program a (code, context). */
// ADR 0935: the table family, the boundary wrappers and the arm shapes are base-only contexts too, but their legal twins run nothing by design (a scheduler's
// callback, an observer's, a generator nothing steps): the Chrome host judges them (focus.mjs) and boundary-scan.mjs screens them, not this fast-runtime test.
// ADR 0962: so are the entry shapes (a generator reached through a helper, a dependency or `yield*`; a function entered twice), whose twins are the Chrome host's to judge:
// kept out, the plan stays what it was before them.
// ADR 0963: so are the object-boundary shapes (a site run by a getter, a coercion, an iterator step, a trap, a constructor, a field, a tag or an exported helper, and the twin that
// never runs it), judged by the Chrome host (focus.mjs): kept out, the plan stays what it was before them. ADR 0964: so are the stored-function shapes; batch 10c: so are the review spellings (`rs-*`, `rm-*`). Batch 11: so are the named-callback shapes (`nm-*`) and the later-assignment shapes (`la-*`).
const EQUIVALENT = new Set([...BASE_ONLY_CONTEXTS().filter((context) => !API_CONTEXTS.includes(context) && !TABLE_CONTEXT_NAMES.includes(context) && !WRAP_CONTEXT_NAMES.includes(context) && !ARM_CONTEXT_NAMES.includes(context) && !SEAM_CONTEXT_NAMES.includes(context) && !REF_LAYER_CONTEXT_NAMES.includes(context) && !ENTRY_CONTEXT_NAMES.includes(context) && !OB_CONTEXT_NAMES.includes(context) && !SF_CONTEXT_NAMES.includes(context) && !RV_CONTEXT_NAMES.includes(context) && !NM_CONTEXT_NAMES.includes(context) && !LA_CONTEXT_NAMES.includes(context)),
  ...CBP_CONTEXTS.filter((context) => context.startsWith("cbp-attached"))]);

/** One program per (code, context, variant) the recall grids hold for the API variants and contexts, direct, no layer. */
export function plans() {
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
  return out.map((plan, index) => ({ ...plan, index }));
}

/** The key of a plan entry: what makes it the entry it is. */
export const keyOf = (plan) => `${plan.kind}|${plan.code}|${plan.context}|${plan.variant}`;

/** The half (0 or 1) a plan entry belongs to: its key's hash, so it does not depend on the other entries. */
export const halfOf = (plan) => fnv(keyOf(plan)) % PARTS;

/** The entries of half `part` of the plan, in the plan's order, each with its place `index` in the whole plan. */
export const partOf = (part, plan = plans()) => plan.filter((entry) => halfOf(entry) === part);

/**
 * Generates the programs of `part` (as the one test did: seed `5000 + index`, id `api-${index}`), runs them on the fast runtime and returns what is wrong: `{ count, wrong }`.
 * `wrong` is empty when every misuse program reports its code at the site and every legal one reports no defect code and ran (or, never-run, did not).
 */
export async function fire(part) {
  const root = mkdtempSync(join(tmpdir(), "fuzz-api-"));
  try {
    const entries = part.map((plan) => {
      const program = generateProgram(5000 + plan.index, { profile: "misuse", misusePlan: { code: plan.code, context: plan.context, variant: plan.variant, binding: "direct", layer: "none", legal: plan.kind === "legal" } });
      return { plan, built: buildCase(program, { id: `api-${plan.index}` }) };
    });
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
    return { count: entries.length, wrong };
  } finally { rmSync(root, { recursive: true, force: true }); }
}
