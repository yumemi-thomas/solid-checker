// The runtime differential: Happy DOM (the fast path) against Chrome (the authority) on the same generated programs, record against
// record, without the checker. Every grid verdict is the fast path's; a program the two runtimes disagree on is a fast-path gap (a
// diagnostic Happy DOM misses, or one it invents) that can hide a false clean, a false positive or a wrong agreement in every grid
// number. See scripts/fuzz/README.md, "The runtime differential".
//
// The grids call it with `--runtime-diff <N|all>`: up to N programs per stratum, or all. A stratum is a value of each of
// `--runtime-diff-axes` (default code, context, source, mount and the row's outcome; every grid program is its own value of the full
// set, so a sample over it would be the whole grid); within one, the programs are taken in the order of a hash of their id, so the
// bindings, layers and variants the stratum leaves open spread over the sample, the same for the same grid.

import { runtimeEvents } from "./classify.mjs";
import { fnv } from "./rng.mjs";
import { consequenceCodes } from "./codes.mjs";

/** Every axis a grid row may carry, and the row's outcome. */
export const STRATUM_AXES = ["code", "context", "variant", "binding", "layer", "mount", "source", "outcome"];
/** The differential's default stratum: the axes whose values change what the runtime does most (see the file comment). */
export const DEFAULT_DIFF_AXES = ["code", "context", "source", "mount", "outcome"];

/** The stratum of a grid row: its value on each of `axes` (default every axis). */
export const stratumOf = (row, axes = STRATUM_AXES) => axes.map((axis) => `${axis}=${row[axis] ?? "-"}`).join("|");

/** `--runtime-diff-axes a,b`: axis names from STRATUM_AXES (default DEFAULT_DIFF_AXES). Throws on an unknown one. */
export function parseAxes(value) {
  if (value === undefined || value === null) return DEFAULT_DIFF_AXES;
  const axes = String(value).split(",").map((axis) => axis.trim()).filter(Boolean);
  const unknown = axes.filter((axis) => !STRATUM_AXES.includes(axis));
  if (unknown.length || !axes.length) throw new Error(`--runtime-diff-axes takes names from ${STRATUM_AXES.join(",")}, not ${JSON.stringify(value)}`);
  return axes;
}

/**
 * The rows the differential runs: up to `perStratum` per stratum of `axes` (`Infinity` for all), in the order of a hash of their id
 * (the same sample for the same grid). Rows whose evaluation failed (no fast record) are left out.
 */
export function sampleStrata(rows, perStratum, hasRecord = () => true, axes = DEFAULT_DIFF_AXES) {
  const taken = new Map();
  const picked = [];
  const ordered = [...rows].sort((a, b) => fnv(String(a.id)) - fnv(String(b.id)) || (a.id < b.id ? -1 : 1));
  for (const row of ordered) {
    if (!hasRecord(row)) continue;
    const key = stratumOf(row, axes);
    const n = taken.get(key) ?? 0;
    if (n >= perStratum) continue;
    taken.set(key, n + 1);
    picked.push(row);
  }
  return picked;
}

/**
 * A record's defect events as `code@line` keys: diagnostics and thrown codes placed in App.tsx by their first App.tsx position, a
 * consequence code (a compiled-module position) or one placed elsewhere (`helpers.ts`, the runtime) as `code@-`, and every code the
 * record saw but could not place (`unattributed`) as `code@-` too: where an event is attributed is the host's, what happened is not.
 */
export function eventKeys(runtime) {
  const placed = runtimeEvents(runtime ?? {}).map((event) => {
    const line = consequenceCodes.includes(event.code) ? "-" : event.positions.find((p) => p.file === "App.tsx" || p.file === undefined)?.line ?? "-";
    return `${event.code}@${line}`;
  });
  return [...placed, ...(runtime?.unattributed ?? []).map((code) => `${code}@-`)];
}

/**
 * A record's defect events as a set: placed `code@line` keys, and positionless codes kept only where the same record places no event
 * of that code (Chrome reports a throw once as a placed diagnostic and again as page errors with no position; Happy DOM places both).
 */
function eventSet(runtime) {
  const keys = eventKeys(runtime);
  const placedCodes = new Set(keys.filter((key) => !key.endsWith("@-")).map((key) => key.slice(0, key.lastIndexOf("@"))));
  return new Set(keys.filter((key) => !key.endsWith("@-") || !placedCodes.has(key.slice(0, -2))));
}
const codeOf = (key) => key.slice(0, key.lastIndexOf("@"));

/** The sites that ran at least once (the hit ledger), sorted. */
const ran = (runtime) => Object.entries(runtime?.hits ?? {}).filter(([, n]) => n > 0).map(([id]) => id).sort();

/**
 * The differences between a program's Happy DOM record and its Chrome record, each `{ kind, detail }`:
 *   chrome-only   a defect event Chrome reports and Happy DOM does not (a Happy DOM miss: a possible hidden false clean)
 *   happy-only    a defect event Happy DOM reports and Chrome does not (a Happy DOM invention: a possible wrong agreement)
 *   moved         a code both report, on line sets that differ (attribution: which frame each side placed an event at)
 *   hits          the sets of sites that ran differ
 *   witness       a site's witness readout differs
 *   failure       one side failed, halted or errored where the other did not, or in another phase
 * A record either side lacks is `missing`. Message texts and throw kinds are host-specific and not compared (the codes are).
 */
