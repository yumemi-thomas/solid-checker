// ADR 0957: the package site as a default twin. Every source (misuse.mjs SOURCES) and every context a cell lists (a code's base, recall and legal contexts) can
// be generated with its site inside a package, reached through an entry (package-site.mjs). `packageSiteOf` builds one such program for a source or a context,
// or says there is none; package-site.test.mjs fails for a source or a context that has none and no reason below. A reason is the only way to leave one out,
// and an exemption for something that has a package variant fails too, so the lists cannot go stale.
import { buildCase } from "./emit.mjs";
import { generateProgram } from "./generate.mjs";
import { cellList, gridShapes, plainShape, recallShapes } from "./matrix.mjs";
import { legalCells, legalShapes, recallLegalShapes } from "./legal.mjs";
import { SPEC, gridVariantsOf, sourcesOf } from "./misuse.mjs";
import { validate, walk } from "./model.mjs";

/**
 * Sources with no package variant, each with its reason. None now: `package-export` and `package-hook`, which looked like candidates (the site already reads a
 * dependency's context, or is in a package), take the generic twin (the project keeps its import of `fz-ctx` and hands the context or the hook to `fz-site`).
 */
export const PACKAGE_SITE_EXEMPT_SOURCES = {};
/** Contexts with no package variant, each with its reason (filled from what building them showed). */
export const PACKAGE_SITE_EXEMPT_CONTEXTS = {};

const plain = { binding: "direct", layer: "none" };
const programOf = (plan) => generateProgram(7, { profile: "misuse", misusePlan: { ...plain, pkg: "call", ...plan } });

/**
 * Whether `built` is a program whose sites are in the package `fz-site`: every `use` is a package site (a context that repeats the site, the stacked pairs of
 * `aw-*`, holds two), the entry is in the text and the package and its files are in the case.
 */
export function holdsPackageSite(built, program) {
  const uses = [...walk(program)].filter(({ node }) => node.k === "use");
  return validate(program).length === 0 && uses.length >= 1 && uses.every(({ node }) => node.pkg) && built.sites.length >= 1 && Boolean(built.packages?.["fz-site"]?.["index.js"]) && /\b(site1|make1|Site1|SiteDyn1|SiteLazy1)\b/.test(built.text);
}

/** `{ plan, built }` of a package-site program with the given `source` or `context` (and `code` where it narrows the context), or `null`; `error` says why not. */
export function packageSiteOf({ source, context, code } = {}) {
  const tries = [];
  if (source !== undefined) {
    // The first (code, variant, context) that takes the source on the direct binding.
    for (const one of Object.keys(SPEC)) {
      if (SPEC[one].typedAway || typeof SPEC[one].call !== "function") continue;
      for (const variant of [null, ...gridVariantsOf(one)]) for (const where of SPEC[one].contexts ?? []) {
        if (sourcesOf(one, variant, "direct", where).includes(source) && source !== "local") tries.push({ code: one, context: where, variant, source });
        else if (source === "local") tries.push({ code: one, context: where, variant });
        if (tries.length) break;
      }
      if (tries.length) break;
    }
  } else {
    const cells = [...cellList({ codes: code ? [code] : undefined, recall: true }), ...legalCells({ codes: code ? [code] : undefined, recall: true }).map((cell) => ({ ...cell, legal: true }))];
    for (const cell of cells.filter((one) => one.context === context)) {
      // A cell only the recall grids list has its plain shapes there (`recallVariants`, a context only some variants take).
      const shapes = [...(cell.legal ? legalShapes(cell.code, ["root"], ["local"], cell.context) : gridShapes(cell.code, cell.context)), ...(cell.legal ? recallLegalShapes(cell.code, cell.context) : recallShapes(cell.code, cell.context))].filter(plainShape);
      for (const shape of shapes) tries.push({ code: cell.code, context: cell.context, ...(cell.legal ? { legal: true } : {}), ...shape });
      if (tries.length) break;
    }
  }
  if (!tries.length) return { error: "no cell takes it" };
  const errors = [];
  for (const plan of tries.slice(0, 3)) {
    try {
      const program = programOf(plan);
      const built = buildCase(program);
      if (holdsPackageSite(built, program)) return { plan, built };
      errors.push(`${plan.code}/${plan.context}: the program does not hold its site in the package`);
    } catch (error) { errors.push(`${plan.code}/${plan.context}: ${String(error.message).slice(0, 160)}`); }
  }
  return { error: errors.join("; ") };
}
