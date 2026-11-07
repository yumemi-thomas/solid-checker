// node arms.mjs <wt> <checker> <code> <context> <id-substring>: role-log decisions on the site line of one census program.
import { readFileSync, rmSync, existsSync, readdirSync } from "node:fs";
const [wt, bin, code, context, want] = process.argv.slice(2);
const main = "/Users/thomas.guehenneux/EDF/Yumemi/GitHub/solid-checker";
const m = await import(`${wt}/scripts/fuzz/matrix.mjs`);
const { materializeForChecker, runChecker } = await import(`${wt}/scripts/fuzz/checker.mjs`);
const cell = { code, context };
const shapes = m.censusShapes(code);
for (let index = 0; index < shapes.length; index++) {
  const id = m.shapeId(cell, shapes[index]);
  if (!id.includes(want)) continue;
  const { built } = m.generateShape({ seed: "1" }, cell, shapes[index], index);
  const log = `/Users/thomas.guehenneux/EDF/tmp/flip-gen-await/rolelog-${process.pid}.jsonl`;
  if (existsSync(log)) rmSync(log);
  process.env.SOLID_CHECKER_IR_ROLE_LOG = log;
  const work = materializeForChecker(built, `${main}/scripts/runtime-oracle-install`, `${built.id}-${process.pid}`);
  const r = await runChecker(work, { bin, typefacts: `${wt}/bin/solid-typefacts` });
  const text = built.text;
  const line = (b) => text.slice(0, b).split("\n").length;
  const site = built.sites.map((s) => s.line);
  console.log(`== ${id} site=${site} findings: ${r.findings?.map((f) => `${f.rule}/${f.kind}@${f.line}`).join(" ")}`);
  const paths = {};
  for (const l of existsSync(log) ? readFileSync(log, "utf8").trim().split("\n") : []) {
    const d = JSON.parse(l);
    if (d.k === "path") { paths[d.p] = d.f; continue; }
    const f = d.f ?? paths[d.p];
    if (!f || !f.endsWith("App.tsx") || !site.includes(line(d.s))) continue;
    console.log(`  ${d.k} ${d.rule ?? ""} role=${d.role ?? ""} ctx=${d.ctx ?? ""} arm=${d.arm} arms=${(d.arms ?? []).join(">")}`);
  }
}