export function runtimeDiff(happy, chrome) {
  if (!happy || !chrome) return [{ kind: "missing", detail: !happy ? "no Happy DOM record" : "no Chrome record" }];
  const diffs = [];
  const a = eventSet(happy);
  const b = eventSet(chrome);
  // A positionless event matches any event of its code on the other side; a placed one, the same code on the same line.
  const matched = (key, other) => other.has(key) || (key.endsWith("@-") && [...other].some((k) => codeOf(k) === codeOf(key)))
    || [...other].some((k) => k.endsWith("@-") && codeOf(k) === codeOf(key));
  const onlyA = [...a].filter((key) => !matched(key, b));
  const onlyB = [...b].filter((key) => !matched(key, a));
  // A code both sides report, on lines that differ, is one `moved` difference: where the host attributed an event of it (a stack
  // frame the other side lost to its depth limit, or chose differently), not whether it happened. Only a code one side never reports
  // is a difference in behaviour (`happy-only`, `chrome-only`).
  const codesA = new Set([...a].map(codeOf));
  const codesB = new Set([...b].map(codeOf));
  const lines = (set, code) => [...set].filter((key) => codeOf(key) === code).map((key) => key.slice(key.lastIndexOf("@") + 1)).sort().join(",");
  for (const code of new Set([...onlyA, ...onlyB].map(codeOf).filter((code) => codesA.has(code) && codesB.has(code)))) {
    diffs.push({ kind: "moved", detail: `${code} happy@${lines(a, code)} chrome@${lines(b, code)}` });
  }
  for (const key of onlyA) if (!codesB.has(codeOf(key))) diffs.push({ kind: "happy-only", detail: key });
  for (const key of onlyB) if (!codesA.has(codeOf(key))) diffs.push({ kind: "chrome-only", detail: key });
  // A ledger that one side could not read (null) is no evidence either way; the failure row speaks for it.
  if (happy.hits && chrome.hits) {
    const ra = ran(happy);
    const rb = ran(chrome);
    if (ra.join(",") !== rb.join(",")) diffs.push({ kind: "hits", detail: `happy-only [${ra.filter((id) => !rb.includes(id)).join(",")}] chrome-only [${rb.filter((id) => !ra.includes(id)).join(",")}]` });
  }
  const witness = (runtime) => new Map((runtime.witnesses ?? []).filter((w) => w.witness).map((w) => [w.siteId, w.actual ?? null]));
  const wa = witness(happy);
  const wb = witness(chrome);
  for (const id of new Set([...wa.keys(), ...wb.keys()])) {
    if ((wa.get(id) ?? null) !== (wb.get(id) ?? null)) diffs.push({ kind: "witness", detail: `${id}: happy ${JSON.stringify(wa.get(id) ?? null)} chrome ${JSON.stringify(wb.get(id) ?? null)}` });
  }
  const fa = happy.failure?.phase ?? null;
  const fb = chrome.failure?.phase ?? null;
  if (fa !== fb) diffs.push({ kind: "failure", detail: `happy ${fa ?? "none"} chrome ${fb ?? "none"}` });
  return diffs;
}

/** What a differing program's record says, for triage without a rerun: its events, the sites that ran, its failure. */
export const evidenceOf = (runtime) => ({
  events: [...eventSet(runtime)].sort(),
  ran: ran(runtime),
  failure: runtime?.failure ? `${runtime.failure.phase}: ${String(runtime.failure.message ?? "").slice(0, 160)}` : null,
});

/** A Chrome record the differential cannot read: the harness, not the program (retried once, then reported as `chrome-inconclusive`). */
export const chromeUnreadable = (record) => !record?.runtime || ["harness-error"].includes(record.runtime.failure?.phase);

/** Differences in behaviour: every kind but `moved` (attribution only). */
export const BEHAVIOUR_KINDS = ["happy-only", "chrome-only", "hits", "witness", "failure", "missing"];

/** The summary of a differential: programs compared, identical, differing in behaviour, differing in attribution only, and per kind. */
export function summarize(results) {
  const byKind = {};
  for (const result of results) for (const kind of new Set(result.diffs.map((d) => d.kind))) byKind[kind] = (byKind[kind] ?? 0) + 1;
  const behaviour = (result) => result.diffs.some((d) => BEHAVIOUR_KINDS.includes(d.kind));
  return {
    compared: results.length,
    same: results.filter((result) => result.diffs.length === 0).length,
    differ: results.filter((result) => result.diffs.length > 0).length,
    behaviour: results.filter(behaviour).length,
    attributionOnly: results.filter((result) => result.diffs.length > 0 && !behaviour(result)).length,
    byKind,
  };
}

/** `--runtime-diff` as a count per stratum: a positive integer, or `all` (every program). Throws on anything else. */
export function parsePerStratum(value) {
  if (value === undefined || value === null || value === false) return 0;
  if (value === "all") return Infinity;
  const n = Number(value);
  if (!Number.isInteger(n) || n < 1) throw new Error(`--runtime-diff takes a positive integer or "all", not ${JSON.stringify(value)}`);
  return n;
}
