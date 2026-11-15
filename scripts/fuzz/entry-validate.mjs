// Validates the named entry (checker.mjs `entryOverride`, ADR 0990) in real Chrome: the same grid programs, run through the real oracle (scripts/runtime-oracle.mjs, `feedback run`)
// once with the oracle's namespace-loop entry and once with the named one (the case's `main`), and their Chrome records compared. The named entry reuses the grids' Chrome
// records, which holds only if it runs the same program; a record that differs stops the work.
//
//   node scripts/fuzz/entry-validate.mjs --out <fresh dir> --checker-bin <solid-checker-rust> --typefacts <solid-typefacts> [--per 2] [--concurrency 4] [--families ph,er,...]
//
// The sample: per family and side (misuse, legal) `--per` cells (the ones whose id hashes smallest, so the pick is the same every run), each at its first plain shape, as the grid
// (seed 1) generates it. Prints one line per program (Chrome's record equal or not; what the oracle's checker said under each entry) and exits 1 on any difference of record.
import { cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { recordDigest } from "./cache.mjs";
import { runChrome } from "./chrome.mjs";
import { writeCase } from "./emit.mjs";
import { namedEntry } from "./checker.mjs";
import { generateLegal, recallLegalShapes, legalCells } from "./legal.mjs";
import { cellList, generateShape, recallShapes } from "./matrix.mjs";
import { fnv } from "./rng.mjs";
import { PH_CONTEXT_NAMES } from "./ph-shapes.mjs";
import { ER_CONTEXT_NAMES } from "./er-shapes.mjs";
import { EM_CONTEXT_NAMES } from "./em-shapes.mjs";
import { OB_CONTEXT_NAMES } from "./ob-shapes.mjs";
import { LA_CONTEXT_NAMES } from "./la-shapes.mjs";
import { SF_CONTEXT_NAMES } from "./sf-shapes.mjs";
import { RV_CONTEXT_NAMES } from "./rv-shapes.mjs";
import { CS_CONTEXT_NAMES } from "./cs-shapes.mjs";
import { PA_CONTEXT_NAMES } from "./pa-shapes.mjs";
import { FC_CONTEXT_NAMES } from "./fc-shapes.mjs";

/** The closure-sensitive families of the boundary twin, by the contexts each names. */
export const FAMILIES = { ph: PH_CONTEXT_NAMES, er: ER_CONTEXT_NAMES, em: EM_CONTEXT_NAMES, ob: OB_CONTEXT_NAMES, la: LA_CONTEXT_NAMES, sf: SF_CONTEXT_NAMES, rs: RV_CONTEXT_NAMES, cs: CS_CONTEXT_NAMES, pa: PA_CONTEXT_NAMES, fc: FC_CONTEXT_NAMES };

/** The sample: `{ family, side, code, context, shape, built }` for `per` cells of each family and side. */
export function sample({ per = 2, families = Object.keys(FAMILIES), seed = 1 } = {}) {
  const out = [];
  for (const family of families) {
    const names = new Set(FAMILIES[family]);
    for (const side of ["misuse", "legal"]) {
      const cells = (side === "misuse" ? cellList({ recall: true }) : legalCells({ recall: true })).filter((cell) => names.has(cell.context));
      const chosen = [...cells].sort((a, b) => fnv(`${a.code}/${a.context}`) - fnv(`${b.code}/${b.context}`));
      const seen = new Set();
      for (const cell of chosen) {
        if (seen.size >= per) break;
        // A different context each, so the sample spreads over the family's routes.
        if (seen.has(cell.context)) continue;
        const shapes = (side === "misuse" ? recallShapes : recallLegalShapes)(cell.code, cell.context);
        const shape = shapes.find((one) => one.binding === "direct" && (one.layer ?? "none") === "none") ?? shapes[0];
        if (!shape) continue;
        seen.add(cell.context);
        const made = side === "legal" ? generateLegal({ seed }, cell, { index: 0, shape }) : generateShape({ seed }, cell, shape, 0);
        out.push({ family, side, code: cell.code, context: cell.context, built: made.built });
      }
    }
  }
  return out;
}

/** The case directory of a program: its files, and the named entry in `oracle.json` and `oracle.run.json` when `named`. */
export function stage(root, entry, named) {
  const id = `${entry.built.id}${named ? "~named" : ""}`;
  const dir = writeCase(join(root, id), entry.built);
  if (named) {
    const text = namedEntry(readFileSync(join(dir, "oracle.json"), "utf8"), entry.built.files["App.tsx"]);
    if (text === null) return { id, dir, skipped: "the named entry cannot be derived for this program" };
    for (const name of ["oracle.json", "oracle.run.json"]) {
      const spec = JSON.parse(readFileSync(join(dir, name), "utf8"));
      writeFileSync(join(dir, name), `${JSON.stringify({ ...spec, main: text }, null, 2)}\n`);
    }
  }
  return { id, dir, sites: entry.built.sites };
}

const checkerKeys = (record) => (record.checker?.findings ?? []).map((finding) => `${finding.rule}/${finding.kind}@${finding.line}`).sort().join(" ") || "-";

async function main(argv) {
  const option = (name, fallback) => { const at = argv.indexOf(`--${name}`); return at < 0 ? fallback : argv[at + 1]; };
  const out = resolve(option("out", ""));
  if (!option("out") || existsSync(out)) throw new Error("entry-validate.mjs: --out <fresh dir>");
  mkdirSync(out, { recursive: true });
  const bin = resolve(option("checker-bin")), typefacts = resolve(option("typefacts"));
  const programs = sample({ per: Number(option("per", 2)), families: option("families") ? option("families").split(",") : undefined });
  const staging = join(out, "stage");
  const staged = programs.map((entry) => ({ entry, plain: stage(staging, entry, false), named: stage(staging, entry, true) }));
  const run = (cases, name) => runChrome(cases.map(({ id, dir, sites }) => ({ id, dir, sites })), { out: join(out, name), bin, typefacts, concurrency: Number(option("concurrency", 4)) });
  mkdirSync(join(out, "plain"), { recursive: true });
  mkdirSync(join(out, "named"), { recursive: true });
  const derivable = staged.filter(({ named }) => !named.skipped);
  const plainRecords = await run(derivable.map(({ plain }) => plain), "plain");
  const namedRecords = await run(derivable.map(({ named }) => named), "named");
  let different = 0;
  const rows = [];
  for (const { entry, plain, named } of staged) {
    if (named.skipped) { rows.push({ id: entry.built.id, family: entry.family, side: entry.side, skipped: named.skipped }); console.log(`${entry.family} ${entry.side} ${entry.context}: skipped, ${named.skipped}`); continue; }
    const a = plainRecords.get(plain.id), b = namedRecords.get(named.id);
    const same = recordDigest(a.runtime) === recordDigest(b.runtime);
    if (!same) different += 1;
    const row = { id: entry.built.id, family: entry.family, side: entry.side, code: entry.code, context: entry.context, sameRecord: same, plainVerdict: a.row?.verdict, namedVerdict: b.row?.verdict, plainChecker: checkerKeys(a), namedChecker: checkerKeys(b), ...(same ? {} : { plain: recordDigest(a.runtime), named: recordDigest(b.runtime) }) };
    rows.push(row);
    console.log(`${entry.family} ${entry.side} ${entry.context} ${entry.code}: Chrome record ${same ? "equal" : "DIFFERENT"}; oracle verdict ${row.plainVerdict} / ${row.namedVerdict}; checker ${row.plainChecker} / ${row.namedChecker}`);
  }
  writeFileSync(join(out, "entry-validate.json"), `${JSON.stringify(rows, null, 1)}\n`);
  console.log(`\n${rows.length} programs, ${rows.filter((row) => row.sameRecord).length} with equal Chrome records, ${different} different, ${rows.filter((row) => row.skipped).length} skipped (${join(out, "entry-validate.json")})`);
  return different ? 1 : 0;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) process.exitCode = await main(process.argv.slice(2));
