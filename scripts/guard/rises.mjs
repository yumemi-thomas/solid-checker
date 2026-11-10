// node scripts/guard/rises.mjs [--append "ADR 0955" --fact "<the typed fact>" [--why "<text>"]]: the rises of the uncertifiable count, named (ADR 0955).
//
// scripts/guard/count.mjs holds the count one way: a fall needs a proof, a rise is recorded with --update and says nothing of itself. A veto or
// a fail-closed change raises it on purpose, on fixtures that already exist, and the rise should say which fact did it. This lists, per
// EXISTING fixture (a snapshot the base also has: the coverage snapshots and the reproduction set's), each `code blocker` key whose
// uncertifiable rows rose against the base (count.mjs's base: COUNT_BASE, else the merge base with origin/main), and checks them against
// scripts/guard/rises.json:
// - a rise is NAMED when a row { fixture, key, from, to, adr, fact, why? } says it (`fact`: the hazards, formula or missing fact, as
//   `scripts/u-blockers.mjs` names them); an unnamed rise is printed, never a failure: a rise is the purpose of a fail-closed change, and
//   a guard that blocked it would push the change toward silence;
// - `rises` is append-only against the base, and every row is well formed: the ADR is in docs/decisions.md, `to` is above `from`, fixture,
//   key and fact are text. Those are the failures (exit 1);
// - `--append` adds a row for every unnamed rise (the ADR and the fact are asked for; one fact for the whole change).
// counts.json is not touched and not rekeyed: its keys stay `code blocker`.
import { existsSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { baseLedger, countRows, fixtureOf, git, snapshotsAt } from "./count.mjs";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const RISES = "scripts/guard/rises.json";
const SNAPSHOTS = "fixtures/findings-snapshots";

const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);

/** The rises: `fixture -> snapshot` maps at the base and now; the fixtures both have, per `code blocker` key whose count went up. */
export function measureRises(baseSnapshots, snapshots) {
  const rises = [];
  for (const [fixture, before] of [...baseSnapshots].sort(([a], [b]) => (a < b ? -1 : 1))) {
    const now = snapshots.get(fixture);
    if (!now) continue;
    const was = countRows([before]);
    const is = countRows([now]);
    for (const key of Object.keys(is).sort()) if ((is[key] ?? 0) > (was[key] ?? 0)) rises.push({ fixture, key, from: was[key] ?? 0, to: is[key] });
  }
  return rises;
}

/** Whether a row of the list says `rise`. */
export const names = (row, rise) => row.fixture === rise.fixture && row.key === rise.key && row.from === rise.from && row.to === rise.to;

/** The failures (sentences), the measured rises that a row names, and those none does. `base` is the list at the base, or null. */
export function checkRises({ ledger, base, measured, decisions }) {
  const failures = [];
  const rows = ledger.rises ?? [];
  for (const row of base?.rises ?? []) if (!rows.some((now) => same(now, row))) failures.push(`rises is append-only: a row at the base is gone or changed (${row.fixture} ${row.key} ${row.from} -> ${row.to})`);
  rows.forEach((row, index) => {
    const label = `rises[${index}] ${row.fixture ?? "(no fixture)"} ${row.key ?? "(no key)"}`;
    for (const field of ["fixture", "key", "fact"]) if (typeof row[field] !== "string" || row[field].trim() === "") failures.push(`${label}: ${field} is empty`);
    if (!Number.isInteger(row.from) || !Number.isInteger(row.to) || !(row.to > row.from) || row.from < 0) failures.push(`${label}: from and to are counts and to is above from (${row.from} -> ${row.to})`);
    const number = /^ADR (\d{4})$/.exec(row.adr ?? "")?.[1];
    if (!number || !decisions.includes(`<a id="adr-${number}"></a>`)) failures.push(`${label}: ${row.adr ?? "no ADR"} is not in docs/decisions.md`);
  });
  return { failures, named: measured.filter((rise) => rows.some((row) => names(row, rise))), unnamed: measured.filter((rise) => !rows.some((row) => names(row, rise))) };
}

const readSnapshots = () => new Map(readdirSync(join(repo, SNAPSHOTS)).filter((name) => name.endsWith(".json")).map((name) => [fixtureOf(name), JSON.parse(readFileSync(join(repo, SNAPSHOTS, name), "utf8"))]));

/** The text of `--append`'s rows for the unnamed rises. */
export const rowsFor = (unnamed, { adr, fact, why }) => unnamed.map((rise) => ({ ...rise, adr, fact, ...(why ? { why } : {}) }));

function main(argv) {
  const value = (name) => { const at = argv.indexOf(name); return at < 0 ? undefined : argv[at + 1]; };
  const path = join(repo, RISES);
  const ledger = existsSync(path) ? JSON.parse(readFileSync(path, "utf8")) : { about: "", rises: [] };
  const { revision, ledger: baseCounts } = baseLedger();
  const baseSnapshots = baseCounts && !revision.includes(",") ? snapshotsAt(revision) : null;
  const baseText = baseSnapshots ? git("show", `${revision}:${RISES}`) : null;
  const base = baseText ? JSON.parse(baseText) : null;
  const measured = baseSnapshots ? measureRises(baseSnapshots, readSnapshots()) : [];
  const decisions = readFileSync(join(repo, "docs/decisions.md"), "utf8");
  const { failures, named, unnamed } = checkRises({ ledger, base: base ?? { rises: [] }, measured, decisions });
  if (argv.includes("--append")) {
    const adr = value("--append");
    const fact = value("--fact");
    if (!adr || !fact) throw new Error('--append "ADR NNNN" --fact "<the typed fact>" [--why "<text>"]');
    const added = rowsFor(unnamed, { adr, fact, why: value("--why") });
    writeFileSync(path, `${JSON.stringify({ about: ledger.about, rises: [...(ledger.rises ?? []), ...added] }, null, 1)}\n`);
    console.log(`${RISES}: ${added.length} rows appended`);
    return;
  }
  for (const failure of failures) console.error(failure);
  for (const rise of unnamed) console.log(`unnamed rise ${rise.fixture} ${rise.key}: ${rise.from} -> ${rise.to} (name it: node scripts/guard/rises.mjs --append "ADR NNNN" --fact "...")`);
  console.log(`rises: ${measured.length} measured against ${baseSnapshots ? `base ${revision}` : `(no snapshots at base ${revision}, none compared)`}, ${named.length} named, ${unnamed.length} unnamed (never a failure); ${(ledger.rises ?? []).length} rows in ${RISES}`);
  process.exitCode = failures.length ? 1 : 0;
}

if (import.meta.main ?? process.argv[1] === fileURLToPath(import.meta.url)) main(process.argv.slice(2));
