const wt = process.argv[2];
const m = await import(`${wt}/scripts/fuzz/matrix.mjs`);
const f = await import(`${wt}/scripts/fuzz/focus.mjs`);
console.log("mx-census args", JSON.stringify(f.GRIDS["mx-census"]));
const cells = m.censusCells({ contexts: ["gen-yield", "gen-sync-loop"] });
let n = 0; const by = {};
for (const c of cells) { const k = m.censusShapes(c.code).length; n += k; by[c.code] = (by[c.code] ?? 0) + k; }
console.log(cells.length, "cells", n, "programs"); console.log(by);
const cells2 = m.cellList({ contexts: ["gen-yield", "gen-sync-loop"], recall: true });
console.log("recall cells", [...new Set(cells2.map(c=>c.code))].length, cells2.map(c=>`${c.code}/${c.context}`).length);
