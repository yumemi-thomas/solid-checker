// One account of a checker violation against what Chrome reported (ADR 0979). The judges that read a violation (the pattern lane, the matrix and
// legal grids' off-site rows, the runtime oracle) used to ask "did Chrome report a code of this rule somewhere in the file?", so a true violation
// of a code Chrome reports in a file hid every false one of the same code in it. They ask here instead: is the violation accounted for by a
// diagnostic at its site?
//
//   - A code Chrome places (a stack with application frames): the violation is accounted for when a frame of a diagnostic of one of its rule's
//     codes is on the violation's lines (`frameAt`), as classify.mjs `offSiteVerdicts` already read a stack. A diagnostic of the code elsewhere
//     in the file accounts for nothing here. A diagnostic of such a code with no frame at all (nothing could be placed) may be the violation's:
//     `unplaced`, unjudged, never a false positive.
//   - A code Chrome cannot place (`unplaceableCodes` below, a subset of `placelessCodes` in codes.mjs: the diagnostic is raised from the scheduler or
//     after the callback returned, so its stack holds no application frame of the read): recall keeps the file judge, one diagnostic anywhere covers the checker's silence. For
//     precision it counts: the distinct violation sites of the rules that claim the code (not already placed by another code, and not known to
//     have never run: `ran`) against the diagnostics of the code in that file. More sites than diagnostics is `unaccounted` (`why: "excess"`).
//     It is a candidate, not a proof: a code the runtime raises once per mount or per computation (ASYNC_OUTSIDE_LOADING_BOUNDARY,
//     CAUGHT_REACTIVE_READ) is raised once for N true sites, so a person reads every candidate.
//
// A function of its inputs only: the judges pass the positions they already hold.

import { codeRules, codesForRule, placelessCodes } from "./codes.mjs";

/**
 * The codes whose diagnostic the pinned runtime (rc.21) raises with no application frame of the offending site: from the scheduler's flush
 * (ASYNC_OUTSIDE_LOADING_BOUNDARY, `dev.js` notifyEffectStatus 2655-2700, once per mount: `dev-shared.js` 2261-2273), from a boundary's re-arm
 * walk in the flush (LOADING_ON_OUTSIDE_HOLD, `dev.js` 10967 and 10981-11029, once per re-arm), from the settle of a computation's flight
 * (CAUGHT_REACTIVE_READ, `dev-shared.js` 1275-1296, once per computation: `caughtReadReported`), and from the queued one-shot's `fire`
 * (SETTLED_CLEANUP_UNOWNED, `dev.js` 3892-3914, once per fire). `placelessCodes` (codes.mjs) also lists LOADING_ON_ACCESSOR, which the
 * recall judges keep as they are; it is not here: `reportOnAccessor` runs in the boundary's first synchronous run (`dev.js` 11173-11175, once
 * per boundary), and Chrome's stack of it holds the `<Loading>` element's frame (a pattern with `on={() => route()}`: App.tsx 7:5, the
 * element, a mount frame), so a violation of its rule is placed by that frame like any placed code's.
 */
export const unplaceableCodes = placelessCodes.filter((code) => code !== "LOADING_ON_ACCESSOR");
export const isUnplaceable = (code) => unplaceableCodes.includes(code);

const fileOf = (position) => position?.file ?? "App.tsx";
/** A violation's place, the key two violations of one site share. */
export const siteKey = (violation) => `${fileOf(violation)}:${violation.line}:${violation.column ?? ""}`;

/** Whether a stack frame is on a violation's lines (the finding's span, `endLine`), in the violation's file (`sameFile`). */
export const frameAt = (violation, frame, sameFile = (one, other) => fileOf(one) === fileOf(other)) => Number.isInteger(frame?.line)
  && sameFile(violation, frame) && violation.line <= frame.line && frame.line <= (violation.endLine ?? violation.line);

/**
 * The violation sites of each placeless code that outnumber its diagnostics in their file.
 * @param {{ rule: string, file?: string, line: number, column?: number }[]} violations the sites to count (the caller passes every violation
 *   of the program: a site the checker reports twice under one rule is one site)
 * @param {{ code: string, file?: string }[]} occurrences one row per diagnostic Chrome raised, not merged by stack: a diagnostic with no
 *   `file` counts for every file (nothing says where it was raised)
 * @param {{ ran?: (violation: object) => boolean | null, placed?: (violation: object) => boolean, only?: string[] | null }} [hooks] `only`: count just these
 *   placeless codes (the oracle judges one of them by file, ADR 0946)
 * @returns {Map<string, { sites: number, occurrences: number, keys: Set<string> }>} `${code}|${file}` -> the excess
 */
