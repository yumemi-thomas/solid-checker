// The conditions under which a violation of an execution-claim rule (one a runtime diagnostic can contradict) at a site that the
// whole driven run never entered is a false positive. They are the confirmation of an unexecuted candidate (unexecuted.mjs
// `judgeUnexecuted`, run by `--confirm-with-chrome` on Chrome's records); `--dead-site` asks for exactly these and nothing looser.
// Each is an explicit condition below, and an unmet one leaves the candidate `chrome-inconclusive`, typed with the condition:
//
//   closed-grammar   the program is closed and branch-free (model.mjs `closedGrammar`), so the executed
//                    set is fixed by the external entries.
//   quiescent        the runner proved the page was still (the counters unchanged over two ticks), rather
//                    than trusting a pause.
//   neutral          the instrumented twin is neutral: strip(runText) == text, and the clean-twin control
//                    matched this run.
//   attributed       no disturbance (throw, halt, runner failure) and no diagnostic that could not be placed.
//   host-ran         some enclosing function (ending at the component) was entered: a host that never ran
//                    is a harness fault, not a claim.
//   not-a-throw      the nearest enclosing function was NOT entered. If it was entered and the site was
//                    skipped, a throw before it is the explanation (a disturbance), not a dead closure.
//   not-a-default    the site is not inside a parameter default (run only when the caller passes no argument).
//
// The lemma: under closed-grammar and quiescent, `entered(fn)` after the driver is true iff some execution
// enters `fn`. A nearest enclosing function never entered while an enclosing function that built it ran is
// "closure built, never invoked" (the ADR 0452 family): the checker proved a role for code no execution runs.
// Two engines must agree: the fast path proposes the candidate and Chrome must itself satisfy every condition (hits == 0, no
// diagnostic of the rule's codes) before the finding counts as confirmed.
//
// Two narrowings of what a condition needs, both for a site the text itself never runs (`lexicallyDead`: below `if (false)`, after `return`,
// in the right of `false &&`) and both off in the strict mode: `not-a-throw` does not apply (no throw is needed to explain a skip the
// text makes), and `closed-grammar` is waived when every site is one (the lemma is about what the driver reaches; this code is not
// reachable by any driver). Everything else holds unchanged.
import { hitCount } from "./hits.mjs";

export const CONDITIONS = ["closed-grammar", "quiescent", "neutral", "attributed", "host-ran", "not-a-throw", "not-a-default"];

/**
 * @param {object} input
 * @param {object[]} input.sites the sites (payload or glue) a finding covers, each with a `hit`
 * @param {object[]} input.ledger rows from printProgram
 * @param {object|null} input.hits `{ id: count }` of the run
 * @param {{closed: boolean}} [input.closed] closedGrammar(program)
 * @param {boolean|null} [input.quiescent]
 * @param {boolean} [input.neutral]
 * @param {string[]} [input.disturbed] classify's disturbances (unattributed diagnostics included)
 * @param {Set<number>} [input.lexicallyDead] hit ids below `if (false)`, after `return` or in `false && ...` (unexecuted.mjs): the text
 *   skips them, so `not-a-throw` (a throw before the site as the explanation of a skip) does not apply, and when every site is one the
 *   closed grammar is waived (the lemma about the driver is not needed for code the text never runs). Empty for the strict mode.
 * @returns {{ ok: boolean, unmet: { condition: string, detail: string }[], dead: { site: string, never: number }[] }}
 */
export function deadSiteConditions({ sites, ledger, hits, closed, quiescent, neutral, disturbed = [], lexicallyDead = new Set() }) {
  const unmet = [];
  const need = (condition, detail) => { if (!unmet.some((u) => u.condition === condition)) unmet.push({ condition, detail }); };
  // A site the text itself skips (`lexicallyDead`) needs no lemma about the driver: its non-execution is syntactic. Every site must be so for the
  // closed grammar to be waived; one site that is not brings the condition back for the whole finding.
  const syntactic = sites.length > 0 && sites.every((site) => lexicallyDead.has(site.hit));
  if (!closed?.closed && !syntactic) need("closed-grammar", (closed?.reasons ?? ["not evaluated"]).join("; "));
  if (quiescent !== true) need("quiescent", quiescent === false ? "the hit counters moved late" : "no quiescence reading");
  if (neutral !== true) need("neutral", "the instrumented twin is not shown neutral");
  if (disturbed.length) need("attributed", disturbed[0]);
  const byHit = new Map((ledger ?? []).map((row) => [row.hit, row]));
  const dead = [];
  for (const site of sites) {
    const row = byHit.get(site.hit);
    if (!row || hits === null || hits === undefined) { need("host-ran", "no ledger row or no hit readout"); continue; }
    if (hitCount(hits, site.hit) !== 0) { need("not-a-throw", `${site.id} ran`); continue; }
    if (row.inDefault) { need("not-a-default", `${site.id} is in a parameter default`); continue; }
    // The enclosing functions, nearest first.
    const chain = [];
    for (let at = row.enclosing; at !== null && at !== undefined; at = byHit.get(at)?.enclosing) {
      const entry = byHit.get(at);
      if (!entry) break;
      chain.push(entry);
    }
    if (chain.some((entry) => entry.inDefault)) { need("not-a-default", `${site.id} sits in a function built in a parameter default`); continue; }
    const nearest = chain[0];
    if (!nearest) { need("host-ran", `${site.id} has no enclosing function`); continue; }
    if (!chain.some((entry) => hitCount(hits, entry.hit) > 0)) { need("host-ran", `no function enclosing ${site.id} was entered`); continue; }
    if (hitCount(hits, nearest.hit) > 0 && lexicallyDead.has(site.hit)) { dead.push({ site: site.id, never: site.hit }); continue; }
    if (hitCount(hits, nearest.hit) > 0) { need("not-a-throw", `${site.id} was skipped inside an entered function`); continue; }
    dead.push({ site: site.id, never: nearest.hit });
  }
  return { ok: unmet.length === 0 && dead.length === sites.length && sites.length > 0, unmet, dead };
}
