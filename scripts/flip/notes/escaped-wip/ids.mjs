// ids.mjs <repo> <outdir>: id manifests (smoke.mjs keepIds format) of the mx-base programs whose variant is an ADR 0860 `-apart` shape,
// for STRICT_READ_UNTRACKED and STORE_WRITE_OUTSIDE_SETTER.
import { writeFileSync, mkdirSync } from "node:fs";
const [repo, out] = process.argv.slice(2);
const m = await import(`${repo}/scripts/fuzz/matrix.mjs`);
const { GENERATOR_VERSION } = await import(`${repo}/scripts/fuzz/generate.mjs`);
mkdirSync(out, { recursive: true });
const codes = ["STRICT_READ_UNTRACKED", "STORE_WRITE_OUTSIDE_SETTER"];
const ids = [];
for (const cell of m.cellList({ codes })) for (const shape of m.gridShapes(cell.code, cell.context)) if (String(shape.variant).endsWith("-apart")) ids.push(m.shapeId(cell, shape));
writeFileSync(`${out}/mx-base.json`, JSON.stringify({ grid: "mx-base", generator: GENERATOR_VERSION, seed: 1, ids }, null, 1));
console.log(ids.length, ids.slice(0, 40).join("\n"));
