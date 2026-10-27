// The hit ledger: knowing, per executed place, that it ran and how often.
//
// `print.mjs` emits the program twice. `text` is clean (the checker reads it). `runText` is the
// instrumented twin: `(__h(N), expr)` on each payload site and each other emitted call (the glue),
// `{ __h(N);` at each function entry, `__h(N); ` before a statement that is itself a counted call, and `INSTRUMENT_PRELUDE` appended after the last line so no
// line moves. This module owns the counter's source, its readout, the strip that must give the
// clean text back, and the pure comparisons the neutrality gate is made of.
//
// The counter touches no Solid state: it increments `globalThis.__scHits`, appends to a bounded
// order log, and mirrors both as JSON into a plain-DOM `<output id="sc-hits">` that sits outside
// `#root`. The element is one pixel, transparent and not `hidden` because the oracle's
// `wait-for-selector` waits for it to be visible and then keeps its text as a checkpoint; the
// fast path reads the global. Neither engine needs more than the existing steps.

export const PRELUDE_MARK = "// __sc_hits__\n";
export const HITS_SELECTOR = "#sc-hits";
/** Longest order log kept (ids, in the order they ran). */
export const ORDER_LIMIT = 400;

/** Appended to `runText`. Function declarations hoist, so a counter may run before this line is evaluated. */
export const INSTRUMENT_PRELUDE = `${PRELUDE_MARK}function __h(id: number): void {
  const g = globalThis as unknown as { __scHits?: Record<number, number>; __scOrder?: number[] };
  const hits = (g.__scHits ??= {});
  hits[id] = (hits[id] ?? 0) + 1;
  const order = (g.__scOrder ??= []);
  if (order.length < ${ORDER_LIMIT}) order.push(id);
  const out = document.getElementById("sc-hits");
  if (out) out.textContent = JSON.stringify({ h: hits, o: order });
}
function __scInit(): void {
  if (document.getElementById("sc-hits")) return;
  const out = document.createElement("output");
  out.id = "sc-hits";
  out.setAttribute("style", "position:fixed;left:0;top:0;width:1px;height:1px;overflow:hidden;opacity:0;pointer-events:none");
  out.textContent = "{}";
  document.body.append(out);
}
__scInit();
`;

/**
 * Removes every counter from an instrumented text. Pure and independent of the printer's clean path, so
 * `stripInstrumentation(runText) === text` is a real test of neutrality (a counter placed anywhere else
 * leaves a residue, and a line that moved shows up as a different text).
 */
export function stripInstrumentation(runText) {
  const cut = runText.indexOf(PRELUDE_MARK);
  const source = cut < 0 ? runText : runText.slice(0, cut);
  // Function-entry counters: `{ __h(N); }` (an empty body) and `{ __h(N);` followed by the statements.
  const entries = source.replace(/\{ __h\(\d+\); \}/g, "{}").replace(/\{ __h\(\d+\);/g, "{")
    // Statement counters: `__h(N); <statement>` on the statement's own line.
    .replace(/^( *)__h\(\d+\); /gm, "$1");
  // Expression counters `(__h(N), <expr>)`: drop the prefix and the matching close.
  const prefix = /^\(__h\(\d+\), /;
  let out = "";
  let depth = 0;
  const open = [];
  for (let at = 0; at < entries.length;) {
    const ch = entries[at];
    if (ch === "\"" || ch === "'" || ch === "`") {
      let end = at + 1;
      while (end < entries.length && entries[end] !== ch) end += entries[end] === "\\" ? 2 : 1;
      out += entries.slice(at, end + 1);
      at = end + 1;
      continue;
    }
    if (ch === "(") {
      const match = prefix.exec(entries.slice(at, at + 24));
      if (match) { depth += 1; open.push(depth); at += match[0].length; continue; }
      depth += 1;
    } else if (ch === ")") {
      if (open.at(-1) === depth) { open.pop(); depth -= 1; at += 1; continue; }
      depth -= 1;
    }
    out += ch;
    at += 1;
  }
  return out;
}

/** The ids that appear in an instrumented text, in order. */
export const counterIds = (runText) => {
  const cut = runText.indexOf(PRELUDE_MARK);
  return [...(cut < 0 ? runText : runText.slice(0, cut)).matchAll(/__h\((\d+)\)/g)].map((match) => Number(match[1]));
};

/** Parses the readout (`{"h":{id:n},"o":[ids]}`, the #sc-hits text) or null when it is absent or malformed. */
export function parseHitsText(text) {
  if (typeof text !== "string" || !text.trim()) return null;
  try {
    const parsed = JSON.parse(text);
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed) || (parsed.h !== undefined && typeof parsed.h !== "object")) return null;
    return { hits: normalizeHits(parsed.h ?? {}), order: Array.isArray(parsed.o) ? parsed.o.map(Number) : [] };
  } catch { return null; }
}

/** `{ "7": 2 }` -> `{ 7: 2 }` with numeric counts; keys that are not ids are dropped. */
export function normalizeHits(hits) {
  const out = {};
  for (const [id, count] of Object.entries(hits ?? {})) if (/^\d+$/.test(id) && Number.isFinite(Number(count))) out[Number(id)] = Number(count);
  return out;
}

/** How many times a counter ran: a number, or null when there is no readout at all. */
export const hitCount = (hits, id) => (hits === null || hits === undefined || id === undefined || id === null ? null : Number(hits[id] ?? 0));

/**
 * The record of hits per place: for each ledger row `{ hit, ref, kind, line, hits, ran }`, plus, for a payload site, `landed`
 * (its witness text was shown). `ran` and `landed` are different facts: a void or unwitnessed site can run without a page
 * text, and a site can run and not land.
 * @param {object[]} ledger rows from printProgram
 * @param {object|null} hits `{ id: count }` from a run, or null when the run was not instrumented
 * @param {{ siteId: string, reached: boolean|null }[]} [witnesses]
 */
