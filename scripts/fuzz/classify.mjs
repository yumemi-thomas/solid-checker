// The comparator: the checker's verdicts and the runtime's verdicts for one program
// -> { verdict, items, reasons }. Pure; unit-tested on hand-written records.
//
// Verdicts, worst first:
//   candidate-false-positive  the checker proves a defect at a site the runtime ran without a
//                             diagnostic, a throw, or any disturbance, and the page shows the
//                             site ran (its witness). The runtime is silent where the checker
//                             claims a certain failure. Chrome decides (the authority).
//   candidate-false-clean     Solid reports a defect (a code the policy table calls a defect, codes.mjs) where no checker
//                             finding covers it. Chrome confirms, falseclean.mjs shrinks and dedupes.
//   candidate-wrong-rule      the runtime reports a defect at the finding, but under a code a different rule
//                             claims: the checker saw a real defect and named it wrongly (SC3001 for SC4001).
//   known-open                a false clean the ledger (known-open.json) accepts, by code and coarse key, with a reference.
//                             Assigned after classification (falseclean.mjs), never here.
//   recall-gap                Solid reports a defect and the checker declines (an uncertifiable finding at the
//                             diagnostic): counted and ranked as a roadmap metric, never a failure.
//   candidate-unexecuted      shown as `candidate-false-positive: unexecuted`. The checker proves a defect at a payload site the hit ledger
//                             shows never ran (unexecuted.mjs). A hypothesis: Chrome confirms it (`confirmed-unexecuted`, exit 3) or not.
//                             Ranked under recall-gap and above inconclusive, so only a row that was `inconclusive: never-ran` moves to it.
//   inconclusive              the evidence cannot decide: the site never ran, nothing shows it ran,
//                             the rule has no runtime counterpart, the program threw, the
//                             checker or the runtime failed. "Unresolved" is not evidence (invariant 2).
//   agree                     both sides say the same thing at every site, or both are silent.
//
// Evidence that a site ran has two parts that are not the same fact. `ran` is the hit ledger (the site's counter
// is above zero); `landed` is the page text of the site's witness. A payload site with a witness needs `landed`
// (a site that ran but did not land is not evidence of silence). A site with no witness (a glue call, a void
// payload) is judged by `ran`. A finding on a line with no payload site is judged by the glue calls (auto-sites)
// on its lines. A site that never ran is a `candidate-unexecuted` when the ledger proves it (unexecuted.mjs: payload sites only, each placed,
// the page quiescent, the host entered), else `inconclusive: never-ran`. Chrome and the conditions of deadsite.mjs decide whether a
// candidate is confirmed (unexecuted.mjs `judgeUnexecuted`); this comparator only says what the ledger shows.
//
// An uncertifiable finding is never a claim; at a runtime diagnostic it counts as covering it
// (the checker said it could not prove), the way the runtime oracle's `covered-uncertifiable` does.
import { codeOfThrown, codeRules, codesForRule, consequenceCodes, ignoredRules, isDefectCode, policyOf, sameDefectFamily, ruleHasRuntimeCounterpart, withholdingRules } from "./codes.mjs";
import { hitCount } from "./hits.mjs";
import { STATUS as UNEXECUTED_STATUS, unexecutedCandidate } from "./unexecuted.mjs";

export const VERDICTS = ["candidate-false-positive", "candidate-false-clean", "candidate-wrong-rule", "known-open", "recall-gap", "candidate-unexecuted", "inconclusive", "agree"];
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

/**
 * Codes of the runtime diagnostics no authored line can be given for: the oracle's `unattributed` list
 * (Chrome saw a diagnostic whose site is not in the case file) and any diagnostic that carries no
 * App.tsx position. Solid spoke and the comparator cannot tell where, so silence at a finding is not
 * evidence: it may be the very diagnostic that confirms the violation.
 */
export function unattributedCodes(runtime) {
  const codes = [...(runtime.unattributed ?? []).map(String)];
  for (const diagnostic of runtime.diagnostics ?? []) if (!(diagnostic.positions ?? []).length) codes.push(String(diagnostic.code));
  return [...new Set(codes)];
}

