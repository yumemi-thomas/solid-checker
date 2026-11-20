// Compares the dev census with the production-build census: per project the writer roots, their causes and the rows.
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
const S = process.env.RUNS ?? "/Users/thomas/dev/sc-scratch.noindex/mac20b/runs"; // the runs directory (RUNS=<scratch>/runs)
const load = (dir) => new Map(readdirSync(join(S, dir)).filter((id) => !id.startsWith(".")).map((id) => [id, { meta: JSON.parse(readFileSync(join(S, dir, id, "meta.json"), "utf8")), census: JSON.parse(readFileSync(join(S, dir, id, "census.json"), "utf8")) }]));
const dev = load("census-v2");
const std = load("census-std");
let same = 0;
const lines = [];
for (const [id, a] of dev) {
  const b = std.get(id);
  const sig = (c) => JSON.stringify(c.writers.map((w) => [w.root, w.loaded, w.counted, w.causes.map((x) => `${x.file}|${x.class}|${x.n}`).sort()]).sort());
  const identical = b && sig(a.census) === sig(b.census);
  if (identical) same += 1;
  else lines.push(`${id}: dev ${a.census.writers.length} writers ${a.meta.family3} rows; std ${b ? b.census.writers.length : "?"} writers ${b ? b.meta.family3 : "?"} rows`);
}
const rowsDev = [...dev.values()].reduce((sum, p) => sum + p.meta.family3, 0);
const rowsStd = [...std.values()].reduce((sum, p) => sum + p.meta.family3, 0);
console.log(`production build against development: ${same} of ${dev.size} projects have the same writer roots, flags and causes; family-3 rows ${rowsDev} (dev) and ${rowsStd} (production)`);
for (const line of lines.slice(0, 12)) console.log(`  ${line}`);
