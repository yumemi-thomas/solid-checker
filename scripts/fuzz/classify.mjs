// The comparator: the checker's verdicts and the runtime's verdicts for one program
// -> { verdict, items, reasons }. Pure; unit-tested on hand-written records.
//
// Verdicts, worst first:
//   candidate-false-positive  the checker proves a defect at a site the runtime ran without a
//                             diagnostic, a throw, or any disturbance, and the page shows the
//                             site ran (its witness). The runtime is silent where the checker
//                             claims a certain failure. Chrome decides (the authority).
//   candidate-false-clean     the runtime reports a defect at a site no checker finding covers.
//                             (Phase 4: classified and reported, not yet hunted or shrunk.)
//   inconclusive              the evidence cannot decide: the site never ran, nothing shows it ran,
//                             the rule has no runtime counterpart, the program threw, the
//                             checker or the runtime failed. "Unresolved" is not evidence (invariant 2).
//   agree                     both sides say the same thing at every site, or both are silent.
//
// An uncertifiable finding is never a claim; at a runtime diagnostic it counts as covering it
// (the checker said it could not prove), the way the runtime oracle's `covered-uncertifiable` does.
import { codeOfThrown, codeRules, codesForRule, ignoredRules, ruleHasRuntimeCounterpart, withholdingRules } from "./codes.mjs";

export const VERDICTS = ["candidate-false-positive", "candidate-false-clean", "inconclusive", "agree"];
const rank = (verdict) => VERDICTS.indexOf(verdict);

const inSpan = (finding, line) => finding.line <= line && line <= finding.endLine;
const lines = (positions) => (positions ?? []).map((position) => position.line);

/** Diagnostics and coded thrown errors, one row per (code, innermost App position), repeats merged. */
export function runtimeEvents(runtime) {
  const rows = [];
  for (const diagnostic of runtime.diagnostics ?? []) {
    rows.push({ code: diagnostic.code, source: "diagnostic", message: diagnostic.message ?? null, positions: diagnostic.positions ?? [] });
  }
  for (const error of runtime.errors ?? []) {
    const code = codeOfThrown(error.message) ?? codeOfThrown(error.cause);
    if (code) rows.push({ code, source: "thrown", message: error.message, positions: error.positions ?? [] });
  }
  const seen = new Set();
  return rows.filter((row) => {
    const key = `${row.code}|${row.source}|${row.positions[0]?.line ?? "-"}`;
    return !seen.has(key) && seen.add(key);
  });
}

/** What disturbs the whole run: a throw (any, anywhere), reactivity halted, a harness failure. */
export function disturbances(runtime, plan) {
  const reasons = [];
  if (runtime.failure) reasons.push(`runner failure in ${runtime.failure.phase}: ${String(runtime.failure.message).slice(0, 120)}`);
  const thrown = (runtime.errors ?? []).filter((error) => !["plan"].includes(error.kind));
  for (const error of thrown.slice(0, 3)) reasons.push(`the program threw (${error.kind}): ${String(error.message).slice(0, 100)}`);
  if ((runtime.diagnostics ?? []).some((diagnostic) => diagnostic.code === "REACTIVITY_HALTED")) reasons.push("reactivity halted");
  for (const error of (runtime.errors ?? []).filter((e) => e.kind === "plan")) reasons.push(`driving plan: ${error.message}`);
  return reasons;
}

/**
 * @param {object} input
 * @param {{id:string,line:number,kind:string,witness?:object}[]} input.sites
 * @param {{ok:boolean,error?:string,findings:{rule:string,id?:string,kind:string,line:number,endLine:number}[]}} input.checker
 * @param {{failure?:object,diagnostics:object[],errors:object[],witnesses:{siteId:string,reached:boolean|null}[]}} input.runtime
 * @param {{clicks:string[]}} [input.plan]
 */
