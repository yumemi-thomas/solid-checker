// A deterministic split of a grid's programs across processes (`--grid-shard i/N`), so a full run can be spread over machines:
// shard i of N keeps the programs whose `fnv("shard:<id>") mod N` is i. The N shards of a grid partition it (every program in
// exactly one), the pick depends on the program ids only (not on the grid's order, the seed of a sample or the machine), and the same
// i/N repeats it. Unlike a sample (sample.mjs) the N shards together are the whole grid: a shard's matrix is gated against the same
// shard of the baseline (matrix.mjs `gate`), never against a whole grid or another shard.
import { fnv } from "./rng.mjs";

/** The shard settings from the grids' options (`--grid-shard i/N`), or null. */
export function gridShardOptions(options) {
  if (options.gridShard === undefined) return null;
  const match = /^(\d+)\/(\d+)$/.exec(String(options.gridShard));
  const index = match ? Number(match[1]) : NaN;
  const count = match ? Number(match[2]) : NaN;
  if (!match || !(count >= 1) || !(index < count)) throw new Error(`--grid-shard: i/N with 0 <= i < N, not ${options.gridShard}`);
  if (options.ids) throw new Error("--grid-shard does not apply to --ids (a smoke manifest names its programs)");
  return { index, count };
}

/** The shard of N that a program id falls in. */
export const shardOf = (id, count) => fnv(`shard:${id}`) % count;

/** The entries shard `index` of `count` keeps, in the grid's order, and what it did: `{ entries, summary }` (`gridShard` in the reports). */
export function shardEntries(entries, { index, count }) {
  const kept = entries.filter((entry) => shardOf(entry.built.id, count) === index);
  return { entries: kept, summary: { index, count, kept: kept.length, of: entries.length } };
}

/** One line for a grid's log. */
export const shardLine = (summary) => `shard ${summary.index}/${summary.count}: ${summary.kept} of ${summary.of} programs; one part of the grid`;

/** The key two reports must share for a gate to compare them: the shard, or "whole". */
export const shardKey = (report) => (report.gridShard ? `${report.gridShard.index}/${report.gridShard.count}` : "whole");
