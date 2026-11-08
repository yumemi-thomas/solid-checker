// open-rows.mjs: writes open-defects-rows.json, the exact grid rows open-defects.json holds, from a smoke run at one sample seed:
//   node scripts/fuzz/open-rows.mjs --update <finished smoke dir> [--allow-new] [--allow-gone]
// The only writer of that file; review its diff. Each row: its outcome, `certified`, and its cause (a family's `item` for a family someone
// already named). The seed comes from the run. New rows need --allow-new; any changed count needs the ratchet in open-defects.json edited
// in the same diff (exact both ways, open-defects.test.mjs), certified and false-positive rows counted on their own.
import { existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { rowProblem, withholdingRules } from "./codes.mjs";
import { STOP_VARIANTS, readStopSource, stopPlan, stopProblems } from "./stop-holder.mjs";
import { ALIAS_OWNER, CASTS_STAGE2, F1_OPEN, F2_OPEN, OPEN_DEFECTS, OPEN_ROWS, HALTED, STACKED, STOP_NAMED_U, UNJUDGED, finalBlockers, heldGridEntry, ledgerCounts, namedKey, readOpenDefects } from "./open-defects.mjs";

/** What the list may hold. Any other problem (wrong-rule, harness, tsc-overlap) stays a failure. */
const LEDGERED = ["false-clean", "misplaced", "unplaced", "false-positive", "stacked-claim", "unjudged-claim", "halted-claim", "secondary-false-clean", "not-legal"];
/** Rows the ledger may not hold: certified CONTEXT_NOT_FOUND, a real hole R5 is closing. They fail the smoke until that lands. */
export const NOT_LISTED = (row) => /^flw-guard-(?:early|probe-loop)$/.test(String(row.context ?? ""));
const ALIAS_BINDINGS = ["ns-default", "ns-member", "extracted"];
/** The callee never resolves as a primitive in these bindings: a `flw-owned-*` row has no owner site (R5). */
const ALIAS_OWNER_BINDINGS = [...ALIAS_BINDINGS, "bind", "call", "or-assign", "ns-nested", "ns-held"];
/** The contexts casts stage 2 closes. */
const CASTS_STAGE2_CONTEXTS = ["patch-getproto", "patch-key-proto", "patch-key-alias", "patch-assign-key-proto"];
const PLACELESS = ["CAUGHT_REACTIVE_READ", "SETTLED_CLEANUP_UNOWNED", "ASYNC_OUTSIDE_LOADING_BOUNDARY"];

/** Why a failing row fails, from what the row itself says. A row of a family already named keeps that name. */
export function causeOf(row, problem, entry) {
  if (CASTS_STAGE2_CONTEXTS.includes(row.context)) return CASTS_STAGE2;
  if (/^flw-owned-/.test(String(row.context ?? ""))) return ALIAS_OWNER_BINDINGS.includes(row.binding) ? ALIAS_OWNER : F2_OPEN;
  if (row.context === "flw-guard-yield") return F1_OPEN;
  if (entry && !entry.item.startsWith("R-")) return entry.item;
  if (problem === "unplaced") return "R-unplaced";
  if (problem === "misplaced") return "R-misplaced";
  if (problem === "not-legal") return "R-not-legal";
  if (problem === "stacked-claim") return STACKED;
  if (problem === "unjudged-claim") return UNJUDGED;
  if (problem === "halted-claim") return HALTED;
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
      const stops = named && !(entry && !entry.item.startsWith("R-")) && !CASTS_STAGE2_CONTEXTS.includes(row.context);
      const owner = list.owners?.[`${grid}/${row.id}`];
      rows[row.id] = { outcome: problem, ...(row.detail?.certified === true ? { certified: true } : {}), cause: stops ? STOP_NAMED_U : causeOf(row, problem, entry), ...(stops ? { blocker: namedKey(named), ...(named.atRead ? { atRead: true } : {}) } : {}), ...(problem === "stacked-claim" ? { pairs: row.detail?.pairs ?? [] } : {}), ...(problem === "unjudged-claim" || problem === "halted-claim" ? { rules: row.detail?.rules ?? [] } : {}), ...(owner ? { owner } : {}) };
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

/** Every halted-claim row by id (`grid/id rules`): each report lists them (coordinator's ruling, Part 17). */
export const haltedIds = (rows) => Object.entries(rows?.grids ?? {}).flatMap(([grid, list]) => Object.entries(list).filter(([, row]) => row.outcome === "halted-claim").map(([id, row]) => `${grid}/${id} ${(row.rules ?? []).join(",")}`));

/** The holder-free check of `rows`' stop rows, on the smoke at `dir`'s programs. `--checker` and `--typefacts` name the build; a run with neither stops. */
async function stopHolderCheck(dir, rows, args) {
  const value = (name) => { const at = args.indexOf(name); return at < 0 ? undefined : args[at + 1]; };
  const [checker, typefacts] = [value("--checker"), value("--typefacts")];
  if (!checker || !typefacts) throw new Error("--update needs --checker and --typefacts: the stop rows' holder-free check runs on the build the smoke judged");
  const variants = new Map();
  for (const grid of Object.keys(rows.grids)) for (const line of readFileSync(join(dir, grid, "programs.jsonl"), "utf8").split("\n").filter(Boolean)) { const row = JSON.parse(line); variants.set(`${grid}/${row.id}`, row.variant); }
  const stop = Object.entries(rows.grids).flatMap(([grid, list]) => Object.entries(list).filter(([, row]) => row.cause === STOP_NAMED_U).map(([id]) => ({ id, grid, variant: variants.get(`${grid}/${id}`) })));
  const plan = stopPlan(stop, ({ grid, id }) => {
    const folder = join(dir, grid, "cases", id);
    return readStopSource(folder);
  });
  const root = mkdtempSync(join(tmpdir(), "stop-holder-"));
  // Every source file of the program, the holder removed where it was (stop-holder.mjs); App.tsx's component mounts as App.
  const cases = plan.checked.map(({ id, files }) => {
    const folder = join(root, "src", id); mkdirSync(folder, { recursive: true });
    for (const [name, text] of Object.entries(files)) writeFileSync(join(folder, name), name === "App.tsx" ? text.replace("export function C1()", "export function App()") : text);
    return { id, dir: folder };
  });
  const { runChecker } = await import("./patterns.mjs");
  const findings = await runChecker(cases, join(root, "work"), { checker, typefacts });
  rmSync(root, { recursive: true, force: true });
  return { checked: plan.checked.length, multiFile: plan.checked.filter(({ files }) => Object.keys(files).length > 1).length, unchecked: plan.unchecked.length, uncheckedIds: plan.unchecked, noHolder: stop.length - plan.checked.length - plan.unchecked.length - plan.unmatched.length, problems: stopProblems(plan, findings) };
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2);
  if (args.includes("--final")) {
    const blockers = finalBlockers(readOpenDefects());
    console.log(`final gate: ${blockers.length} rows under ${F1_OPEN}, ${F2_OPEN}, ${ALIAS_OWNER} or ${CASTS_STAGE2}, certified, false positive or unchecked stop row`);
    for (const id of readOpenDefects().rows?.stopUnchecked ?? []) console.log(`unchecked stop row ${id}`);
    for (const id of haltedIds(readOpenDefects().rows)) console.log(`halted claim ${id}`);
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
  // The stop rows must keep a finding with the generator's holder removed (stop-holder.mjs). Needs the build the run judged.
  const stopRun = await stopHolderCheck(dir, out, args);
  console.log(`stop holder check: ${stopRun.checked} checked (${stopRun.multiFile} of them multi-file), ${stopRun.unchecked} unchecked (no App.tsx), ${stopRun.noHolder} with no holder`);
  for (const problem of stopRun.problems) console.log(`stop holder ${problem}`);
  if (stopRun.problems.length) throw new Error(`${stopRun.problems.length} stop rows fail the holder-free check: a U that was the holder's is a false clean, back to its owner`);
  if (added.length && !args.includes("--allow-new")) throw new Error(`${added.length} new rows: a new failing row is a defect to read, not to list. Pass --allow-new to list them`);
  out.stopUnchecked = stopRun.uncheckedIds;
  writeFileSync(OPEN_ROWS, printLedger(out));
  for (const id of haltedIds(out)) console.log(`halted claim ${id}`);
  console.log(JSON.stringify(ledgerCounts(out), null, 1));
}