export function classify({ sites, checker, runtime, plan }) {
  const items = [];
  const add = (kind, reason, extra = {}) => items.push({ kind, reason, ...extra });

  if (!checker.ok) return result([{ kind: "inconclusive", reason: `checker failed: ${String(checker.error).slice(0, 160)}` }]);
  if (runtime.failure && !(runtime.witnesses ?? []).length) {
    return result([{ kind: "inconclusive", reason: `runtime failed in ${runtime.failure.phase}: ${String(runtime.failure.message).slice(0, 160)}` }]);
  }

  const events = runtimeEvents(runtime);
  const disturbed = disturbances(runtime, plan);
  const reached = new Map((runtime.witnesses ?? []).map((w) => [w.siteId, w.reached]));
  const findings = checker.findings.filter((finding) => !ignoredRules.includes(finding.rule));

  // A checker violation against the runtime.
  for (const finding of findings.filter((f) => f.kind === "violation")) {
    const label = { rule: finding.rule, id: finding.id ?? null, line: finding.line };
    const covered = sites.filter((site) => inSpan(finding, site.line));
    if (!ruleHasRuntimeCounterpart(finding.rule)) { add("inconclusive", `no runtime diagnostic claims rule ${finding.rule}`, { finding: label }); continue; }
    if (!covered.length) { add("inconclusive", "the finding spans no site of interest", { finding: label }); continue; }
    const codes = codesForRule(finding.rule);
    const here = events.filter((event) => lines(event.positions).some((line) => inSpan(finding, line)));
    if (here.some((event) => codes.includes(event.code))) { add("agree", `runtime ${here.find((e) => codes.includes(e.code)).code} at the finding`, { finding: label, siteIds: covered.map((s) => s.id) }); continue; }
    if (here.length) { add("inconclusive", `runtime reports ${[...new Set(here.map((e) => e.code))].join(", ")} at the finding, not a code that claims ${finding.rule}`, { finding: label }); continue; }
    if (disturbed.length) { add("inconclusive", `the run was disturbed: ${disturbed[0]}`, { finding: label }); continue; }
    const states = covered.map((site) => reached.get(site.id) ?? null);
    if (states.some((state) => state === true)) {
      add("candidate-false-positive", `${finding.rule} (${finding.id ?? "?"}) is a violation at line ${finding.line}; the runtime ran the site (witness shown) with no diagnostic and no throw`,
        { finding: label, siteIds: covered.filter((site) => reached.get(site.id) === true).map((site) => site.id) });
    } else if (states.every((state) => state === null)) {
      add("inconclusive", "the runtime is silent, but nothing shows the site ran (no witness)", { finding: label });
    } else {
      add("inconclusive", "the runtime is silent, but the site's witness is absent: the site did not run, or did not land", { finding: label });
    }
  }

  // An uncertifiable finding at a runtime diagnostic is the checker declining; it covers.
  const spans = findings;
  // A runtime diagnostic against the checker (phase 4).
  for (const event of events) {
    const rules = codeRules[event.code];
    const attributed = event.positions.filter((position) => position.file === "App.tsx" || position.file === undefined);
    if (!rules || !attributed.length) continue;
    const covering = spans.filter((finding) => attributed.some((position) => inSpan(finding, position.line))
      && (rules.includes(finding.rule) || withholdingRules.includes(finding.rule)));
    if (covering.some((finding) => finding.kind === "violation")) continue; // already an agree through the finding above
    if (covering.length) { add("agree", `${event.code}: the checker declines (uncertifiable) at line ${attributed[0].line}`, { diagnostic: { code: event.code, line: attributed[0].line } }); continue; }
    add("candidate-false-clean", `${event.code} at line ${attributed[0].line} has no finding of a rule that claims it`, { diagnostic: { code: event.code, line: attributed[0].line, message: event.message }, provisional: true });
  }

  if (!items.length) add("agree", disturbed.length ? `no claims to compare (${disturbed[0]})` : "the checker and the runtime are both silent");
  return result(items);

  function result(rows) {
    const verdict = rows.reduce((worst, row) => (rank(row.kind) < rank(worst) ? row.kind : worst), "agree");
    const reasons = [...new Set(rows.filter((row) => row.kind === verdict).map((row) => row.reason))];
    return { verdict, reasons, items: rows };
  }
}
