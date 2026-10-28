// The false-clean stream of a sweep (phase D): collect the candidates the comparator found, group them by coarse key, let the
// ledger accept the known ones, have Chrome confirm one representative per new group, shrink it with a control, and
// reconfirm the minimum in Chrome. run.mjs calls `falseCleanStream`; everything it needs from the sweep comes in as arguments.
import { createHash } from "node:crypto";
import { mapPoolSettled } from "../lib/pool.mjs";
import { join } from "node:path";
import { classifyChrome, evaluate, judgeEntries } from "./pipeline.mjs";
import { VERDICTS } from "./classify.mjs";
import { buildCase, writeCase } from "./emit.mjs";
import {
  applyLedger, confirmFalseClean, falseCleanItems, groupFalseCleans, isConfirmed, legalTwin, rankRecallGaps, staleOpen, stillShowsFalseClean, twinIsClean,
} from "./falseclean.mjs";
import { isSite, walk } from "./model.mjs";
import { shrink } from "./shrink.mjs";
import { signature as findingSignature } from "./dedupe.mjs";
import { callAtLine } from "./falseclean.mjs";

const rank = (kind) => VERDICTS.indexOf(kind);

/** Marks the ledger's false cleans `known-open` in every result; returns the entries that fired. */
export function applyLedgerToResults(results, ledger) {
  const fired = [];
  for (const value of results.values()) if (value.classification?.items?.length) fired.push(...applyLedger(value, ledger, rank));
  return fired;
}

/** What the sweep knows about false cleans before Chrome: the new groups, the known-open count, the ranked recall gaps. */
export function collectFalseCleans(results) {
  const fresh = []; let knownOpen = 0; const gaps = [];
  for (const [id, value] of results) {
    if (!value.classification?.items) continue;
    for (const { item, key } of falseCleanItems(value)) {
      if (item.kind === "known-open") knownOpen += 1;
      else fresh.push({ id, key, item, value, size: value.built.text.split("\n").length });
    }
    for (const item of value.classification.items.filter((row) => row.kind === "recall-gap")) {
      const node = callAtLine(value.built.lineTable, item.diagnostic.line);
      gaps.push({ item, key: `${item.diagnostic.code}|${node ? node.label : "?"}` });
    }
  }
  return { groups: groupFalseCleans(fresh), knownOpen, recallGaps: rankRecallGaps(gaps) };
}

/** The fine signature of a false-clean item (dedupe.mjs), or null when it has no line. */
export function fineSignature(value, item) {
  const node = item.diagnostic.line === null ? null : callAtLine(value.built.lineTable, item.diagnostic.line);
  return node ? findingSignature({ rule: item.diagnostic.code, kind: item.kind, program: value.program, path: node.path, outcome: "diagnosed" }) : null;
}

/**
 * Focused shrink of a representative: same code and coarse key, no covering finding, and the legal twin stays clean.
 * @returns {{ program: object, dir: string, size: number, attempts: number, accepted: number, controlled: boolean }}
 */
export async function shrinkFalseClean(rep, config, options, out) {
  const target = { code: rep.item.diagnostic.code, key: rep.key };
  let counter = 0;
  const entryOf = (program, tag) => { counter += 1; return { program, built: buildCase(program, { id: `${rep.id}-${tag}${counter}` }) }; };
  const where = { ...config, casesDir: join(out, "shrink") };
  // The neutrality control is asked of a program the plain pass accepts (judgeEntries), not of every program tried.
  const judge = async (program) => (await judgeEntries([entryOf(program, "fc")], where, (_entry, result) => stillShowsFalseClean(result, target)))[0];
  // The control: the program with its payload removed must not show the code. A tree with no valid twin is not held to it.
  // `twinIsClean` reads the twin's own run, which the neutrality control does not change, so the twin runs without it.
  const control = async (program) => {
    const twin = legalTwin(program);
    if (twin === null) return true;
    const entry = entryOf(twin, "twin");
    return twinIsClean((await evaluate([entry], { ...where, control: false })).get(entry.built.id), target.code);
  };
  const cheap = (program) => [...walk(program)].some(({ node }) => isSite(node));
  const reduced = await shrink(rep.value.program, judge, { budget: options.budget, cheap, control });
  const built = buildCase(reduced.program, { id: `${rep.id}-min`, note: `shrunk from fuzz seed ${rep.value.program.seed} (${reduced.accepted} reductions, ${reduced.attempts} runs), false clean ${target.code}` });
  const dir = writeCase(join(out, "repros", createHash("sha1").update(built.text).digest("hex").slice(0, 10)), built);
  const lines = built.text.split("\n").length;
  console.error(`shrunk ${rep.id} (${target.code}): ${reduced.attempts} runs, ${reduced.accepted} accepted, ${lines} lines -> ${dir}`);
  return { program: reduced.program, built, dir, size: lines, attempts: reduced.attempts, accepted: reduced.accepted };
}

