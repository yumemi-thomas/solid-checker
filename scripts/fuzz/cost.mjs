// The per-program cost record of a grid (measurement only: no verdict, row, cache key or finding identity reads it). Each program's
// checker wall time, and from the Rust executor's `wait4` (exec/src/main.rs `usage_fields`, Linux) its CPU time, peak RSS and block
// writes, go to `<grid>/costs.jsonl` beside programs.jsonl, one line per program; the role-log fold (fold-worker.mjs) adds each
// program's decision count when the role log is on. `<grid>/cost.json` is the summary: the checker's distribution, the slowest cells
// (code, context, layer, variant) and every program over the budget (default 1 s or 500 MB; matrix.mjs and legal.mjs take
// `--budget-ms` and `--budget-mb`, focus.mjs passes its own). A measure the run could not take is null with its reason, never zero.
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

export const DEFAULT_BUDGET = Object.freeze({ ms: 1000, mb: 500 });
/** How many cells the summary ranks. */
export const SLOWEST = 10;

/** The budget a grid's options name (`budgetMs`, `budgetMb`), else the default. */
export function budgetOf(options = {}) {
  const ms = options.budgetMs === undefined ? DEFAULT_BUDGET.ms : Number(options.budgetMs);
  const mb = options.budgetMb === undefined ? DEFAULT_BUDGET.mb : Number(options.budgetMb);
  if (!(ms > 0) || !(mb > 0)) throw new Error(`a cost budget is a positive number (--budget-ms ${options.budgetMs}, --budget-mb ${options.budgetMb})`);
  return { ms, mb };
}

/** A checker answer's cost (exec/src/main.rs: `wallMs`, `usage`); what it lacks is null, with the reason in `why`. */
export function costOf(answer) {
  if (!answer) return { checkerMs: null, cpuMs: null, maxRssKb: null, writeKb: null, why: "no checker run" };
  const usage = answer.usage;
  const checkerMs = typeof answer.wallMs === "number" && !answer.spawnError ? answer.wallMs : null;
  const cost = { checkerMs, cpuMs: usage?.cpuMs ?? null, maxRssKb: usage?.maxRssKb ?? null, writeKb: usage?.writeKb ?? null };
  const why = [checkerMs === null ? (answer.spawnError ? "the checker did not start" : "no wall time") : null, usage ? usage.usageWhy : "no usage in the checker answer (not the Rust executor)"].filter(Boolean);
  return why.length ? { ...cost, why: why.join("; ") } : cost;
}

/** A row's cost line: its identity and cell, and its value's cost (`value.cost`, pipeline.mjs). */
export function costLine(row, value) {
  const cost = value?.cost ?? (value?.error ? { checkerMs: null, cpuMs: null, maxRssKb: null, writeKb: null, why: "harness error" } : costOf(null));
  return { id: row.id, code: row.code, context: row.context, layer: row.layer, variant: row.variant ?? null, ...cost };
}

const quantile = (sorted, q) => (sorted.length ? sorted[Math.min(sorted.length - 1, Math.max(0, Math.ceil(q * sorted.length) - 1))] : null);
/** p50, p90, p99 and max of numbers (nulls left out), with how many were measured. */
export function distribution(values) {
  const sorted = values.filter((v) => typeof v === "number").sort((a, b) => a - b);
  return { n: sorted.length, p50: quantile(sorted, 0.5), p90: quantile(sorted, 0.9), p99: quantile(sorted, 0.99), max: sorted.length ? sorted[sorted.length - 1] : null, total: sorted.reduce((a, b) => a + b, 0) };
}

export const cellKey = (line) => `${line.code}|${line.context}|${line.layer}|${line.variant ?? "-"}`;
const mb = (kb) => (typeof kb === "number" ? Math.round((kb / 1024) * 10) / 10 : null);

/**
 * The summary of a grid's cost lines: the checker's wall, CPU and RSS distributions, the slowest cells by mean checker time (with
 * their share of the grid's checker time), every program over `budget` (wall or RSS), and the role-log decisions when the lines have them.
 */
