// node arms-text.mjs <wt> <checker> <file.tsx> <line,...>: role-log decisions on the given lines of one App.tsx.
import { readFileSync, rmSync, existsSync } from "node:fs";
const [wt, bin, file, lines] = process.argv.slice(2);
const main = "/Users/thomas.guehenneux/EDF/Yumemi/GitHub/solid-checker";
const { materializeForChecker, runChecker } = await import(`${wt}/scripts/fuzz/checker.mjs`);
const text = readFileSync(file, "utf8");
const built = { id: "probe-" + process.pid, files: { "App.tsx": text, "App.run.txt": text } };
const log = `/Users/thomas.guehenneux/EDF/tmp/flip-gen-await/rolelog-${process.pid}.jsonl`;
if (existsSync(log)) rmSync(log);
process.env.SOLID_CHECKER_IR_ROLE_LOG = log;
const work = materializeForChecker(built, `${main}/scripts/runtime-oracle-install`, built.id);
const r = await runChecker(work, { bin, typefacts: `${wt}/bin/solid-typefacts` });
const want = lines.split(",").map(Number);
const line = (b) => text.slice(0, b).split("\n").length;
console.log(`findings: ${r.ok ? r.findings.map((f) => `${f.rule}/${f.kind}@${f.line}${f.blocker ? "[" + f.blocker + "]" : ""}`).join(" ") : "ERROR " + r.error}`);
const paths = {};
for (const l of existsSync(log) ? readFileSync(log, "utf8").trim().split("\n") : []) {
  const d = JSON.parse(l);
  if (d.k === "path") { paths[d.p] = d.f; continue; }
  const f = d.f ?? paths[d.p];
  if (!f || !f.endsWith("App.tsx") || !want.includes(line(d.s))) continue;
  console.log(`  L${line(d.s)} ${d.k} ${d.rule ?? ""} role=${d.role ?? ""} ctx=${d.ctx ?? ""} arm=${d.arm} arms=${(d.arms ?? []).join(">")}`);
}
