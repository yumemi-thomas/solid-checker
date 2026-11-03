// The rows a grid process has judged so far (`programs.partial.jsonl`), appended chunk by chunk while it evaluates, removed once its
// `programs.jsonl` is written. A grid process that dies before then (a crashed host, a killed executor, a refusal) leaves them behind:
// focus.mjs reports the unit as a harness failure with the reason and the rows judged before it stopped, never as `0 programs {}`.
import { appendFileSync, existsSync, readFileSync, rmSync } from "node:fs";
import { join } from "node:path";

export const PARTIAL = "programs.partial.jsonl";

/** A writer of `out`'s partial rows (any earlier run's are removed): `append(rows)` per chunk, `finish()` once programs.jsonl is written. */
export function partialRows(out, { project = (row) => row } = {}) {
  const path = join(out, PARTIAL);
  rmSync(path, { force: true });
  return {
    path,
    append(rows) { if (rows.length) appendFileSync(path, rows.map((row) => JSON.stringify(project(row))).join("\n") + "\n"); },
    finish() { rmSync(path, { force: true }); },
  };
}

/** `out`'s partial rows, or null when there are none; a line cut short by the process's death is dropped. */
export function readPartial(out) {
  const path = join(out, PARTIAL);
  if (!existsSync(path)) return null;
  const rows = [];
  for (const line of readFileSync(path, "utf8").split("\n")) {
    if (!line) continue;
    try { rows.push(JSON.parse(line)); } catch { /* the last line, half written */ }
  }
  return rows;
}

/**
 * Why a grid process stopped, from its log: its own last word (`matrix: stopped: …`, `legal: stopped: …`, the grids' top-level catch),
 * else Node's report of an uncaught error (`Error: …`, an errno such as `EPIPE`), else the log's last line.
 */
export function stopReason(log) {
  const lines = log.split("\n").map((line) => line.trim()).filter(Boolean);
  const own = lines.filter((line) => /^(matrix|legal|ssr): stopped: /.test(line)).at(-1);
  if (own) return own.slice(0, 300);
  const thrown = lines.filter((line) => /^(\w*Error\b|Uncaught\b)|\bE[A-Z]{4,}\b/.test(line)).at(-1);
  return (thrown ?? lines.at(-1) ?? "an empty log").slice(0, 300);
}

/**
 * A grid process's last word on an error nothing caught (an unhandled rejection, a stream error event): `<name>: stopped: …` on stderr
 * (what `stopReason` reads) and exit 2, so the run that judged it sees a failed grid, its reason, and the partial rows.
 */
export function guardGridExit(name) {
  const stop = (error) => {
    try { process.stderr.write(`${name}: stopped: uncaught ${String(error?.stack ?? error).split("\n").slice(0, 4).join(" | ")}\n`); } catch {}
    process.exit(2);
  };
  process.on("uncaughtException", stop);
  process.on("unhandledRejection", stop);
}