/**
 * The stream. `chromeRecords(values)` runs Chrome for evaluated values and returns a Map id -> record (run.mjs owns the oracle).
 * @returns {Promise<{ groups: object[], knownOpen: number, staleOpen: object[], recallGaps: object[], confirmed: number }>}
 */
export async function falseCleanStream({ results, ledger, firedEntries, config, options, out, chromeRecords }) {
  const { groups, knownOpen, recallGaps } = collectFalseCleans(results);
  const reps = groups.map((group) => group.representative);
  const records = options.confirmWithChrome && reps.length ? await chromeRecords(reps.map((rep) => rep.value)) : new Map();
  const shrinkable = [];
  for (const group of groups) {
    const rep = group.representative;
    const record = records.get(rep.id);
    group.status = !options.confirmWithChrome ? "candidate"
      : record ? confirmFalseClean({ item: rep.item, judged: classifyChrome(rep.value, record, config), record }).status : "chrome-inconclusive";
    group.signature = fineSignature(rep.value, rep.item);
    group.repro = join(config.casesDir, rep.id);
    group.size = rep.size;
    if (options.shrink && !["refuted-by-chrome", "line-differs", "covered-in-chrome", "chrome-inconclusive"].includes(group.status)) shrinkable.push(group);
  }
  // One reduction per group, a bounded pool across groups: each depends on its own runs only (the fast host serves them in turn).
  const reduced = await mapPoolSettled(shrinkable, (group) => shrinkFalseClean(group.representative, config, options, out), { concurrency: options.shrinkJobs ?? Math.min(4, shrinkable.length || 1) });
  const reconfirm = [];
  for (const [index, group] of shrinkable.entries()) {
    if (reduced[index].error) throw reduced[index].error;
    const minimal = reduced[index].value;
    group.repro = minimal.dir; group.size = minimal.size;
    group.minimum = { reductions: minimal.accepted, runs: minimal.attempts, status: "fast-only" };
    if (options.confirmWithChrome) reconfirm.push({ group, minimal });
  }
  // Reconfirm the minima in Chrome, all in one call: the fast path alone can drift.
  const values = await mapPoolSettled(reconfirm, async ({ minimal }) => (await evaluate([{ program: minimal.program, built: minimal.built }], { ...config, casesDir: join(out, "shrink") })).get(minimal.built.id), { concurrency: 1 });
  const chromed = reconfirm.map(({ group, minimal }, index) => {
    const value = values[index].value;
    const item = value && falseCleanItems(value).find(({ item: candidate }) => candidate.kind === "candidate-false-clean" && candidate.diagnostic.code === group.code);
    return { group, minimal, value, item };
  });
  const asked = chromed.filter(({ item }) => item);
  const again = asked.length ? await chromeRecords(asked.map(({ value }) => value)) : new Map();
  for (const { group, minimal, value, item } of chromed) {
    const record = item ? again.get(minimal.built.id) : null;
    group.minimum.status = record ? confirmFalseClean({ item: item.item, judged: classifyChrome(value, record, config), record }).status : "chrome-inconclusive";
  }
  let confirmed = 0;
  for (const group of groups) if (isConfirmed(group.status)) confirmed += 1;
  return { groups, knownOpen, staleOpen: staleOpen(ledger, firedEntries), recallGaps, confirmed };
}