export function hitRecord(ledger, hits, witnesses = []) {
  const landed = new Map(witnesses.map((w) => [w.siteId, w.reached ?? null]));
  return ledger.map((row) => {
    const count = hitCount(hits, row.hit);
    return { hit: row.hit, ref: row.ref, kind: row.kind, node: row.node, line: row.line, hits: count, ran: count === null ? null : count > 0, ...(row.kind === "site" ? { landed: landed.get(row.ref) ?? null } : {}) };
  });
}

/** A compact per-program summary of the record: each payload site's hits and landing, and counts for the rest. */
export function hitSummary(ledger, hits, witnesses = [], quiescent = null) {
  const record = hitRecord(ledger, hits, witnesses);
  const count = (kind) => ({ ran: record.filter((r) => r.kind === kind && r.ran).length, total: record.filter((r) => r.kind === kind).length });
  return {
    instrumented: hits !== null && hits !== undefined, quiescent,
    sites: record.filter((r) => r.kind === "site").map(({ ref, line, hits: n, ran, landed }) => ({ site: ref, line, hits: n, ran, landed })),
    glue: count("auto"), entries: count("entry"),
  };
}

// ---------------------------------------------------------------------------
// The neutrality gate's comparisons (pure; the engines that feed them are in pipeline.mjs and neutrality.mjs).

/** Diagnostics whose position says nothing about the program (see `outcomeOf`). */
const CONSEQUENCE_CODES = ["REACTIVITY_HALTED"];

const multiset = (rows) => rows.reduce((counts, key) => ({ ...counts, [key]: (counts[key] ?? 0) + 1 }), {});

/**
 * The part of a runtime record the instrumentation must not move: the multiset of (code, innermost App line)
 * diagnostics, the set of thrown errors, the witness texts, and the failure phase.
 */
export function outcomeOf(runtime) {
  // REACTIVITY_HALTED is the engine's consequence of an earlier throw, raised wherever the next operation happens; its
  // frame is a position in the compiled module (past the end of the file), which shifts with the text. Its code counts, its line does not.
  const diagnostics = (runtime.diagnostics ?? []).map((d) => `${d.code}@${CONSEQUENCE_CODES.includes(d.code) ? "-" : d.positions?.[0]?.line ?? "-"}`).sort();
  const errors = (runtime.errors ?? []).map((e) => `${e.kind}:${String(e.message).slice(0, 100)}`).sort();
  const witnesses = Object.fromEntries((runtime.witnesses ?? []).map((w) => [w.siteId, w.actual ?? null]));
  return { diagnostics: multiset(diagnostics), errors: multiset(errors), witnesses, failure: runtime.failure?.phase ?? null };
}

/** Differences between two outcomes, as readable strings (empty when they match). */
export function outcomeDiff(instrumented, clean) {
  const a = outcomeOf(instrumented);
  const b = outcomeOf(clean);
  const diffs = [];
  for (const key of ["diagnostics", "errors"]) if (JSON.stringify(a[key]) !== JSON.stringify(b[key])) diffs.push(`${key}: instrumented ${JSON.stringify(a[key])} vs clean ${JSON.stringify(b[key])}`);
  for (const id of new Set([...Object.keys(a.witnesses), ...Object.keys(b.witnesses)])) {
    if ((a.witnesses[id] ?? null) !== (b.witnesses[id] ?? null)) diffs.push(`witness ${id}: instrumented ${JSON.stringify(a.witnesses[id] ?? null)} vs clean ${JSON.stringify(b.witnesses[id] ?? null)}`);
  }
  if (a.failure !== b.failure) diffs.push(`failure: instrumented ${a.failure} vs clean ${b.failure}`);
  return diffs;
}

/** The checker's findings as a set of `rule|kind|line` keys (the form the instrumentation must not change). */
export const findingKeys = (findings) => [...new Set((findings ?? []).map((f) => `${f.rule}|${f.kind}|${f.line}`))].sort();

/** Differences between the checker's findings on the clean text and on the instrumented text. */
export function findingsDiff(clean, perturbed) {
  const a = new Set(findingKeys(clean));
  const b = new Set(findingKeys(perturbed));
  return [...[...a].filter((k) => !b.has(k)).map((k) => `missing on the instrumented text: ${k}`), ...[...b].filter((k) => !a.has(k)).map((k) => `new on the instrumented text: ${k}`)];
}

/**
 * Compares verdicts of the same programs run with and without instrumentation.
 * Only an `inconclusive` verdict may move, and only to a resolved one; every other change is a neutrality failure.
 * @param {Map<string,string>|Record<string,string>} base verdict by program id, clean run
 * @param {Map<string,string>|Record<string,string>} instrumented verdict by program id, instrumented run
 */
export function compareVerdictRuns(base, instrumented) {
  const entries = (value) => (value instanceof Map ? [...value] : Object.entries(value));
  const other = new Map(entries(instrumented));
  const same = [];
  const resolved = [];
  const failures = [];
  const missing = [];
  for (const [id, was] of entries(base)) {
    if (!other.has(id)) { missing.push(id); continue; }
    const now = other.get(id);
    if (now === was) same.push(id);
    else if (was === "inconclusive" && now !== "inconclusive") resolved.push({ id, from: was, to: now });
    else failures.push({ id, from: was, to: now });
  }
  return { total: same.length + resolved.length + failures.length, same: same.length, resolved, failures, missing, equal: failures.length === 0 && missing.length === 0 };
}
