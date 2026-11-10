// Re-measures the primitive-layer table (ADR 0940, primitive-layer-shapes.mjs `PL_MEASURED`) from a probe run, so a Solid bump or a new layer is a re-run, not a guess.
//
//   node scripts/fuzz/primitive-layer-scan.mjs probe                     the command that runs the probe (every pl-* context a cell of every PL code, both sides)
//   node scripts/fuzz/primitive-layer-scan.mjs table <focus run dir>     the `PL_MEASURED` text the run measured, with the pairs it could not place
//   node scripts/fuzz/primitive-layer-scan.mjs check <focus run dir>     exit 1 when the run's table is not the module's
//
// A program of the probe is read for what the runtime did, never for what the checker said:
//   misuse   the matrix judged it runtime-confirmed (`agree`, `recall-gap`, `false-clean`, `wrong-rule`, `misplaced`): the code is reported at the site;
//   legal    the matrix found the code not fired, the legal grid found the site ran and the program clean (no violation, an obligation, or a violation the runtime did not
//            confirm), and the run reported no defect code at all (a legal program reports nothing; codes.mjs `policyOf`);
//   none     anything else: another code is reported, the program halted, the site never ran.
// A (code, context) is a misuse or a legal cell when its variants of that kind agree; a variant of the other kind (or `none`) is listed in `without`. A cell whose variants are
// of both sides cannot be one list and is reported unresolved.
import { existsSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { policyOf } from "./codes.mjs";
import { SPEC } from "./misuse.mjs";
import { PL_CODES, PL_CONTEXT_NAMES, PL_HOSTS, PL_HOSTS_OF, PL_LAYERS, PL_MEASURED, plParts } from "./primitive-layer-shapes.mjs";

const CONFIRMED = new Set(["agree", "recall-gap", "false-clean", "wrong-rule", "misplaced"]);
const LEGAL_LG = new Set(["clean", "uncertifiable", "false-positive"]);
const NOT_CONFIRMED = new Set(["not-fired", "stacked-claim", "false-positive"]);
const read = (path) => readFileSync(path, "utf8").split("\n").filter(Boolean).map((line) => JSON.parse(line));
const keyOf = (row) => `${row.code}\t${row.context}\t${row.variant ?? "-"}`;

/** What one program of the probe was: `misuse`, `legal` or `none` (see above). */
export function kindOf(mxRow, lgRow) {
  if (CONFIRMED.has(mxRow.outcome)) return "misuse";
  const reported = (mxRow.detail?.reported ?? []).filter((code) => policyOf(code)?.disposition?.startsWith("defect"));
  return NOT_CONFIRMED.has(mxRow.outcome) && lgRow && LEGAL_LG.has(lgRow.outcome) && reported.length === 0 ? "legal" : "none";
}

/**
 * The table of a probe: `{ table, unresolved }`, `table` shaped as `PL_MEASURED` (`{ code: { misuse: { host: [layer] }, legal: {...}, without?: { host: { layer: [variant] } } } }`).
 * @param {object[]} mx the mx-recall grid's rows (programs.jsonl); @param {object[]} lg the lg-recall grid's
 */
export function measuredTable(mx, lg) {
  const lgBy = new Map(lg.map((row) => [keyOf(row), row]));
  const cells = new Map();
  for (const row of mx) {
    if (!plParts(row.context)) continue;
    const cell = `${row.code}\t${row.context}`;
    if (!cells.has(cell)) cells.set(cell, []);
    cells.get(cell).push({ variant: row.variant ?? "-", kind: kindOf(row, lgBy.get(keyOf(row))) });
  }
  const table = {};
  const unresolved = [];
  for (const [cell, list] of cells) {
    const [code, context] = cell.split("\t");
    const { layer, host } = plParts(context);
    if (PL_HOSTS_OF[code] && !PL_HOSTS_OF[code].includes(host)) continue;
    // The cell's side is the base variants' (the `plVariants` are beside them, and take the side only where the base variants have none): a variant of another
    // side is left out of the cell (`without`), where it is a cell of the code whose own site it is (`resolve` under `isPending` is RESOLVE_IN_REACTIVE_SCOPE's legal cell).
    const extra = new Set(SPEC[code]?.plVariants ?? []);
    const sideOf = (entries) => { const kinds = new Set(entries.map((entry) => entry.kind).filter((kind) => kind !== "none")); return kinds.size > 1 ? "both" : kinds.size ? [...kinds][0] : null; };
    const side = sideOf(list.filter((entry) => !extra.has(entry.variant))) ?? sideOf(list);
    if (side === null) continue;
    if (side === "both") { unresolved.push(`${code} ${context}: ${list.map((entry) => `${entry.variant}=${entry.kind}`).join(" ")}`); continue; }
    const kind = side;
    const entry = (table[code] ??= { misuse: {}, legal: {} });
    (entry[kind][host] ??= []).push(layer);
    const without = list.filter((one) => one.kind !== kind).map((one) => one.variant);
    if (without.length) ((entry.without ??= {})[host] ??= {})[layer] = without;
  }
  for (const entry of Object.values(table)) for (const kind of ["misuse", "legal"]) for (const layers of Object.values(entry[kind])) layers.sort((a, b) => PL_LAYERS.indexOf(a) - PL_LAYERS.indexOf(b));
  return { table, unresolved };
}

/** The table as the text of `PL_MEASURED`, hosts and layers in the module's order. */
export function tableText(table) {
  const hosts = (byHost) => Object.keys(byHost).sort((a, b) => PL_HOSTS.indexOf(a) - PL_HOSTS.indexOf(b));
  let out = "export const PL_MEASURED = {\n";
  for (const code of PL_CODES) {
    const entry = table[code];
    if (!entry) continue;
    out += `  ${code}: {\n`;
    for (const kind of ["misuse", "legal"]) {
      out += `    ${kind}: {\n`;
      for (const host of hosts(entry[kind])) out += `      ${JSON.stringify(host)}: ${JSON.stringify(entry[kind][host]).replace(/","/g, '", "')},\n`;
      out += "    },\n";
    }
    if (entry.without) {
      out += "    without: {\n";
      for (const host of hosts(entry.without)) out += `      ${JSON.stringify(host)}: ${JSON.stringify(entry.without[host])},\n`;
      out += "    },\n";
    }
    out += "  },\n";
  }
  return `${out}};\n`;
}

/** The focus command that runs the probe (the module's env switch makes every pl-* context a cell of every PL code, both sides, every variant taken). */
export function probeCommand(out = "<fresh dir>") {
  return `SOLID_CHECKER_FUZZ_PL_PROBE=1 node scripts/fuzz/focus.mjs --codes ${PL_CODES.join(",")} --grids mx-recall,lg-recall --runtime-diff all --out ${out} -- --contexts ${PL_CONTEXT_NAMES.join(",")} --no-chrome-cache`;
}

/** Whether two tables hold the same cells (key order aside). */
export function sameTable(a, b) {
  const norm = (value) => (Array.isArray(value) ? value : value && typeof value === "object" ? Object.fromEntries(Object.keys(value).sort().map((key) => [key, norm(value[key])])) : value);
  return JSON.stringify(norm(a)) === JSON.stringify(norm(b));
}

function main(argv) {
  const [command, dir] = argv;
  if (command === "probe") { console.log(probeCommand(dir)); return 0; }
  if ((command !== "table" && command !== "check") || !dir) { console.error("usage: primitive-layer-scan.mjs probe | table <focus run dir> | check <focus run dir>"); return 2; }
  const grid = (name) => { const path = join(resolve(dir), name, "programs.jsonl"); if (!existsSync(path)) throw new Error(`${path} is missing: run the probe first`); return read(path); };
  const { table, unresolved } = measuredTable(grid("mx-recall"), grid("lg-recall"));
  if (command === "table") console.log(tableText(table));
  for (const line of unresolved) console.error(`unresolved: ${line}`);
  if (command === "check") {
    const same = sameTable(table, PL_MEASURED);
    console.log(same ? "primitive-layer table: the run measured the module's table" : "primitive-layer table: the run measured another table (node scripts/fuzz/primitive-layer-scan.mjs table <dir>)");
    return same && !unresolved.length ? 0 : 1;
  }
  return unresolved.length ? 1 : 0;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) process.exitCode = main(process.argv.slice(2));
