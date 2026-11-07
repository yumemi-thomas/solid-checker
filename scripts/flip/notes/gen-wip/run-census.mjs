// node run-census.mjs <wt> <checker> <contexts> [code[,code]] [id-substring]: check mx-census programs of the cells with one checker.
const [wt, bin, contexts, codes, want] = process.argv.slice(2);
const main = "/Users/thomas.guehenneux/EDF/Yumemi/GitHub/solid-checker";
const m = await import(`${wt}/scripts/fuzz/matrix.mjs`);
const { materializeForChecker, runChecker } = await import(`${wt}/scripts/fuzz/checker.mjs`);
const cells = m.censusCells({ codes: codes ? codes.split(",") : undefined, contexts: contexts.split(",") });
for (const cell of cells) m.censusShapes(cell.code).forEach(async () => {});
for (const cell of cells) {
  const shapes = m.censusShapes(cell.code);
  for (let index = 0; index < shapes.length; index++) {
    const id = m.shapeId(cell, shapes[index]);
    if (want && !id.includes(want)) continue;
    const { built } = m.generateShape({ seed: "1" }, cell, shapes[index], index);
    const work = materializeForChecker(built, `${main}/scripts/runtime-oracle-install`, `${built.id}-${process.pid}`);
    const r = await runChecker(work, { bin, typefacts: `${wt}/bin/solid-typefacts` });
    const sites = built.sites?.map?.((s) => s.line) ?? [];
    console.log(`${id} site=${sites.join(",")} ${r.ok ? r.findings.map((f) => `${f.rule}/${f.kind}@${f.line}${f.blocker || f.analysisContext ? "[" + (f.blocker ?? f.analysisContext).split(":")[0] + "]" : ""}`).join(" ") || "(none)" : "ERROR " + r.error}`);
  }
}