/** What disturbs the whole run: a throw (any, anywhere), reactivity halted, a diagnostic that could not be placed, a harness failure. */
export function disturbances(runtime, plan) {
  const reasons = [];
  if (runtime.failure) reasons.push(`runner failure in ${runtime.failure.phase}: ${String(runtime.failure.message).slice(0, 120)}`);
  const thrown = (runtime.errors ?? []).filter((error) => !["plan"].includes(error.kind));
  for (const error of thrown.slice(0, 3)) reasons.push(`the program threw (${error.kind}): ${String(error.message).slice(0, 100)}`);
  if ((runtime.diagnostics ?? []).some((diagnostic) => diagnostic.code === "REACTIVITY_HALTED")) reasons.push("reactivity halted");
  const unplaced = unattributedCodes(runtime).filter((code) => code !== "REACTIVITY_HALTED");
  if (unplaced.length) reasons.push(`a runtime diagnostic could not be placed on a line (${unplaced.slice(0, 3).join(", ")})`);
  for (const error of (runtime.errors ?? []).filter((e) => e.kind === "plan")) reasons.push(`driving plan: ${error.message}`);
  return reasons;
}

/**
 * @param {object} input
 * @param {{id:string,line:number,kind:string,witness?:object}[]} input.sites
 * @param {{ok:boolean,error?:string,findings:{rule:string,id?:string,kind:string,line:number,endLine:number}[]}} input.checker
 * @param {{failure?:object,diagnostics:object[],errors:object[],witnesses:{siteId:string,reached:boolean|null}[]}} input.runtime
 * @param {{clicks:string[]}} [input.plan]
 * @param {{id:string,hit:number,line:number}[]} [input.glue] auto-sites: one per emitted call (print.mjs)
 * @param {{ledger?: object[], lexicallyDead?: Set<number>}} [input.evidence] what the unexecuted candidate reads
 *   Also read from `runtime`: `hits` (`{ id: count }`, null when the run was not instrumented) and `quiescent`.
 */
