const wt = process.argv[2];
const m = await import(`${wt}/scripts/fuzz/matrix.mjs`);
const l = await import(`${wt}/scripts/fuzz/legal.mjs`);
const code = "UNTRACKED_READ_AFTER_AWAIT";
for (const ctx of ["gen-yield", "gen-sync-loop"]) console.log("mx", ctx, m.recallShapes(code, ctx, { lean: true }).length, "base", m.gridShapes(code, ctx).length);
const cells = l.legalCells({ codes: [code] });
console.log("legal cells", cells.map((c) => c.context).join(","));
for (const c of cells) console.log("lg", c.context, l.recallLegalShapes ? l.recallLegalShapes(code, c.context, { lean: true }).length : "?", l.legalShapes(code, c.context).length);
