// False cleans as a ledgered, coarsely deduped stream (phase D, proposal section 4).
//
// A false clean is a defect Solid reports (a code the policy table calls a defect, codes.mjs) where the checker says
// nothing. This module owns everything after `classify` finds one:
//   * the coarse key: the code, the chain of primitives above the diagnostic, and the dynamic stack shape,
//     so a hundred wrappers around one root cause are one group (the fine key, dedupe.mjs, keeps them apart);
//   * the ledger of accepted trades (known-open.json) and its validation: no wildcards, a real reference;
//   * Chrome's confirmation of a candidate and of its minimum;
//   * the focused shrink's predicate and its control (the legal twin).
// Pure functions; run.mjs wires them to the pipeline.
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { runtimeEvents } from "./classify.mjs";
import { isDefectCode, policyOf, severityRank } from "./codes.mjs";
import { clone, getAt, isSite, isValid, num, slotsOf, walk } from "./model.mjs";
import { nodesAtLine } from "./print.mjs";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
export const LEDGER_PATH = join(dirname(fileURLToPath(import.meta.url)), "known-open.json");

// --- The coarse key --------------------------------------------------------------------------------------------

/** The primitive a node stands for in a chain: `createMemo`, `runWithOwner(null, ...)`, `onCleanup(...)`, or null. */
function primToken(node) {
  if (node.k === "prim") return node.name === "owner" ? `owner:${node.ownerArg ?? "null"}` : node.name;
  if (node.k === "own" && node.op !== "context") return node.op;
  return null;
}

/** The ancestors of the node at `path` (the node itself included), root first. */
function ancestorsOf(program, path) {
  const out = [program];
  let node = program;
  for (const key of path) {
    node = node?.[key];
    if (node && typeof node === "object" && typeof node.k === "string") out.push(node);
  }
  return out;
}

/**
 * The chain of primitives above the node at `path`, root first: `root>untrack`, `memo>effect`, or "-" for none. The node itself
 * is left out when it is an owner call (`onCleanup`, `createEffect`): it is the site, and the code already names it.
 */
export function primChain(program, path) {
  const nodes = ancestorsOf(program, path);
  const chain = nodes.filter((node, index) => !(index === nodes.length - 1 && node.k === "own")).map(primToken).filter(Boolean);
  return chain.length ? chain.join(">") : "-";
}

/** A node label, coarsened to a frame's dynamic role: the primitive a frame runs in, a handler, JSX; wrappers (functions, calls, statements) are noise. */
export function frameKind(label) {
  if (label === undefined) return null;
  if (label.startsWith("prim:")) return label;
  if (label === "button") return "handler";
  if (["child", "attr", "show", "for", "probe"].includes(label)) return "jsx";
  return null;
}

const MAX_FRAMES = 12;
/**
 * The dynamic stack shape: the roles of the diagnostic's frames, innermost first, runs collapsed. Wrapper layers (closures,
 * helpers, calls) change the stack's depth and not its cause, so only frames inside a primitive, a handler or JSX count: where the
 * code was entered from (a click, a timer, an effect's compute) is what separates root causes.
 */
export function stackShape(lineTable, frames) {
  const kinds = [];
  for (const line of (frames ?? []).slice(0, MAX_FRAMES)) {
    // The deepest node covering the frame's line that has a role; a line the table does not know has none.
    const kind = nodesAtLine(lineTable, line).map((row) => frameKind(row.label)).find(Boolean) ?? null;
    if (kind && kinds.at(-1) !== kind) kinds.push(kind);
  }
  return kinds.length ? kinds.join("<") : "-";
}

/** The call a diagnostic at `line` is about: an owner call or a payload read/write there beats a deeper node that is only its argument. */
export function callAtLine(lineTable, line) {
  const rows = nodesAtLine(lineTable, line);
  return rows.find((row) => /^(own:|read$|write$)/.test(row.label)) ?? rows[0] ?? null;
}

/**
 * The coarse key of a false clean: code, primitive chain, stack shape. `diagnostic` is a classification item's
 * (`code`, `line`, `frames`). An unplaced diagnostic (no App line) has no chain and no shape.
 */
export function coarseKey({ diagnostic, program, lineTable }) {
  if (diagnostic.line === null || diagnostic.line === undefined) return `${diagnostic.code}|-|unplaced`;
  const node = callAtLine(lineTable, diagnostic.line);
  const chain = node ? primChain(program, node.path) : "?";
  return `${diagnostic.code}|${chain}|${stackShape(lineTable, diagnostic.frames?.length ? diagnostic.frames : [diagnostic.line])}`;
}

/** The false-clean items of a classification, each with its coarse key. */
export function falseCleanItems(value) {
  return (value.classification?.items ?? []).filter((item) => ["candidate-false-clean", "known-open"].includes(item.kind) && item.diagnostic)
    .map((item) => ({ item, key: coarseKey({ diagnostic: item.diagnostic, program: value.program, lineTable: value.built.lineTable }) }));
}

// --- The ledger ------------------------------------------------------------------------------------------------