export function placelessExcess(violations, occurrences, { ran = () => null, placed = () => false, only = null } = {}) {
  const excess = new Map();
  const codes = [...new Set(occurrences.map((row) => row.code).concat(violations.flatMap((violation) => codesForRule(violation.rule))))].filter((code) => isUnplaceable(code) && (!only || only.includes(code)));
  for (const code of codes) {
    const rules = codeRules[code] ?? [];
    const sites = new Map();
    for (const violation of violations) {
      if (rules.includes(violation.rule) && !placed(violation) && ran(violation) !== false) sites.set(`${fileOf(violation)}\0${siteKey(violation)}`, violation);
    }
    const files = new Set([...sites.values()].map(fileOf));
    for (const file of files) {
      const here = [...sites.values()].filter((violation) => fileOf(violation) === file);
      const raised = occurrences.filter((row) => row.code === code && (row.file === undefined || row.file === null || row.file === file)).length;
      if (here.length > raised) excess.set(`${code}|${file}`, { sites: here.length, occurrences: raised, keys: new Set(here.map(siteKey)) });
    }
  }
  return excess;
}

/**
 * Accounts for every violation against the diagnostics of its program.
 * @param {object} input
 * @param {{ rule: string, file?: string, line: number, endLine?: number, column?: number }[]} input.violations the violations to judge
 * @param {{ code: string, frames?: { file?: string, line: number, column?: number }[], file?: string }[]} input.diagnostics one row per
 *   diagnostic Chrome raised (and per coded throw), innermost frame first
 * @param {(violation: object, frame: object) => boolean} [input.connects] whether a frame places a violation (default: `frameAt`)
 * @param {(violation: object) => boolean | null} [input.ran] whether the violation's lines ran (null: unknown, counted as ran)
 * @param {object[]} [input.alsoCounted] violations already judged elsewhere (a grid's own site) that still count as sites of a placeless code
 * @returns {{ violation: object, verdict: "placed" | "counted" | "unplaced" | "unaccounted", code?: string, why?: "none" | "elsewhere" | "excess", sites?: number, occurrences?: number }[]}
 */
export function accountViolations({ violations, diagnostics, connects = frameAt, ran, alsoCounted = [] }) {
  const placedBy = (violation) => {
    const codes = codesForRule(violation.rule).filter((code) => !isUnplaceable(code));
    return diagnostics.find((row) => codes.includes(row.code) && (row.frames ?? []).some((frame) => connects(violation, frame)))?.code ?? null;
  };
  const placed = new Map(violations.map((violation) => [violation, placedBy(violation)]));
  const excess = placelessExcess([...violations, ...alsoCounted], diagnostics, { ran, placed: (violation) => Boolean(placed.get(violation)) });
  return violations.map((violation) => {
    const code = placed.get(violation);
    if (code) return { violation, verdict: "placed", code };
    const codes = codesForRule(violation.rule);
    const placeless = codes.filter(isUnplaceable);
    const raised = (name) => diagnostics.filter((row) => row.code === name && (row.file === undefined || row.file === null || row.file === fileOf(violation))).length;
    const over = placeless.map((name) => ({ name, found: excess.get(`${name}|${fileOf(violation)}`) })).find((one) => one.found?.keys.has(siteKey(violation)));
    // No diagnostic of the code at all is the plain "none" below, not an excess.
    if (over && over.found.occurrences > 0) return { violation, verdict: "unaccounted", code: over.name, why: "excess", sites: over.found.sites, occurrences: over.found.occurrences };
    const counted = placeless.find((name) => raised(name) > 0);
    if (counted) return { violation, verdict: "counted", code: counted };
    // A diagnostic of a code Chrome places that no authored frame could be given for may be this violation's: unjudged, never a false positive.
    const placedCodes = codes.filter((name) => !isUnplaceable(name));
    if (diagnostics.some((row) => placedCodes.includes(row.code) && !(row.frames ?? []).some((frame) => Number.isInteger(frame?.line)))) return { violation, verdict: "unplaced" };
    return { violation, verdict: "unaccounted", why: diagnostics.some((row) => codes.includes(row.code)) ? "elsewhere" : "none" };
  });
}
