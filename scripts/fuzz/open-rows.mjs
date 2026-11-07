// open-rows.mjs: writes open-defects-rows.json, the exact grid rows open-defects.json holds, from a smoke run at one sample seed:
//   node scripts/fuzz/open-rows.mjs --update <finished smoke dir> [--allow-new] [--allow-gone]
// The only writer of that file; review its diff. Each row: its outcome, `certified`, and its cause (a family's `item` for a family someone
// already named). The seed comes from the run. New rows need --allow-new; any changed count needs the ratchet in open-defects.json edited
// in the same diff (exact both ways, open-defects.test.mjs), certified and false-positive rows counted on their own.
import { existsSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { rowProblem, withholdingRules } from "./codes.mjs";
import { F1_OPEN, F2_OPEN, OPEN_DEFECTS, OPEN_ROWS, STOP_NAMED_U, finalBlockers, heldGridEntry, ledgerCounts, namedKey, readOpenDefects } from "./open-defects.mjs";

/** What the list may hold. Any other problem (wrong-rule, harness, tsc-overlap) stays a failure. */
const LEDGERED = ["false-clean", "misplaced", "unplaced", "false-positive", "secondary-false-clean", "not-legal"];
/** Rows the ledger may not hold: certified CONTEXT_NOT_FOUND, a real hole R5 is closing. They fail the smoke until that lands. */
export const NOT_LISTED = (row) => /^flw-guard-(?:early|probe-loop)$/.test(String(row.context ?? ""));
const ALIAS_BINDINGS = ["ns-default", "ns-member", "extracted"];
const PLACELESS = ["CAUGHT_REACTIVE_READ", "SETTLED_CLEANUP_UNOWNED", "ASYNC_OUTSIDE_LOADING_BOUNDARY"];

/** Why a failing row fails, from what the row itself says. A row of a family already named keeps that name. */
export function causeOf(row, problem, entry) {
  if (/^flw-owned-/.test(String(row.context ?? ""))) return F2_OPEN;
  if (row.context === "flw-guard-yield") return F1_OPEN;
  if (entry && !entry.item.startsWith("R-")) return entry.item;
  if (problem === "unplaced") return "R-unplaced";
  if (problem === "misplaced") return "R-misplaced";
  if (problem === "not-legal") return "R-not-legal";
  if (problem !== "false-clean") return problem;
  const variant = String(row.variant ?? "");
  if (/^(?:store-)?proto-|^installed-|^use-|^net-|^cast-|^store-dstr-/.test(variant)) return "R-carrier";
  if (variant === "refresh") return "R-call-arg";
  if (PLACELESS.includes(row.code)) return "R-placeless";
  if (ALIAS_BINDINGS.includes(row.binding) || (/^flw-(?:guard|getter)-/.test(String(row.context ?? "")) && ["call", "or-assign"].includes(row.binding))) return "R-alias-call";
  const atSite = (row.detail?.findings ?? []).map((finding) => finding.split("/")[0]);
  if (atSite.some((rule) => !withholdingRules.includes(rule))) return "R-neighbour-U";
  return row.detail?.withheldNear ? "R-near" : "R-other";
}

/**
 * Owner's ruling on the triage (ADR 0815): a false clean of a generator-only shape (the 10 variants measured; CAUGHT in an `evt-*` or
 * `flw-*` context) with a named uncertifiable in the statement holding the read (Chrome's frame on the site's lines) stops there. The row keeps its outcome and is listed
 * by id under this cause, with the blocker. Nothing else enters: not a shape real code has, not any other group, and not a row with no
 * such finding, which keeps its own cause and is returned as rejected.
 */
const STOP_VARIANTS = ["installed-set-reflect", "installed-set-alias", "store-proto-merge", "store-proto-literal", "store-proto-string", "store-proto-member", "proto-key-handed", "proto-member", "proto-literal", "proto-method-this"];
export const inStopGroup = (row) => row.outcome === "false-clean" && (STOP_VARIANTS.includes(row.variant) || (row.code === "CAUGHT_REACTIVE_READ" && /^(?:evt|flw)-/.test(String(row.context ?? ""))));

/** Rules whose uncertifiable is about the read itself: a CAUGHT row stops only on one starting exactly at the read Chrome reported. */
const READ_RULES = ["strict-read-untracked", "pending-async-unsuspendable-read", "async-outside-loading-boundary"];

/**
 * The finding a row of the stop group stops on, or null. A neighbouring rule's U in the statement may say nothing about a caught read, so
 * CAUGHT takes a read rule's row at Chrome's exact position; the generator-only variants take any named row in the statement.
 */
export function stopFinding(row) {
  const named = row.detail?.namedU ?? [];
  if (row.code === "CAUGHT_REACTIVE_READ") return named.find((finding) => finding.atRead === true && READ_RULES.includes(finding.rule)) ?? null;
  return named[0] ?? null;
}

export function ledger(dir, seed, list) {
  const grids = {};
  const stopCheck = { listed: [], rejected: [], family: [] };
  const judged = new Set();
  for (const grid of readdirSync(dir).sort()) {
    const path = join(dir, grid, "programs.jsonl");
    if (!existsSync(path)) continue;
    const rows = {};
    for (const line of readFileSync(path, "utf8").split("\n").filter(Boolean)) {
      const row = JSON.parse(line);
      judged.add(`${grid}/${row.id}`);
      const problem = rowProblem(row);
      if (!LEDGERED.includes(problem) || NOT_LISTED(row)) continue;
      const named = inStopGroup(row) ? stopFinding(row) : null;
      const entry = heldGridEntry(list, row, problem);
      if (inStopGroup(row)) { (entry && !entry.item.startsWith("R-") ? stopCheck.family : named ? stopCheck.listed : stopCheck.rejected).push(row.id); }
      const stops = named && !(entry && !entry.item.startsWith("R-"));
      const owner = list.owners?.[`${grid}/${row.id}`];
      rows[row.id] = { outcome: problem, ...(row.detail?.certified === true ? { certified: true } : {}), cause: stops ? STOP_NAMED_U : causeOf(row, problem, entry), ...(stops ? { blocker: namedKey(named), ...(named.atRead ? { atRead: true } : {}) } : {}), ...(owner ? { owner } : {}) };
    }
    if (Object.keys(rows).length) grids[grid] = Object.fromEntries(Object.entries(rows).sort(([a], [b]) => (a < b ? -1 : 1)));
  }
  // A listed row the run did not judge has not stopped failing: the smoke takes every ledger id (cover.mjs requiredIds), so one is missing only
  // when the grid no longer generates it, and the update says so instead of dropping it as if fixed.
  const unjudged = Object.entries(list.rows?.grids ?? {}).flatMap(([grid, rows]) => Object.keys(rows).map((id) => `${grid}/${id}`)).filter((key) => !judged.has(key));
  return { unjudged, rows: { note: "Generated by open-rows.mjs. R-F2-open (flw-owned-*) holds open defects with certified false cleans, their fix in R5's queue: it must reach 0 before #173 moves, and make flip-final fails until it does. So must R-F1-open (flw-guard-yield), W's F1.", seed: Number(seed), complete: false, grids }, stopCheck };
}

/** One row a line, so the diff of an update is the rows that moved. */
export const printLedger = ({ grids, ...head }) => `{\n${Object.entries(head).map(([key, value]) => ` ${JSON.stringify(key)}: ${JSON.stringify(value)},\n`).join("")} "grids": {\n${Object.entries(grids).map(([grid, rows]) => `  ${JSON.stringify(grid)}: {\n${Object.entries(rows).map(([id, row]) => `   ${JSON.stringify(id)}: ${JSON.stringify(row)}`).join(",\n")}\n  }`).join(",\n")}\n }\n}\n`;

/** A run the ledger may come from: finished, every grid it selected read, its seed in its own report. */
export function runSeed(dir) {
  const report = join(dir, "perf.json");
  if (!existsSync(report)) throw new Error(`${dir}: no perf.json, the run did not finish`);
  const { sampleSeed, selection } = JSON.parse(readFileSync(report, "utf8"));
  if (sampleSeed === undefined) throw new Error(`${dir}: not a smoke run with a recorded sample seed`);
  const missing = String(selection.grids).split(",").filter((grid) => !existsSync(join(dir, grid, "programs.jsonl")));
  if (missing.length) throw new Error(`${dir}: stopped or missing grids: ${missing.join(", ")}`);
  return sampleSeed;
}

/** The ids an update adds and removes. */
export function ledgerChange(before, after) {
  const ids = (rows) => new Set(Object.entries(rows?.grids ?? {}).flatMap(([grid, list]) => Object.keys(list).map((id) => `${grid}/${id}`)));
  const was = ids(before);
  const now = ids(after);
  const causes = (rows) => new Map(Object.entries(rows?.grids ?? {}).flatMap(([grid, list]) => Object.entries(list).map(([id, row]) => [`${grid}/${id}`, row.cause])));
  const old = causes(before);
  const moved = [...causes(after)].filter(([id, cause]) => old.has(id) && old.get(id) !== cause).map(([id, cause]) => ({ id, from: old.get(id), to: cause }));
  return { added: [...now].filter((id) => !was.has(id)), removed: [...was].filter((id) => !now.has(id)), moved };
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2);
  if (args.includes("--final")) {
    const blockers = finalBlockers(readOpenDefects());
    console.log(`final gate: ${blockers.length} rows under ${F1_OPEN} or ${F2_OPEN}, certified or false positive`);
    process.exit(blockers.length ? 1 : 0);
  }
  const at = args.indexOf("--update");
  if (at < 0) throw new Error("usage: open-rows.mjs --update <smoke dir> [--allow-new]");
  const dir = args[at + 1];
  const before = existsSync(OPEN_ROWS) ? JSON.parse(readFileSync(OPEN_ROWS, "utf8")) : { grids: {} };
  const list = { ...JSON.parse(readFileSync(OPEN_DEFECTS, "utf8")), rows: before };
  const { rows: out, stopCheck, unjudged } = ledger(dir, runSeed(dir), list);
  for (const id of unjudged) console.log(`not in this run ${id}`);
  if (unjudged.length && !args.includes("--allow-gone")) throw new Error(`${unjudged.length} listed rows are not in this run: the grid no longer generates them, or the run missed the ledger's ids. Read them; --allow-gone drops them`);
  console.log(`stop rule: ${stopCheck.listed.length} checked and listed, ${stopCheck.family.length} in a named family, ${stopCheck.rejected.length} rejected (no named uncertifiable on the site's line)`);
  for (const id of stopCheck.rejected) console.log(`rejected ${id}`);
  const { added, removed, moved } = ledgerChange(before, out);
  for (const id of added) console.log(`+ ${id}`);
  for (const id of removed) console.log(`- ${id}`);
  for (const row of moved) console.log(`~ ${row.id}: ${row.from} -> ${row.to}`);
  const blockers = { read: {}, variant: {} };
  for (const rows of Object.values(out.grids)) for (const row of Object.values(rows)) if (row.blocker) { const name = row.blocker.split("@")[0]; const group = row.atRead ? blockers.read : blockers.variant; group[name] = (group[name] ?? 0) + 1; }
  console.log(`stop rule blockers: ${JSON.stringify(blockers)}`);
  if (added.length && !args.includes("--allow-new")) throw new Error(`${added.length} new rows: a new failing row is a defect to read, not to list. Pass --allow-new to list them`);
  writeFileSync(OPEN_ROWS, printLedger(out));
  console.log(JSON.stringify(ledgerCounts(out), null, 1));
}
