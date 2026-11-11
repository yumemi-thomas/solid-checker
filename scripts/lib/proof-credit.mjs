// Batch 11, slice 0 (measurement only): who ends a site. The proof log (ADR 0933) lists the sites of the independent universe and the findings the final program has; today a
// site is accounted for when ANY rule's finding stands at it. This fold gives each site three fields and changes nothing else:
//
//   code      the Chrome runtime code the site's table answers (`SITE_CODE`: a read is STRICT_READ_UNTRACKED's, a write REACTIVE_WRITE_IN_OWNED_SCOPE's, ...), so the policy of
//             scripts/fuzz/codes.mjs (`codeRules`: which rules claim the code, `declinesCode`: which rule's obligation declines it) can be asked of the site;
//   by        the findings (verdicts of the backend, the final program) that stand at the site or at one of its alternate spans: `{ rule, id, kind }`, violations first;
//   credited  `claims` (a finding of a rule that claims `code`: its violation or its own obligation), `declines` (an obligation of a rule that declines it), `other` (a finding stands
//             there and none is the claiming or declining rule's: the site is ended only by ANOTHER rule's finding: the any-rule-credit list), or `none` (no finding: the site ends
//             in a row's census, a discard, a named silence or nothing).
//
// Nothing here reads a rule's behaviour back into the checker: the fold is of a log a run already wrote, and `scripts/proof-credit.mjs` only counts.
import { codeRules, declinesCode } from "../fuzz/codes.mjs";
import { foldFiles } from "./proof-log.mjs";

/** The Chrome runtime code each table of the site universe (rust/crates/solid-reactive-ir/src/proof_universe.rs) answers. */
export const SITE_CODE = {
  read: "STRICT_READ_UNTRACKED",
  write: "REACTIVE_WRITE_IN_OWNED_SCOPE",
  action: "ACTION_CALLED_IN_OWNED_SCOPE",
  "store-write": "STORE_WRITE_OUTSIDE_SETTER",
  "on-flush": "ON_FLUSH_IN_OWNED_SCOPE",
};

const spanKey = (line) => `${line.s}:${line.e}`;

/**
 * What a finding of `rule` and `kind` is to a site of `code`: `claims` (a finding of a rule that claims the code, its violation or its own obligation: the site is that rule's own), `declines`
 * (the obligation of a rule that declines it without claiming it), or neither (`null`: another rule's finding stands at the site).
 */
export function creditOf(code, { rule, kind }) {
  if ((codeRules[code] ?? []).includes(rule)) return "claims";
  if (declinesCode(code, { rule, kind })) return "declines";
  return null;
}

/**
 * Every site of the log's universe with its `code`, `by` and `credited`: `[{ file, s, e, table, source, via, end, detail, code, by, credited }]`, in the log's order.
 * A finding with no `rule` (a log from before the field) is credited to nobody, never guessed: its site reads `other`.
 */
export function siteCredits(lines) {
  const credits = [];
  for (const [file, tables] of foldFiles(lines)) {
    const verdicts = new Map();
    for (const verdict of tables.verdict) {
      const key = spanKey(verdict);
      if (!verdicts.has(key)) verdicts.set(key, []);
      verdicts.get(key).push(verdict);
    }
    for (const site of tables.site) {
      const code = SITE_CODE[site.t] ?? null;
      const spans = new Set([spanKey(site), ...(site.alt ?? []).map(([s, e]) => `${s}:${e}`)]);
      const found = [...spans].flatMap((key) => verdicts.get(key) ?? []);
      const by = [...new Map(found.map((verdict) => [`${verdict.rule}|${verdict.code}|${verdict.kind}`, { rule: verdict.rule ?? null, id: verdict.code, kind: verdict.kind }])).values()]
        .sort((a, b) => (a.kind === b.kind ? 0 : a.kind === "violation" ? -1 : 1));
      let credited = "none";
      if (by.length > 0) credited = code === null ? "other" : by.map((one) => (one.rule === null ? null : creditOf(code, one))).find(Boolean) ?? "other";
      credits.push({ file, s: site.s, e: site.e, table: site.t, source: site.source, via: site.via, end: site.end, detail: site.detail, code, by, credited });
    }
  }
  return credits;
}

/**
 * The counts of `credits` per code, and per rule that ended a site only as ANOTHER rule: `{ sites, byCredit, perCode: { code: { sites, claims, declines, other, none } },
 * otherBy: { "<code> <- <rule>": sites } }`.
 */
export function creditSummary(credits) {
  const summary = { sites: credits.length, byCredit: { claims: 0, declines: 0, other: 0, none: 0 }, perCode: {}, otherBy: {} };
  for (const credit of credits) {
    summary.byCredit[credit.credited] += 1;
    const entry = (summary.perCode[credit.code ?? "(no code)"] ??= { sites: 0, claims: 0, declines: 0, other: 0, none: 0 });
    entry.sites += 1;
    entry[credit.credited] += 1;
    if (credit.credited === "other") for (const one of credit.by) summary.otherBy[`${credit.code} <- ${one.rule ?? one.id}`] = (summary.otherBy[`${credit.code} <- ${one.rule ?? one.id}`] ?? 0) + 1;
  }
  return summary;
}