export function classify({ sites, glue = [], checker, runtime, plan, evidence = {} }) {
  const items = [];
  const add = (kind, reason, extra = {}) => items.push({ kind, reason, ...extra });

  if (!checker.ok) return result([{ kind: "inconclusive", reason: `checker failed: ${String(checker.error).slice(0, 160)}`, why: "checker-failed" }]);
  if (checker.perturbed?.length) return result([{ kind: "inconclusive", reason: `instrumentation-perturbed-checker: ${checker.perturbed[0]}`, why: "perturbed-checker" }]);
  if (runtime.failure && !(runtime.witnesses ?? []).length) {
    return result([{ kind: "inconclusive", reason: `runtime failed in ${runtime.failure.phase}: ${String(runtime.failure.message).slice(0, 160)}` }]);
  }

  const events = runtimeEvents(runtime);
  const disturbed = disturbances(runtime, plan);
  const reached = new Map((runtime.witnesses ?? []).map((w) => [w.siteId, w.reached]));
  const findings = checker.findings.filter((finding) => !ignoredRules.includes(finding.rule));

  // A checker violation against the runtime.
  const hits = runtime.hits ?? null;
  for (const finding of findings.filter((f) => f.kind === "violation")) {
    const label = { rule: finding.rule, id: finding.id ?? null, line: finding.line };
    const payload = sites.filter((site) => inSpan(finding, site.line));
    // A finding on a plumbing line (`m1()()`) is judged by the glue calls on its lines.
    const covered = payload.length ? payload : glue.filter((site) => inSpan(finding, site.line));
    if (!ruleHasRuntimeCounterpart(finding.rule)) { add("inconclusive", `no runtime diagnostic claims rule ${finding.rule}`, { finding: label, why: "no-counterpart" }); continue; }
    if (!covered.length) { add("inconclusive", "the finding spans no site of interest", { finding: label, why: "no-site" }); continue; }
    const codes = codesForRule(finding.rule);
    // A consequence code (the engine halting after a throw) is not a diagnostic of this site.
    const here = events.filter((event) => !consequenceCodes.includes(event.code) && lines(event.positions).some((line) => inSpan(finding, line)));
    if (here.some((event) => codes.includes(event.code))) { add("agree", `runtime ${here.find((e) => codes.includes(e.code)).code} at the finding`, { finding: label, siteIds: covered.map((s) => s.id) }); continue; }
    const elsewhere = here.filter((event) => (codeRules[event.code] ?? []).some((rule) => sameDefectFamily(rule, finding.rule)));
    if (elsewhere.length) {
      const [event] = elsewhere;
      add("candidate-wrong-rule", `runtime ${event.code} at the finding: the same kind of defect, claimed by ${codeRules[event.code].join(" or ")}, not ${finding.rule}`,
        { finding: label, diagnostic: { code: event.code, line: lines(event.positions).find((line) => inSpan(finding, line)) }, runtimeRules: codeRules[event.code] });
      continue;
    }
    if (here.length) { add("inconclusive", `runtime reports ${[...new Set(here.map((e) => e.code))].join(", ")} at the finding, not a code that claims ${finding.rule}`, { finding: label, why: "other-code" }); continue; }
    if (disturbed.length) { add("inconclusive", `the run was disturbed: ${disturbed[0]}`, { finding: label, why: "disturbed" }); continue; }
    const states = covered.map((site) => ({ site, landed: reached.get(site.id) ?? null, count: hitCount(hits, site.hit) }));
    const ran = (state) => state.count !== null && state.count > 0;
    // The counter and the page must agree: a witness shown by a site the ledger says never ran is a broken readout.
    if (states.some((state) => state.landed === true && state.count === 0)) {
      add("inconclusive", "the witness is shown but the counter says the site never ran: the hit readout is not trusted", { finding: label, why: "readout-mismatch" });
      continue;
    }
    // Proof the site ran and has nothing further to show, or ran and landed.
    const proven = states.filter((state) => state.landed === true || (ran(state) && !state.site.witness));
    if (proven.length) {
      add("candidate-false-positive", `${finding.rule} (${finding.id ?? "?"}) is a violation at line ${finding.line}; the runtime ran the site (${proven.some((s) => s.landed === true) ? "witness shown" : "its counter ran"}) with no diagnostic and no throw`,
        { finding: label, siteIds: proven.map((state) => state.site.id) });
    } else if (states.some((state) => ran(state))) {
      add("inconclusive", "the site ran but its witness did not land: nothing proves the claimed code executed", { finding: label, why: "ran-not-landed" });
    } else if (hits !== null && states.every((state) => state.count === 0)) {
      // Payload sites only: a finding on a plumbing line (glue calls) is never a candidate.
      const unexecuted = payload.length ? unexecutedCandidate({ sites: covered, ledger: evidence.ledger, hits, quiescent: runtime.quiescent ?? null, lexicallyDead: evidence.lexicallyDead }) : null;
      if (unexecuted?.ok) {
        add("candidate-unexecuted", `${finding.rule} (${finding.id ?? "?"}) is a violation at line ${finding.line}; the hit ledger shows no execution entered the site (${[...new Set(unexecuted.bases)].join(", ")})`,
          { finding: label, status: UNEXECUTED_STATUS, siteIds: covered.map((site) => site.id), bases: unexecuted.bases });
      } else {
        add("inconclusive", "the site never ran", { finding: label, why: "never-ran" });
      }
    } else if (states.every((state) => state.landed === null)) {
      add("inconclusive", "the runtime is silent, but nothing shows the site ran (no witness)", { finding: label, why: "no-witness" });
    } else {
      add("inconclusive", "the runtime is silent, but the site's witness is absent: the site did not run, or did not land", { finding: label, why: "witness-absent" });
    }
  }

  items.push(...diagnosticItems(events, findings, evidence.registrationLines));

  if (!items.length) add("agree", disturbed.length ? `no claims to compare (${disturbed[0]})` : "the checker and the runtime are both silent");
  return result(items);

  function result(rows) {
    const verdict = rows.reduce((worst, row) => (rank(row.kind) < rank(worst) ? row.kind : worst), "agree");
    const reasons = [...new Set(rows.filter((row) => row.kind === verdict).map((row) => row.reason))];
    return { verdict, reasons, items: rows };
  }
}


