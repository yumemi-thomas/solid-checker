// A checker violation at a site that no execution entered ("unexecuted"). ADR 0458: such a violation is uncertifiable, so the checker
// reporting one is a false positive. The fuzzer used to file it as `inconclusive: never-ran` and nobody saw it. Two tiers:
//
//   candidate-false-positive: unexecuted   (the fast path; classify.mjs, item kind `candidate-unexecuted`)
//       The ledger shows every payload site of the finding at counter zero, each placed in the ledger, the page quiescent, and a
//       function enclosing the site entered (the host ran). A finding on a plumbing line, a site that ran, a site the ledger
//       cannot place, a disturbed run: stays `inconclusive`. A candidate is a hypothesis; it never stops a sweep by itself.
//   confirmed-unexecuted                   (run.mjs --confirm-with-chrome; `judgeUnexecuted`)
//       Chrome shows the same: the site's counter is zero after quiescence, no diagnostic of the rule's codes anywhere, no
//       disturbance, and every condition of deadsite.mjs holds: closed grammar (with containers, see model.mjs closedGrammar),
//       quiescent, neutral, attributed, host-ran, not-a-throw, not-a-default. That is a real checker false positive (exit 3).
//
// The one thing added to the deadsite.mjs conditions is `lexicallyDead`: a site below `if (false)`, after `return` or in the right of
// `false &&` is skipped by the text, so "skipped inside an entered function" (the throw-before-it explanation of not-a-throw) cannot
// apply to it, and the closed grammar (a lemma about what the driver reaches) is not needed for it. Every other site still needs the
// closed grammar, with the containers of container.mjs admitted (model.mjs closedGrammar); nothing else is relaxed.
import { codesForRule } from "./codes.mjs";
import { deadSiteConditions } from "./deadsite.mjs";
import { hitCount } from "./hits.mjs";
import { closedGrammar, walk } from "./model.mjs";

export const STATUS = "candidate-false-positive: unexecuted";

/** Containers whose body the text itself never reaches. */
export const LEXICALLY_DEAD = new Set(["dead-if", "dead-after-return", "and-false"]);

const under = (path, prefix) => prefix.length <= path.length && prefix.every((part, at) => String(part) === String(path[at]));

/** The hit ids of the ledger rows that sit below a lexically dead container. */
export function lexicallyDeadHits(program, ledger) {
  const dead = [...walk(program)].filter(({ node }) => node.k === "contain" && LEXICALLY_DEAD.has(node.kind)).map(({ path }) => path);
  if (!dead.length) return new Set();
  return new Set((ledger ?? []).filter((row) => dead.some((prefix) => under(row.path, prefix))).map((row) => row.hit));
}

/** The enclosing function entries of a ledger row, nearest first. */
function enclosingChain(row, byHit) {
  const chain = [];
  for (let at = row.enclosing; at !== null && at !== undefined; at = byHit.get(at)?.enclosing) {
    const entry = byHit.get(at);
    if (!entry) break;
    chain.push(entry);
  }
  return chain;
}

/**
 * What the ledger says about why one payload site did not run, or why it cannot say.
 * @returns {{ placed: false, why: string } | { placed: true, hostRan: boolean, basis: string }}
 *   basis: `dead-code` (below a lexically dead container), `container-not-entered`, `function-not-entered`, `in-default`
 *   (the site sits in a parameter default), `skipped-in-entered-function` (its nearest function ran and the site did not).
 */
export function siteBasis({ site, ledger, hits, lexicallyDead = new Set() }) {
  const byHit = new Map((ledger ?? []).map((row) => [row.hit, row]));
  const row = byHit.get(site.hit);
  if (!row) return { placed: false, why: "no ledger row" };
  if (hitCount(hits, site.hit) === null) return { placed: false, why: "no hit readout" };
  const chain = enclosingChain(row, byHit);
  const hostRan = chain.some((entry) => hitCount(hits, entry.hit) > 0);
  if (lexicallyDead.has(site.hit)) return { placed: true, hostRan, basis: "dead-code" };
  if (row.inDefault || chain.some((entry) => entry.inDefault)) return { placed: true, hostRan, basis: "in-default" };
  const nearest = chain[0];
  if (nearest && hitCount(hits, nearest.hit) === 0) return { placed: true, hostRan, basis: nearest.node === "contain" ? "container-not-entered" : "function-not-entered" };
  return { placed: true, hostRan, basis: "skipped-in-entered-function" };
}

/**
 * Whether a finding's payload sites (all at counter zero) make a fast-path candidate.
 * @param {object[]} input.sites the payload sites the finding covers (never plumbing glue: the caller passes payload only)
 * @returns {{ ok: boolean, why?: string, bases?: string[] }}
 */
