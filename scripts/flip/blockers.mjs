// Every uncertifiable row of a sweep names its blocker (ADR 0680): node scripts/flip/blockers.mjs <sweep.json>. Exits 1 on a row that does not.
import { readFileSync } from "node:fs";

const named = (context) => /^[a-z0-9]+(-[a-z0-9]+)+(:.*)?$/s.test(context ?? "") && !String(context).startsWith("proof-step-unnamed");
const sweep = JSON.parse(readFileSync(process.argv[2], "utf8"));
const counts = new Map();
const missing = new Map();
let rows = 0;
for (const result of sweep.results) {
  for (const finding of result.findings ?? []) {
    if (finding.kind !== "uncertifiable") continue;
    rows += 1;
    const key = `${finding.id} ${finding.analysisContext ?? ""}`;
    counts.set(key, (counts.get(key) ?? 0) + 1);
    if (!named(finding.analysisContext)) missing.set(key, (missing.get(key) ?? 0) + 1);
  }
}
if (process.argv.includes("--all")) for (const [key, count] of [...counts].sort((a, b) => b[1] - a[1])) console.log(count, key);
console.log(`${rows} uncertifiable rows, ${[...missing.values()].reduce((a, b) => a + b, 0)} with no named blocker`);
for (const [key, count] of [...missing].sort((a, b) => b[1] - a[1]).slice(0, 40)) console.log(" ", count, key);
process.exit(missing.size ? 1 : 0);