export function costSummary(lines, budget = DEFAULT_BUDGET, { slowest = SLOWEST } = {}) {
  const wall = distribution(lines.map((line) => line.checkerMs));
  const cells = new Map();
  for (const line of lines) {
    if (typeof line.checkerMs !== "number") continue;
    const cell = cells.get(cellKey(line)) ?? { code: line.code, context: line.context, layer: line.layer, variant: line.variant ?? null, programs: 0, ms: 0, maxMs: 0 };
    cell.programs += 1;
    cell.ms += line.checkerMs;
    cell.maxMs = Math.max(cell.maxMs, line.checkerMs);
    cells.set(cellKey(line), cell);
  }
  const ranked = [...cells.values()].map((cell) => ({ ...cell, meanMs: Math.round(cell.ms / cell.programs), share: wall.total ? Math.round((cell.ms / wall.total) * 10000) / 10000 : 0 }))
    .sort((a, b) => b.meanMs - a.meanMs || b.maxMs - a.maxMs || cellKey(a).localeCompare(cellKey(b)));
  const over = lines.filter((line) => (line.checkerMs ?? 0) > budget.ms || (line.maxRssKb ?? 0) > budget.mb * 1024)
    .map((line) => ({ id: line.id, cell: cellKey(line), checkerMs: line.checkerMs, maxRssMb: mb(line.maxRssKb), ...(line.decisions !== undefined ? { decisions: line.decisions } : {}) }))
    .sort((a, b) => (b.checkerMs ?? 0) - (a.checkerMs ?? 0) || a.id.localeCompare(b.id));
  const unmeasured = {};
  for (const line of lines) if (line.why) unmeasured[line.why] = (unmeasured[line.why] ?? 0) + 1;
  const decided = lines.filter((line) => typeof line.decisions === "number");
  return {
    budget, programs: lines.length,
    checkerMs: wall,
    cpuMs: distribution(lines.map((line) => line.cpuMs)),
    maxRssMb: (({ total, ...rest }) => Object.fromEntries(Object.entries(rest).map(([k, v]) => [k, k === "n" ? v : mb(v)])))(distribution(lines.map((line) => line.maxRssKb))),
    checkerWriteMiB: Math.round((lines.reduce((a, line) => a + (line.writeKb ?? 0), 0) / 1024) * 100) / 100,
    decisions: decided.length ? distribution(decided.map((line) => line.decisions)) : null,
    unmeasured,
    slowestCells: ranked.slice(0, slowest),
    overBudget: over,
  };
}

/** The summary as log lines (the grids print them at their end). */
export function costLines(summary, { prefix = "cost" } = {}) {
  const { checkerMs: w, maxRssMb: r } = summary;
  const lines = [`${prefix}: checker ${w.n}/${summary.programs} programs timed: p50 ${w.p50 ?? "-"} ms, p90 ${w.p90 ?? "-"}, p99 ${w.p99 ?? "-"}, max ${w.max ?? "-"}; peak RSS ${r.n ? `p50 ${r.p50} MB, max ${r.max} MB` : "not measured"}${summary.decisions ? `; role decisions p50 ${summary.decisions.p50}, max ${summary.decisions.max}` : ""}; ${summary.overBudget.length} over budget (${summary.budget.ms} ms or ${summary.budget.mb} MB)`];
  for (const [why, n] of Object.entries(summary.unmeasured)) lines.push(`${prefix}:   ${n} not fully measured: ${why}`);
  for (const cell of summary.slowestCells.slice(0, 5)) lines.push(`${prefix}:   slow cell ${cell.code} ${cell.context} ${cell.layer} ${cell.variant ?? "-"}: mean ${cell.meanMs} ms, max ${cell.maxMs} ms over ${cell.programs} (${(cell.share * 100).toFixed(2)}% of checker time)`);
  for (const program of summary.overBudget.slice(0, 10)) lines.push(`${prefix}:   over budget ${program.id}: ${program.checkerMs ?? "-"} ms, ${program.maxRssMb ?? "-"} MB`);
  if (summary.overBudget.length > 10) lines.push(`${prefix}:   ... ${summary.overBudget.length - 10} more over budget (cost.json)`);
  return lines;
}

export const readCostLines = (dir) => (existsSync(join(dir, "costs.jsonl")) ? readFileSync(join(dir, "costs.jsonl"), "utf8").split("\n").filter(Boolean).map((line) => JSON.parse(line)) : []);

/** Writes a grid's `costs.jsonl` and `cost.json`; returns the summary. */
export function writeCosts(out, lines, budget = DEFAULT_BUDGET) {
  writeFileSync(join(out, "costs.jsonl"), lines.length ? `${lines.map((line) => JSON.stringify(line)).join("\n")}\n` : "");
  const summary = costSummary(lines, budget);
  writeFileSync(join(out, "cost.json"), `${JSON.stringify(summary, null, 2)}\n`);
  return summary;
}

/**
 * Adds the role log's decisions per program (`perProgram`: program -> decisions, roles.mjs `foldDecisions`) to a grid's cost lines and
 * rewrites its summary. A program's decisions are its own and its instrumented twin's (`<id>-run`); a program the log never names has
 * none recorded, 0 (the log was on and the checker made no decision there).
 */
export function addDecisions(dir, perProgram) {
  const lines = readCostLines(dir);
  if (!lines.length) return null;
  for (const line of lines) line.decisions = (perProgram.get(line.id) ?? 0) + (perProgram.get(`${line.id}-run`) ?? 0);
  const budget = existsSync(join(dir, "cost.json")) ? JSON.parse(readFileSync(join(dir, "cost.json"), "utf8")).budget ?? DEFAULT_BUDGET : DEFAULT_BUDGET;
  return writeCosts(dir, lines, budget);
}