/**
 * The runtime diagnostics against the checker's findings (the verdict tiers of the owner's policy).
 * A diagnostic of a defect code (the policy table) at a line:
 *   a checker violation there         agree (already an item from the finding above);
 *   an uncertifiable finding there    `recall-gap`: the checker declined; counted, never a failure;
 *   nothing, or no finding that could claim it    `candidate-false-clean`.
 * A diagnostic with no authored line (Solid spoke, the harness could not place it) is a false clean only when the checker has
 * no finding of a claiming rule anywhere in the file; with one it is left alone (nothing says which line it was).
 * `registrationLines(line)` (registration.mjs) gives, for a stack line, the lines of the calls that registered the callback the line
 * is in: a callback that runs later has no registering call on its stack, and the checker answers there (`onSettled(cb1)`). A finding
 * of a claiming rule on such a line covers the diagnostic like one on a stack line.
 */
export function diagnosticItems(events, findings, registrationLines = () => []) {
  const items = [];
  for (const event of events) {
    if (!isDefectCode(event.code)) continue;
    const rules = codeRules[event.code] ?? [];
    const severity = policyOf(event.code).severity;
    const attributed = event.positions.filter((position) => position.file === "App.tsx" || position.file === undefined);
    const claims = (finding) => rules.includes(finding.rule) || withholdingRules.includes(finding.rule);
    if (!attributed.length) {
      if (event.positions.length || findings.some(claims)) continue;
      items.push({ kind: "candidate-false-clean", reason: `${event.code} (no line could be given) has no finding of a rule that claims it`,
        diagnostic: { code: event.code, line: null, message: event.message, severity, source: event.source, unplaced: true, frames: [] } });
      continue;
    }
    const registered = attributed.flatMap((position) => registrationLines(position.line));
    const onStack = (finding) => attributed.some((position) => inSpan(finding, position.line));
    const covering = findings.filter((finding) => (onStack(finding) || registered.some((line) => inSpan(finding, line))) && claims(finding));
    if (covering.some((finding) => finding.kind === "violation")) {
      // A violation on a stack line is already an agree through the finding; one on a registration line is not met by that pass.
      const viaRegistration = covering.find((finding) => finding.kind === "violation" && !onStack(finding));
      if (viaRegistration && !covering.some((finding) => finding.kind === "violation" && onStack(finding))) {
        items.push({ kind: "agree", reason: `${event.code} at line ${attributed[0].line}: the checker's violation ${viaRegistration.rule} (${viaRegistration.id ?? "?"}) is at line ${viaRegistration.line}, where the callback is registered`, diagnostic: { code: event.code, line: attributed[0].line } });
      }
      continue;
    }
    // A violation of the same defect family at the diagnostic is a wrong-rule item from the finding, not a missing finding.
    if (!covering.length && findings.some((finding) => finding.kind === "violation" && (onStack(finding) || registered.some((line) => inSpan(finding, line))) && rules.some((rule) => sameDefectFamily(rule, finding.rule)))) continue;
    // `frames`: the App lines of the diagnostic's stack, innermost first (the dynamic stack shape of the coarse key reads them).
    const diagnostic = { code: event.code, line: attributed[0].line, message: event.message, severity, source: event.source, frames: attributed.map((position) => position.line) };
    if (covering.length) {
      items.push({ kind: "recall-gap", reason: `${event.code}: the checker declines (uncertifiable ${covering[0].rule}) at line ${diagnostic.line}`, diagnostic: { ...diagnostic, rule: covering[0].rule } });
      continue;
    }
    items.push({ kind: "candidate-false-clean", reason: `${event.code} at line ${diagnostic.line} has no finding of a rule that claims it`, diagnostic });
  }
  return items;
}