/** The GitHub anchor of a markdown heading. */
export const slug = (heading) => heading.trim().toLowerCase().replace(/[^\p{L}\p{N}\s-]/gu, "").replace(/\s/g, "-");

/** Anchors of `docs/<file>` headings, and the ADR numbers of `docs/decisions.md`. */
export function docTargets(root = repo) {
  const read = (name) => (existsSync(join(root, "docs", name)) ? readFileSync(join(root, "docs", name), "utf8") : "");
  const headings = (text) => [...text.matchAll(/^#{1,6}\s+(.+)$/gm)].map((match) => match[1]);
  return {
    anchors: new Map([["open-work.md", new Set(headings(read("open-work.md")).map(slug))]]),
    adrs: new Set([...read("decisions.md").matchAll(/^## ADR (\d{4}) /gm)].map((match) => match[1])),
  };
}

/** Problems with a ledger, as strings (empty when valid). A reference must resolve; nothing is a wildcard. */
export function ledgerProblems(entries, targets = docTargets()) {
  if (!Array.isArray(entries)) return ["the ledger is not an array"];
  const problems = [];
  const seen = new Set();
  entries.forEach((entry, index) => {
    const where = `entry ${index} (${entry?.code ?? "?"})`;
    for (const field of ["code", "coarseKey", "ref", "reason"]) if (typeof entry?.[field] !== "string" || !entry[field].trim()) problems.push(`${where}: ${field} is missing`);
    if (typeof entry?.code !== "string" || typeof entry?.coarseKey !== "string") return;
    if (!isDefectCode(entry.code)) problems.push(`${where}: ${entry.code} is not a defect code in the policy table`);
    if (/[*?]/.test(entry.coarseKey)) problems.push(`${where}: the coarse key has a wildcard; a ledger entry names one exact key`);
    if (!entry.coarseKey.startsWith(`${entry.code}|`)) problems.push(`${where}: the coarse key does not start with its code`);
    const duplicate = `${entry.code}|${entry.coarseKey}`;
    if (seen.has(duplicate)) problems.push(`${where}: duplicate entry`);
    seen.add(duplicate);
    const ref = String(entry.ref ?? "");
    const anchor = ref.match(/^(open-work\.md)#([a-z0-9-]+)$/);
    const adr = ref.match(/^ADR (\d{4})$/);
    if (anchor) { if (!targets.anchors.get(anchor[1])?.has(anchor[2])) problems.push(`${where}: no anchor ${ref} in docs/open-work.md`); }
    else if (adr) { if (!targets.adrs.has(adr[1])) problems.push(`${where}: no ${ref} in docs/decisions.md`); }
    else problems.push(`${where}: ref ${JSON.stringify(ref)} is neither "open-work.md#anchor" nor "ADR nnnn"`);
  });
  return problems;
}

/** Reads and validates the ledger; throws on a problem (a bad ledger must not quiet a sweep). */
export function loadLedger(path = LEDGER_PATH, targets) {
  const entries = JSON.parse(readFileSync(path, "utf8"));
  const problems = ledgerProblems(entries, targets);
  if (problems.length) throw new Error(`${path}: ${problems.join("; ")}`);
  return entries;
}

/**
 * Gives every false-clean item its coarse key, turns the ones the ledger accepts into `known-open` (with the entry's ref and
 * reason) and recomputes the program's verdict. Returns the entries that fired. Mutates `value.classification`.
 */
export function applyLedger(value, ledger, rank) {
  const fired = [];
  const items = value.classification?.items;
  if (!items?.length) return fired;
  for (const { item, key } of falseCleanItems(value)) {
    item.coarseKey = key;
    const entry = ledger.find((candidate) => candidate.code === item.diagnostic.code && candidate.coarseKey === key);
    if (!entry || item.kind !== "candidate-false-clean") continue;
    item.kind = "known-open";
    item.ref = entry.ref;
    item.reason = `${item.reason}; accepted by the ledger (${entry.ref}): ${entry.reason}`;
    fired.push(entry);
  }
  if (fired.length) {
    value.classification.verdict = items.reduce((worst, row) => (rank(row.kind) < rank(worst) ? row.kind : worst), "agree");
    value.classification.reasons = [...new Set(items.filter((row) => row.kind === value.classification.verdict).map((row) => row.reason))];
  }
  return fired;
}

/** Ledger entries that fired in no program: `stale-open` when the run covered the profile the entry came from. */
export const staleOpen = (ledger, fired) => ledger.filter((entry) => !fired.some((hit) => hit.code === entry.code && hit.coarseKey === entry.coarseKey));

// --- Groups ----------------------------------------------------------------------------------------------------

/**
 * Groups the new (not known-open) false cleans by coarse key. Worst severity first, then the biggest group; the smallest
 * program represents a group.
 * @param {{ id: string, key: string, item: object, size: number, value: object }[]} entries
 */
export function groupFalseCleans(entries) {
  const groups = new Map();
  for (const entry of entries) {
    const group = groups.get(entry.key) ?? { coarseKey: entry.key, code: entry.item.diagnostic.code, severity: entry.item.diagnostic.severity, count: 0, members: [], representative: entry };
    group.count += 1;
    group.members.push(entry.id);
    if (entry.size < group.representative.size) group.representative = entry;
    groups.set(entry.key, group);
  }
  return [...groups.values()].sort((a, b) => severityRank(a.code) - severityRank(b.code) || b.count - a.count || a.coarseKey.localeCompare(b.coarseKey));
}

/** Recall gaps (the checker declined at a runtime diagnostic) ranked by code, rule and shape: a roadmap metric, never a failure. */
export function rankRecallGaps(rows) {
  const counts = new Map();
  for (const { item, key } of rows) {
    const label = `${key}|${item.diagnostic.rule}`;
    counts.set(label, (counts.get(label) ?? 0) + 1);
  }
  return [...counts].map(([signature, count]) => ({ signature, count })).sort((a, b) => b.count - a.count || a.signature.localeCompare(b.signature));
}

// --- Chrome's confirmation ---------------------------------------------------------------------------------------

/**
 * What Chrome says about a false-clean candidate. `judged` is the comparator's classification of Chrome's records
 * (`classifyChrome`), `record` Chrome's raw runtime and checker.
 *   confirmed            Solid reported the code at the innermost App line in Chrome too, and no finding covers it
 *   confirmed-unplaced   Chrome's diagnostic of the code could not be placed on a line, or the candidate had no line: Solid
 *                        still spoke (for a false clean that counts; for a false positive it would not), and no finding claims it
 *   line-differs         Chrome reports the code, at another line: not reproduced
 *   covered-in-chrome    Chrome's checker has a finding at the line
 *   refuted-by-chrome    Chrome does not report the code
 *   chrome-inconclusive  the Chrome run itself failed or its checker was perturbed
 */
export function confirmFalseClean({ item, judged, record }) {
  const { code, line } = item.diagnostic;
  if (!judged || !record || record.runtime?.failure || !record.checker?.ok || record.checker.perturbed?.length) return { status: "chrome-inconclusive", reason: judged?.reasons?.[0] ?? "no usable Chrome run" };
  const chromeItems = judged.items.filter((candidate) => candidate.diagnostic?.code === code);
  const placed = chromeItems.filter((candidate) => candidate.kind === "candidate-false-clean" && candidate.diagnostic.line !== null);
  if (line !== null && placed.some((candidate) => candidate.diagnostic.line === line)) return { status: "confirmed", placed: true };
  if (chromeItems.some((candidate) => ["recall-gap", "agree"].includes(candidate.kind) && (line === null || candidate.diagnostic.line === line))) return { status: "covered-in-chrome" };
  if (placed.length) return { status: "line-differs", chromeLines: placed.map((candidate) => candidate.diagnostic.line) };
  const claims = (record.checker.findings ?? []).length;
  const unplaced = chromeItems.some((candidate) => candidate.kind === "candidate-false-clean" && candidate.diagnostic.line === null)
    || ((record.runtime.unattributed ?? []).map(String).includes(code) && !runtimeEvents(record.runtime).some((event) => event.code === code && event.positions.length));
  if (unplaced && !(claims && chromeItems.some((candidate) => candidate.kind !== "candidate-false-clean"))) return { status: "confirmed-unplaced", placed: false };
  return { status: "refuted-by-chrome" };
}
export const isConfirmed = (status) => ["confirmed", "confirmed-unplaced"].includes(status);

// --- The shrink's predicate and control ----------------------------------------------------------------------------

/**
 * Does this evaluated program still show the target false clean? Same code, same coarse key, no covering finding (the
 * item is a `candidate-false-clean`, which the policy table and the comparator already guarantee).
 */
export function stillShowsFalseClean(value, target) {
  return falseCleanItems(value).some(({ item, key }) => item.kind === "candidate-false-clean" && item.diagnostic.code === target.code && key === target.key);
}

/**
 * The legal twin of a program: the same tree with every payload site replaced by a literal. What is left is legal by
 * construction (no defect), so Solid must not report the code and the checker must not either. A shrink step that keeps the
 * code in the twin has kept something other than the payload that produces it, and is refused.
 * Returns null when the twin is not a valid program.
 */
export function legalTwin(program) {
  const twin = clone(program);
  let replaced = 0;
  for (const { node, path } of [...walk(twin)]) {
    for (const [key, isList] of slotsOf(node)) {
      if (isList) node[key].forEach((child, index) => { if (isSite(child)) { node[key][index] = num(1); replaced += 1; } });
      else if (isSite(node[key])) { node[key] = num(1); replaced += 1; }
    }
    void path;
  }
  return replaced && isValid(twin) ? twin : null;
}

/** The control holds when the twin's run reports no diagnostic of the code (and ran at all). */
export function twinIsClean(twinValue, code) {
  if (!twinValue?.runtime || !twinValue.checker?.ok || !twinValue.typecheck?.ok) return false;
  return !runtimeEvents(twinValue.runtime).some((event) => event.code === code)
    && !(twinValue.checker.findings ?? []).some((finding) => finding.kind === "violation" && (policyOf(code)?.rules ?? []).includes(finding.rule));
}

export { getAt };
