// Mutation mode: the misuse grid against its legal twins, with the checker alone (no runtime, no Chrome).
//
// Equivalence mode (equivalence.mjs) rewrites a program without changing what it does and asks for the same verdict. This asks the
// converse: a misuse program and the legal program of the same shape (the same variant, binding, layer, mount, source and callback
// form, in a context of the template's `legal` list) do different things, the first a defect the code's rule claims, the second
// correct Solid. A checker that gives both the same answer cannot tell them apart. That is found without running either program, so
// it runs over whole grids, the recall axes included (`--recall`), in the time the checker takes.
//
// Per pair, at the misuse site (matrix.mjs regionOf; a claiming violation anywhere in a one-site file counts, as the matrix counts it):
//   distinguished   the misuse has a violation of a rule that claims the code; the legal twin has none
//   declined        the misuse has only an uncertifiable finding of a claiming rule (a recall gap the checker admits)
//   static-miss     no claiming finding on the misuse, none on the twin: the checker reads them alike. `certified` when the misuse
//                   file has no uncertifiable finding at all (the checker claims it proved the file: invariant 2)
//   legal-flagged   the legal twin has a claiming violation (a false-positive candidate; legal.mjs and Chrome decide)
//   typed-away      either program does not type-check (invariant 1: not evidence)
//   harness         the checker could not run
// A `static-miss` is a candidate, not a false clean: the misuse grid (matrix.mjs, with the runtime) says whether Solid reports the
// code there. A shape whose misuse never fires is `not-fired` there, and its static-miss says nothing.
//
//   node scripts/fuzz/mutation.mjs --out <fresh dir> [--seed 1] [--codes A,B] [--recall] [--checker-bin <solid-checker-rust>]
//     [--typefacts <solid-typefacts>] [--tsc <tsrs>] [--concurrency N] [--limit N]
import { existsSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { cpus } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { DEFAULT_ORACLE_INSTALL } from "./chrome.mjs";
import { assertFindingsMapped, materializeForChecker, runChecker } from "./checker.mjs";
import { codeRules, ignoredRules } from "./codes.mjs";
import { generateLegal } from "./legal.mjs";
import { cellList, generateShape, gridShapes, recallShapes, regionOf, siteOf } from "./matrix.mjs";
import { CALLBACK_CONTEXTS, SPEC, legalVariantsOf } from "./misuse.mjs";
import { requireTypeChecker, resolveTsc, typecheck, typecheckBatch } from "./typecheck.mjs";

export const MUTATION_OUTCOMES = ["distinguished", "declined", "static-miss", "legal-flagged", "typed-away", "harness"];

/**
 * The legal program a misuse shape is paired with: the first of the template's legal contexts that takes the shape (a callback form
 * needs a primitive's callback), with the shape's variant, or the first legal variant when the legal list has its own (a store write
 * through its draft for a write outside the setter). Null when the template has no legal context.
 */
export function legalPartner(code, shape) {
  const spec = SPEC[code];
  if (!spec.legal?.length) return null;
  const context = spec.legal.find((candidate) => !shape.callback || CALLBACK_CONTEXTS.includes(candidate));
  if (!context) return null;
  const variants = legalVariantsOf(code);
  return { context, variant: variants.includes(shape.variant ?? null) ? shape.variant ?? null : variants[0] };
}

/** A program's answer at its site for `code`: violation, uncertifiable or none, and whether the file holds any uncertifiable finding. */
export function siteAnswer(code, built, checker) {
  const rules = codeRules[code] ?? [];
  const region = regionOf(siteOf(built));
  const relevant = checker.findings.filter((finding) => !ignoredRules.includes(finding.rule));
  const claiming = relevant.filter((finding) => rules.includes(finding.rule));
  const covers = (finding) => region.some((line) => finding.line <= line && line <= finding.endLine);
  const answer = claiming.some((finding) => finding.kind === "violation") ? "violation"
    : claiming.some((finding) => covers(finding)) ? "uncertifiable" : "none";
  return { answer, anyUncertifiable: relevant.some((finding) => finding.kind !== "violation"), findings: claiming.map((finding) => `${finding.rule}/${finding.kind}@${finding.line}`) };
}

/** The pair's outcome from the two answers (see the header). */
export function pairOutcome(misuse, legal) {
  if (legal.answer === "violation") return { outcome: "legal-flagged" };
  if (misuse.answer === "violation") return { outcome: "distinguished" };
  if (misuse.answer === "uncertifiable") return { outcome: "declined" };
  return { outcome: "static-miss", certified: !misuse.anyUncertifiable };
}

async function pool(items, work, concurrency) {
  let next = 0;
  await Promise.all(Array.from({ length: Math.max(1, Math.min(concurrency, items.length)) }, async () => {
    while (next < items.length) { const index = next++; await work(items[index], index); }
  }));
}

export async function runMutation(options) {
  const out = resolve(options.out);
  if (existsSync(out)) throw new Error(`use a fresh --out path (${out} exists)`);
  const tsc = resolveTsc(options.tsc);
  requireTypeChecker(tsc);
  mkdirSync(out, { recursive: true });
  const install = options.install ?? DEFAULT_ORACLE_INSTALL;
  const concurrency = Number(options.concurrency ?? Math.max(1, Math.floor(cpus().length / 2)));
  const pairs = [];
  for (const cell of cellList({ codes: options.codes?.split(","), recall: options.recall })) {
    const shapes = options.recall ? recallShapes(cell.code, cell.context) : gridShapes(cell.code, cell.context);
    shapes.forEach((shape, index) => {
      const partner = legalPartner(cell.code, shape);
      if (partner) pairs.push({ cell, shape, index, legalContext: partner.context, legalShape: { ...shape, variant: partner.variant } });
    });
  }
  const chosen = options.limit ? pairs.slice(0, Number(options.limit)) : pairs;
  console.error(`mutation: ${chosen.length} pairs${chosen.length < pairs.length ? ` of ${pairs.length}` : ""}`);
  const rows = [];
  // In chunks: each chunk's projects are written, type-checked in `tsrs -b` batches, checked, and removed.
  const chunk = 256;
  for (let start = 0; start < chosen.length; start += chunk) {
    const part = chosen.slice(start, start + chunk).map((pair) => {
      const misuse = generateShape(options, pair.cell, pair.shape, pair.index).built;
      const legal = generateLegal(options, { code: pair.cell.code, context: pair.legalContext }, { shape: pair.legalShape }).built;
      return { ...pair, misuse, legal, misuseDir: materializeForChecker(misuse, install, `mu-${misuse.id}`), legalDir: materializeForChecker(legal, install, `mu-${legal.id}-l`) };
    });
    const dirs = part.flatMap((pair) => [pair.misuseDir, pair.legalDir]);
    const types = new Map();
    const groups = [];
    for (let index = 0; index < dirs.length; index += 32) groups.push(dirs.slice(index, index + 32));
    await pool(groups, async (group) => { const got = await typecheckBatch(group, { tsc }); for (const dir of group) types.set(dir, got?.get(dir) ?? await typecheck(dir, { tsc })); }, concurrency);
    await pool(part, async (pair) => {
      const row = { id: pair.misuse.id, legalId: pair.legal.id, code: pair.cell.code, context: pair.cell.context, legalContext: pair.legalContext, shape: pair.shape };
      if (!types.get(pair.misuseDir)?.ok || !types.get(pair.legalDir)?.ok) rows.push({ ...row, outcome: "typed-away" });
      else {
        const [misuseRun, legalRun] = [await runChecker(pair.misuseDir, { bin: options.checkerBin, typefacts: options.typefacts }), await runChecker(pair.legalDir, { bin: options.checkerBin, typefacts: options.typefacts })];
        if (!misuseRun.ok || !legalRun.ok) rows.push({ ...row, outcome: "harness", why: String(misuseRun.error ?? legalRun.error).slice(0, 200) });
        else {
          const misuse = siteAnswer(pair.cell.code, pair.misuse, misuseRun);
          const legal = siteAnswer(pair.cell.code, pair.legal, legalRun);
          rows.push({ ...row, ...pairOutcome(misuse, legal), misuse: misuse.findings, legal: legal.findings });
        }
      }
      if (!options.keepWork) for (const dir of [pair.misuseDir, pair.legalDir]) rmSync(dir, { recursive: true, force: true });
    }, concurrency);
    console.error(`mutation: ${Math.min(start + chunk, chosen.length)}/${chosen.length}`);
  }
  assertFindingsMapped();
  const summary = { pairs: rows.length, outcomes: Object.fromEntries(MUTATION_OUTCOMES.map((outcome) => [outcome, rows.filter((row) => row.outcome === outcome).length])), certifiedStaticMisses: rows.filter((row) => row.certified).length };
  const byCell = {};
  for (const row of rows) { const key = `${row.code}/${row.context}`; byCell[key] ??= Object.fromEntries(MUTATION_OUTCOMES.map((outcome) => [outcome, 0])); byCell[key][row.outcome] += 1; }
  writeFileSync(join(out, "mutation.jsonl"), rows.map((row) => JSON.stringify(row)).join("\n") + (rows.length ? "\n" : ""));
  writeFileSync(join(out, "mutation.json"), `${JSON.stringify({ seed: options.seed, recall: Boolean(options.recall), summary, byCell }, null, 2)}\n`);
  console.log(JSON.stringify(summary));
  return { summary, rows };
}

function parse(argv) {
  const options = { seed: "1" };
  for (let index = 0; index < argv.length; index += 1) {
    const key = argv[index].replace(/^--/, "").replace(/-([a-z])/g, (_, c) => c.toUpperCase());
    if (["recall", "keepWork"].includes(key)) { options[key] = true; continue; }
    if (!["out", "seed", "codes", "checkerBin", "typefacts", "tsc", "concurrency", "limit", "install"].includes(key) || argv[index + 1] === undefined) throw new Error(`mutation.mjs: unknown or empty option ${argv[index]}`);
    options[key] = argv[++index];
  }
  if (!options.out) throw new Error("mutation.mjs: --out <fresh dir>");
  return options;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  runMutation(parse(process.argv.slice(2))).catch((error) => { console.error(error.message); process.exitCode = 2; });
}