export function unexecutedCandidate({ sites, ledger, hits, quiescent, lexicallyDead }) {
  if (!sites.length) return { ok: false, why: "no payload site" };
  if (hits === null || hits === undefined) return { ok: false, why: "no hit readout" };
  if (quiescent !== true) return { ok: false, why: quiescent === false ? "the hit counters moved late" : "no quiescence reading" };
  const bases = [];
  for (const site of sites) {
    if (hitCount(hits, site.hit) !== 0) return { ok: false, why: `${site.id} ran` };
    const basis = siteBasis({ site, ledger, hits, lexicallyDead });
    if (!basis.placed) return { ok: false, why: `${site.id}: ${basis.why}` };
    if (!basis.hostRan) return { ok: false, why: `no function enclosing ${site.id} was entered` };
    bases.push(basis.basis);
  }
  return { ok: true, bases };
}

/** Chrome's answer for one unexecuted candidate. `status` is `confirmed-unexecuted`, `refuted-by-chrome` or `chrome-inconclusive`; `why` is typed. */
const verdict = (status, why, extra = {}) => ({ status, why, ...extra });

/**
 * Judges a fast-path unexecuted candidate with Chrome's records.
 * @param {object} input
 * @param {object} input.item the fast path's `candidate-unexecuted` item
 * @param {object} input.program the model program (closed grammar, lexically dead containers)
 * @param {{ sites: object[], ledger: object[], neutral?: boolean }} input.built
 * @param {{ runtime: object, checker: object }} input.record Chrome's runtime and checker records (chrome.mjs)
 * @param {object} input.judged classify's answer on Chrome's records (pipeline.mjs classifyChrome)
 * @param {object[]} input.events Chrome's runtime events (classify.mjs runtimeEvents)
 * @param {boolean|null} input.controlMatched the clean-twin control of the fast pipeline matched this program
 * @param {boolean} [input.strict] `--dead-site`: the strict closed grammar of model.mjs (no containers) and no dead-code waiver, exactly the
 *   conditions of deadsite.mjs as they were before containers existed
 */
export function judgeUnexecuted({ item, program, built, record, judged, events, controlMatched, strict = false }) {
  if (!record?.checker?.ok) return verdict("chrome-inconclusive", "chrome-failed", { detail: String(record?.checker?.error ?? "no oracle row").slice(0, 120) });
  if (record.checker.perturbed?.length) return verdict("chrome-inconclusive", "instrumentation-perturbed-checker", { detail: record.checker.perturbed[0] });
  const same = (other) => other.finding?.rule === item.finding.rule && other.finding?.line === item.finding.line;
  const chrome = (judged?.items ?? []).find(same);
  if (!chrome) return verdict("chrome-inconclusive", "chrome-checker-differs", { detail: "Chrome's checker run has no violation of this rule at this line" });
  if (chrome.kind === "agree") return verdict("refuted-by-chrome", "chrome-diagnostic", { detail: chrome.reason });
  if (chrome.kind === "candidate-false-positive") return verdict("refuted-by-chrome", "chrome-ran", { detail: chrome.reason });
  if (chrome.kind !== "candidate-unexecuted") return verdict("chrome-inconclusive", chrome.why ?? chrome.kind, { detail: chrome.reason });
  // No diagnostic of the rule's codes anywhere: the finding is a claim about the whole program, not only about its line.
  const codes = codesForRule(item.finding.rule);
  const reported = events.filter((event) => codes.includes(event.code));
  if (reported.length) return verdict("chrome-inconclusive", "rule-code-reported", { detail: reported[0].code });
  const covered = built.sites.filter((site) => (chrome.siteIds ?? []).includes(site.id));
  const conditions = deadSiteConditions({
    sites: covered, ledger: built.ledger, hits: record.runtime.hits ?? null,
    closed: closedGrammar(program, { containers: !strict }), quiescent: record.runtime.quiescent ?? null,
    neutral: Boolean(built.neutral) && controlMatched === true,
    disturbed: [], lexicallyDead: strict ? new Set() : lexicallyDeadHits(program, built.ledger),
  });
  if (!conditions.ok) return verdict("chrome-inconclusive", "conditions-unmet", { unmet: conditions.unmet.map((u) => u.condition), detail: conditions.unmet[0]?.detail });
  return verdict("confirmed-unexecuted", "confirmed", { bases: chrome.bases });
}

/**
 * The run's unexecuted rows as a report: counts by status and by basis, and the groups by construct signature (`dedupe`, smallest first).
 * @param {object[]} rows `{ id, signature, status, why, bases, ... }` per candidate item
 */
export function summarizeUnexecuted(rows, dedupe) {
  const count = (values) => values.reduce((counts, key) => ({ ...counts, [key]: (counts[key] ?? 0) + 1 }), {});
  return {
    candidates: rows.length, programs: new Set(rows.map((row) => row.id)).size,
    confirmed: rows.filter((row) => row.status === "confirmed-unexecuted").length,
    byStatus: count(rows.map((row) => (row.why && row.status === "chrome-inconclusive" ? `${row.status}:${row.why}` : row.status))),
    byBasis: count(rows.flatMap((row) => [...new Set(row.bases ?? [])])),
    groups: dedupe(rows.filter((row) => row.signature !== "-").map((row) => ({ ...row }))),
    items: rows.map(({ shrinkKey, ...row }) => row),
  };
}
