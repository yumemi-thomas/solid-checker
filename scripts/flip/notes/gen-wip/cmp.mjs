// node cmp.mjs <old programs.jsonl> <new programs.jsonl> [all]: outcome/chrome tallies and every moved row; with `all`, every problem row of new.
import { readFileSync } from "node:fs";
const rd = (f) => new Map(readFileSync(f, "utf8").trim().split("\n").map(JSON.parse).map((r) => [r.id, r]));
const [o, n] = [rd(process.argv[2]), rd(process.argv[3])];
const key = (r) => `${r.outcome}/${r.chrome?.status ?? "-"}`;
const t = (m) => { const c = {}; for (const r of m.values()) c[key(r)] = (c[key(r)] || 0) + 1; return c; };
console.log("old", o.size, t(o)); console.log("new", n.size, t(n));
const fs = (r) => JSON.stringify(r?.detail?.findings ?? r?.detail?.offSite ?? []);
for (const [id, r] of n) { const a = o.get(id); if (!a || key(a) !== key(r) || fs(a) !== fs(r)) console.log("moved", id.replace("mx-", ""), a ? key(a) : "-", "->", key(r), fs(a), "->", fs(r)); }
if (process.argv[4]) for (const r of n.values()) if (!["agree/same", "not-fired/same", "recall-gap/same"].includes(key(r))) console.log("problem", key(r), r.id.replace("mx-", ""), fs(r).slice(0, 160));
