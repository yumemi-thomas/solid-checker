// The twin of a repeat claim (ADR 0953, option c; the owner's ruling). A repeat (classify.mjs `offSiteVerdicts`, why `repeat`) is a violation at a read
// whose source the runtime already reported once, at another read of the same program: Solid reports some codes once per source, so its silence at the
// second read is its own dedup, not a contradiction. The judge scores such a claim as the agreement it is only where the claim's twin holds: the program
// with the first read (the site holding the report's frame) replaced by an inert statement, judged as a program of its own at the remaining read. The twin
// holds when the runtime reports the first code at the remaining read AND a violation of the claim's rule covers it there, which is what
// `judge` calls an `agree` (nothing else reported, no off-site problem). Anything else fails it: the claim stays a stacked claim, listed and counted by
// its pair as before, and an unbuildable twin is a failed one. A twin never makes an agreement out of a false clean (no violation covers the read),
// a false positive (the runtime reports nothing there) or a wrong rule: each is a different outcome of the twin's judge.
//
// Pure: `repeatTwins` builds the twin programs, the grid evaluates them (matrix.mjs `settleRepeats`), `twinHolds` reads each record through the judge.
import { buildCase } from "./emit.mjs";
import { clone, getAt, isValid, num } from "./model.mjs";
import { STACKED_PAIRS, isRepeat } from "./stacked.mjs";

/** The lines of a site: where its misuse is written and the lines of a wrapper's inner call (matrix.mjs `regionOf`). */
const regionOf = (site) => [site.line, ...(site.aux ?? [])].filter((line) => Number.isInteger(line));
const samePath = (one, other) => Array.isArray(one) && Array.isArray(other) && one.length === other.length && one.every((key, index) => key === other[index]);

/**
 * The twins of a program's repeat claims, one request each, in the order of `judged`.
 * @param {{ judged: object[], offSite: object[], built: object, program: object, pairs?: object }} input `judged`: the off-site verdict rows
 *   (offSiteVerdicts), `offSite`: the findings they judged, `built` and `program` the printed case and its tree
 * @returns {{ label: string, rule: string, first: string, program?: object, built?: object, site?: object, refused?: string }[]} a request has the twin
 *   (`program`, `built`) and the claim's site in it (`site`), or `refused`: why none can be built (a claim with no twin never holds)
 */
export function repeatTwins({ judged, offSite, built, program, pairs = STACKED_PAIRS }) {
  const requests = [];
  for (const [index, claim] of judged.filter((row) => isRepeat(row, pairs)).entries()) {
    const [rule, line] = [claim.finding.split("@")[0], Number(claim.finding.split("@")[1])];
    const request = { label: claim.finding, rule, first: claim.first };
    const refuse = (why) => requests.push({ ...request, refused: why });
    if (!program || !built?.sites) { refuse("no program tree"); continue; }
    const findings = offSite.filter((finding) => finding.rule === rule && finding.line === line);
    // The site the claim stands on, and the site holding the first report's own read: one each, and not the same.
    const claimSites = built.sites.filter((site) => findings.some((finding) => finding.line <= site.line && site.line <= finding.endLine));
    const firstSites = built.sites.filter((site) => (claim.firstAt ?? []).some((at) => regionOf(site).includes(at.line)));
    if (claimSites.length !== 1) { refuse(`the claim stands on ${claimSites.length} sites`); continue; }
    if (firstSites.length !== 1) { refuse(`the first report's read is in ${firstSites.length} sites`); continue; }
    const [claimSite, firstSite] = [claimSites[0], firstSites[0]];
    if (claimSite === firstSite) { refuse("the first read and the claim are one site"); continue; }
    const node = Array.isArray(firstSite.path) && firstSite.path.length ? getAt(program, firstSite.path) : null;
    if (!node?.site) { refuse("the first read is no site node"); continue; }
    const twin = clone(program);
    getAt(twin, firstSite.path.slice(0, -1))[firstSite.path.at(-1)] = num(1);
    if (!isValid(twin)) { refuse("the twin is not a valid program"); continue; }
    const twinBuilt = buildCase(twin, { id: `${built.id}-twin${index + 1}`, note: `the twin of ${built.id}: the first read (${firstSite.id}) removed, the repeat claim ${claim.finding} standing alone (ADR 0953)` });
    const twinSite = twinBuilt.sites.find((site) => samePath(site.path, claimSite.path));
    if (!twinSite) { refuse("the claim's site is not in the twin"); continue; }
    requests.push({ ...request, program: twin, built: twinBuilt, site: twinSite });
  }
  return requests;
}

/**
 * Whether a twin holds, given its evaluation (`value`: the pipeline's record of the twin) and the grid's `judge`: the twin judged at the claim's remaining
 * read for the first code is an `agree`, and the violation that covers it is the claim's own rule's. `why` says what failed, for the row's detail.
 * @returns {{ holds: boolean, why?: string }}
 */
export function twinHolds(request, value, judge) {
  if (request.refused) return { holds: false, why: `no twin: ${request.refused}` };
  if (!value || value.error) return { holds: false, why: "the twin did not run" };
  const verdict = judge({ code: request.first, site: request.site, typecheck: value.typecheck, checker: value.checker, runtime: value.runtime, others: [], built: request.built, program: request.program });
  if (verdict.outcome !== "agree") return { holds: false, why: `twin ${verdict.outcome}` };
  const own = (verdict.detail?.findings ?? []).some((entry) => String(entry).startsWith(`${request.rule}/violation@`)) || String(verdict.detail?.remote ?? "").startsWith(`${request.rule}@`);
  return own ? { holds: true } : { holds: false, why: "twin agrees through another rule" };
}
