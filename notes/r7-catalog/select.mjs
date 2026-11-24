// R7 (rebuilt from SIMPLE-UNPROVEN.md §1, origin/claude/r3-catalog 4e8216aae): the SIMPLE programs of every focus grid.
//   node select.mjs <clone> <out dir>
// Shape: binding direct/alias/alias-const/namespace; layer none or closure (recall grids: none only, and no context matching
// patch/cast/recv/flw/impostor/eval/proxy/reflect/proto); mount none or child/show/for/show-signal/for-item/show-later; no file
// source (a module or package source; prop and context sources are in-file, R4's src grids); callback form none or const: the population,
// <out>/<grid>.json (what is run). Text: one source file App.tsx and no generated package, at most 30 non-blank lines, none of the adversarial
// tokens below: SIMPLE, <out>/<grid>.simple.json (what a U is counted in).
import { writeFileSync, mkdirSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
const [clone, out] = process.argv.slice(2);
const imp = (f) => import(pathToFileURL(join(clone, "scripts/fuzz", f)).href);
const { gridRows } = await imp("cover.mjs");
const { GRIDS } = await imp("focus-grids.mjs");
const { buildRow } = await imp("blocker-scan.mjs");
const { GENERATOR_VERSION } = await imp("generate.mjs");
mkdirSync(out, { recursive: true });
const BINDINGS = new Set(["direct", "alias", "alias-const", "namespace"]);
const MOUNTS = new Set(["undefined", "child", "show", "for", "show-signal", "for-item", "show-later"]);
const RECALL_CONTEXT = /patch|cast|recv|flw|impostor|eval|proxy|reflect|proto/;
const TOKENS = [
  /prototype|__proto__|setPrototypeOf|Object\.create/, /Reflect\.|\bProxy\b/, /defineProperty|Object\.assign\(/, /\beval\b|new Function/,
  /toString|valueOf|Symbol\.|toJSON/, /\bget\s+\w+\s*\(|\bset\s+\w+\s*\(/, /^(?!\s*import\b).*\bas\s+(?!const\b)[A-Za-z_$[{(]/m, /\.call\(|\.apply\(|\.bind\(/,
  /\b(window|document)\.\w+\s*=[^=]|\.(innerHTML|onclick|textContent|value)\s*=[^=]/,
  /dispatchEvent|new \w*Event\b|addEventListener|\.click\(\)|\.focus\(\)|createElement|DOMParser|srcdoc|javascript:|\.attributes\b/,
  /JSON\./, /solid-js\/internal|createOwner/, /\bclass\s+\w+|function\s*\*|\byield\b/, /\bwith\s*\(/, /\w\[[^\]]+\]\s*\(/,
];
const reasons = {};
const bump = (k) => { reasons[k] = (reasons[k] ?? 0) + 1; };
let all = 0;
let pop = 0;
for (const grid of Object.keys(GRIDS)) {
  const recall = grid.endsWith("-recall");
  const ids = [];
  const population = [];
  let n = 0;
  for (const row of gridRows(grid, {})) {
    n += 1;
    if (!BINDINGS.has(row.binding) || !(recall ? row.layer === "none" : ["none", "closure"].includes(row.layer))
      || !MOUNTS.has(String(row.mount)) || /^(module|package)/.test(row.source ?? "") || !(row.callback === undefined || row.callback === "const")
      || (recall && RECALL_CONTEXT.test(row.context ?? ""))) { bump("shape"); continue; }
    population.push(row.id);
    let built;
    try { built = buildRow(row, "1"); } catch { bump("build-error"); continue; }
    const sources = Object.keys(built.files ?? {}).filter((file) => /\.[cm]?[jt]sx?$/.test(file));
    if (sources.length !== 1 || sources[0] !== "App.tsx" || (built.packages ?? []).length) { bump("files-or-package"); continue; }
    const text = built.files["App.tsx"];
    if (text.split("\n").filter((line) => line.trim()).length > 30) { bump("length"); continue; }
    if (TOKENS.some((token) => token.test(text))) { bump("token"); continue; }
    ids.push(row.id);
  }
  all += ids.length;
  writeFileSync(join(out, `${grid}.json`), JSON.stringify({ generator: GENERATOR_VERSION, seed: 1, ids: population }));
  writeFileSync(join(out, `${grid}.simple.json`), JSON.stringify({ generator: GENERATOR_VERSION, seed: 1, ids }));
  pop += population.length;
  console.log(grid, n, "rows,", population.length, "by shape,", ids.length, "simple by text");
}
console.log("total by shape", pop, "simple by text", all, JSON.stringify(reasons));
