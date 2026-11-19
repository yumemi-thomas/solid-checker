// node scripts/guard/count.mjs [--update [--adr "ADR NNNN"] [--proof file::symbol]]: the one-way uncertifiable count (endstage-report.md 1b).
// Counts the uncertifiable rows of every coverage snapshot by `code blocker` and checks them against scripts/guard/counts.json:
// - the ledger's `inRepo` must equal the snapshots exactly; a rise is recorded with --update, in the same commit;
// - against the ledger at the base (COUNT_BASE, else the merge base with origin/main, else with origin/claude/flip-census while the
//   flip runs), a count that falls, or a key that goes, needs a `drops` row { fixture, key, from, to, adr, proof, fixtures: { proved,
//   open } }: counted per fixture against the base's own snapshots, so a fall in one fixture is not netted by a rise in another (a base
//   with no snapshots falls back to the totals, rows without `fixture`); the ADR exists in docs/decisions.md, `proof` is `file::symbol`
//   with the symbol declared in that file, the `proved` fixture's snapshot held the key at the base and holds fewer now, and the `open`
//   one holds it now. A row with `kind: "relabel"` { fixture, key, from, to, adr, kind, relabeledTo } instead records rows that kept their
//   finding under another blocker: it names no proof or fixtures, and `relabeledTo` must gain, in that fixture, what `key` lost. It is no
//   clean. A rename needs a `renames` row { from, to, adr } with equal counts.
// - `drops` and `renames` are append-only against the base.
// - a row that left a SITE is a fall too (ADR 0995): counted per (fixture, key, path), so a key whose fixture total holds because another
//   path gained one is not netted. When the fixture total fell by exactly what its paths lost, the fixture-level row answers them; when
//   it did not (a path gained, or the total held), each path that lost rows needs a row of its own: `{ fixture, key, path, from, to, adr,
//   proof, fixtures }` (a clean at that path: `proved` held the key at that path at the base and holds fewer there now), or, for a row
//   that stayed in the fixture under the same key at another path, `{ fixture, key, path, from, to, adr, kind: "moved", movedTo }`:
//   no proof (it is no clean), and `movedTo` must gain in that fixture what `path` lost. Spans are byte offsets that move with any edit
//   of a fixture, so a path is the finest site that holds; scripts/guard/regress.mjs reads spans where the file is unchanged.
// - no base is a failure, never a pass (scripts/guard/base-revision.mjs): COUNT_BASE=<rev> names one explicitly.
// A clean needs a proof: removing an obligation to gain cleans is what this makes visible. Exit 1 on any failure.
//
// Recording an intended change: `node scripts/guard/count.mjs --update --adr "ADR NNNN"` (or COUNT_ADR) records the counts and writes
// the drops rows every fall needs, from the snapshots: a relabel where another key of the fixture gained what the key lost, a moved
// row where another path gained what a site lost, otherwise a clean, which also needs `--proof file::symbol` (or COUNT_PROOF); it
// writes nothing and exits 1 when the ADR or the proof is missing, and checks what it wrote. Rows can still be written by hand (the
// file `--update` rewrites keeps them). The ADR must exist in docs/decisions.md. `--help` prints this.
import { existsSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { baseFile, git as gitIn, helpOf, repo } from "./base-revision.mjs";
const LEDGER = "scripts/guard/counts.json";
const SNAPSHOTS = "fixtures/findings-snapshots";

/** `code blocker` -> uncertifiable rows, over snapshot objects `{ findings }`. */
export function countRows(snapshots) {
  const counts = {};
  for (const snapshot of snapshots) {
    for (const finding of snapshot.findings ?? []) {
      if (finding.kind !== "uncertifiable") continue;
      const key = `${finding.code} ${finding.blocker ?? ""}`;
      counts[key] = (counts[key] ?? 0) + 1;
    }
  }
  return Object.fromEntries(Object.entries(counts).sort(([a], [b]) => a.localeCompare(b)));
}

const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);

/** A snapshot file's fixture name: `group__name.json` -> `group/name`. */
export const fixtureOf = (name) => name.replace(/\.json$/, "").replace("__", "/");

/** Whether `text` declares `symbol` (a Rust or JavaScript item, binding or method), not merely mentions it. */
export function declares(text, symbol) {
  if (!/^[A-Za-z_$][\w$]*$/.test(symbol ?? "")) return false;
  return new RegExp(`(?:^|[^\\w$])(?:fn|function|const|let|var|class|struct|enum|trait|type|static|mod|macro_rules!)\\s+${symbol.replace(/\$/g, "\\$")}(?![\\w$])`, "m").test(text);
}

/**
 * The failures, as sentences. `ledger` is the working counts.json, `base` the one at the base (or null), `computed` the snapshots'
 * counts. `decisions` is docs/decisions.md's text; `read(path)` returns a repository file's text or null; `snapshotOf(fixture)` returns
 * a fixture's snapshot or null.
 */
export function checkLedger({ ledger, base, computed, decisions, read, snapshotOf, baseSnapshots = null, baseSnapshotOf = () => null }) {
  const failures = [];
  if (!same(ledger.inRepo, computed)) {
    const keys = new Set([...Object.keys(ledger.inRepo ?? {}), ...Object.keys(computed)]);
    const moved = [...keys].filter((key) => (ledger.inRepo?.[key] ?? 0) !== (computed[key] ?? 0));
    failures.push(`${LEDGER} does not match the snapshots (${moved.slice(0, 5).map((key) => `${key}: ${ledger.inRepo?.[key] ?? 0} -> ${computed[key] ?? 0}`).join("; ")}${moved.length > 5 ? "; ..." : ""}): run \`node scripts/guard/count.mjs --update\`, after the drops rows a fall needs`);
  }
  if (!base) return failures;
  for (const field of ["drops", "renames"]) {
    for (const row of base[field] ?? []) {
      if (!(ledger[field] ?? []).some((now) => same(now, row))) failures.push(`${field} is append-only: a row at the base is gone or changed (${row.key ?? row.from})`);
    }
  }
  const adrExists = (adr) => {
    const number = /^ADR (\d{4})$/.exec(adr ?? "")?.[1];
    return Boolean(number) && decisions.includes(`<a id="adr-${number}"></a>`);
  };
  const renamed = new Map();
  for (const rename of ledger.renames ?? []) {
    if (!adrExists(rename.adr)) failures.push(`rename ${rename.from} -> ${rename.to}: ${rename.adr ?? "no ADR"} is not in docs/decisions.md`);
    else if ((base.inRepo?.[rename.from] ?? 0) !== (computed[rename.to] ?? 0)) failures.push(`rename ${rename.from} -> ${rename.to}: ${base.inRepo?.[rename.from] ?? 0} rows became ${computed[rename.to] ?? 0}`);
    else renamed.set(rename.from, rename.to);
  }
  const keyCount = (snapshot, key, path) => (snapshot?.findings ?? []).filter((finding) => finding.kind === "uncertifiable" && `${finding.code} ${finding.blocker ?? ""}` === key && (path === undefined || finding.path === path)).length;
  const { falls, pathFalls } = fallsAgainst({ base, computed, baseSnapshots, snapshotOf, renamed });
  const checkRow = (row, { fixture, key, path, was, now }) => {
    const why = `${fixture ? `${fixture} ` : ""}${key}${path ? ` at ${path}` : ""}: ${was} -> ${now}`;
    if (!adrExists(row.adr)) failures.push(`${why}: ${row.adr ?? "no ADR"} is not in docs/decisions.md`);
    if (row.kind !== undefined && row.kind !== "relabel" && row.kind !== "moved") failures.push(`${why}: kind ${row.kind} is not a kind of drop`);
    if (row.kind === "moved") {
      // The row stayed in the fixture under the same key, at another path: no proof to name (it is no clean), and the other path gained it.
      if (row.proof !== undefined || row.fixtures !== undefined) failures.push(`${why}: a moved row names no proof or fixtures`);
      const gained = row.movedTo && row.movedTo !== path ? keyCount(snapshotOf(fixture), key, row.movedTo) - keyCount(baseSnapshotOf(fixture), key, row.movedTo) : 0;
      if (!path || !row.movedTo || row.movedTo === path || gained < was - now) failures.push(`${why}: movedTo ${row.movedTo ?? "(none)"} gained ${gained} in the fixture, not the ${was - now} that left`);
      return;
    }
    if (row.kind === "relabel") {
      // The rows stay uncertifiable under another blocker: no proof to name, and naming one would record a relabel as a clean.
      if (row.proof !== undefined || row.fixtures !== undefined) failures.push(`${why}: a relabel names no proof or fixtures`);
      const gained = keyCount(snapshotOf(fixture), row.relabeledTo) - keyCount(baseSnapshotOf(fixture), row.relabeledTo);
      if (!fixture || !row.relabeledTo || gained < was - now) failures.push(`${why}: relabeledTo ${row.relabeledTo ?? "(none)"} gained ${gained} in the fixture, not the ${was - now} that left`);
      return;
    }
    const [file, symbol, ...rest] = String(row.proof ?? "").split("::");
    const text = file ? read(file) : null;
    if (!text || rest.length || !declares(text, symbol)) failures.push(`${why}: proof ${row.proof ?? "(none)"} is no file::symbol whose file declares the symbol`);
    // A row at a path is proved at that path too: the proved fixture held the key there at the base and holds fewer there now, and
    // fewer in total (a row that only moved inside the proved fixture proves nothing).
    const provedAt = baseSnapshotOf(row.fixtures?.proved);
    const provedNow = snapshotOf(row.fixtures?.proved);
    const fewer = (scope) => keyCount(provedNow, key, scope) < keyCount(provedAt, key, scope);
    if (!provedAt || !provedNow || !keyCount(provedAt, key, path) || !fewer(undefined) || (path !== undefined && !fewer(path))) failures.push(`${why}: fixtures.proved ${row.fixtures?.proved ?? "(none)"} is no snapshot that held a ${key} row${path ? ` at ${path}` : ""} at the base and holds fewer now`);
    const open = snapshotOf(row.fixtures?.open);
    if (!open || !keyCount(open, key)) failures.push(`${why}: fixtures.open ${row.fixtures?.open ?? "(none)"} is no snapshot that keeps a ${key} row`);
  };
  for (const { fixture, key, was, now } of falls) {
    const row = (ledger.drops ?? []).find((drop) => drop.fixture === fixture && drop.path === undefined && drop.key === key && drop.from === was && drop.to === now);
    if (!row) {
      failures.push(`${fixture ? `${fixture} ` : ""}${key}: ${was} -> ${now}, no drops row: name the fixture, an ADR, a proof, and a fixture on each side`);
      continue;
    }
    checkRow(row, { fixture, key, was, now });
  }
  // A site that lost rows (ADR 0995). When the fixture total fell by exactly what its paths lost, the fixture-level row above answers
  // them; otherwise (a path gained, or the total held) a row left a site and the fixture total hides it: each such path needs its own row.
  for (const fall of pathFalls) {
    if (!pathFallNeedsRow(fall, falls, pathFalls)) continue;
    const total = falls.find((other) => other.fixture === fall.fixture && other.key === fall.key);
    const row = (ledger.drops ?? []).find((drop) => drop.fixture === fall.fixture && drop.key === fall.key && drop.path === fall.path && drop.from === fall.was && drop.to === fall.now);
    if (!row) {
      failures.push(`${fall.fixture} ${fall.key} at ${fall.path}: ${fall.was} -> ${fall.now}, no drops row: ${total ? "the fixture's total fell by less than its paths lost" : "the fixture's total holds or rose"}, so a row left this site (another path gained one) or was cleaned and another appeared; add { fixture, key, path, from, to, adr, kind: "moved", movedTo } or a path-level proof row`);
      continue;
    }
    checkRow(row, fall);
  }
  return failures;
}

/**
 * The falls against the base, as the guard counts them: per fixture against the base's snapshots (with none there, per key against its
 * totals), and per (fixture, key, path) site (ADR 0995). `renamed` keys are skipped.
 */
export function fallsAgainst({ base, computed, baseSnapshots, snapshotOf, renamed = new Map() }) {
  const falls = [];
  const pathFalls = [];
  if (baseSnapshots) {
    for (const [fixture, snapshot] of baseSnapshots) {
      const sites = keySites(snapshot);
      const sitesNow = keySites(snapshotOf(fixture));
      for (const [key, paths] of sites) {
        if (renamed.has(key)) continue;
        const was = [...paths.values()].reduce((a, b) => a + b, 0);
        const now = [...(sitesNow.get(key)?.values() ?? [])].reduce((a, b) => a + b, 0);
        if (now < was) falls.push({ fixture, key, was, now });
        for (const [path, before] of paths) {
          const after = sitesNow.get(key)?.get(path) ?? 0;
          if (after < before) pathFalls.push({ fixture, key, path, was: before, now: after });
        }
      }
    }
  } else {
    for (const [key, was] of Object.entries(base?.inRepo ?? {})) {
      if (renamed.has(key)) continue;
      const now = computed[key] ?? 0;
      if (now < was) falls.push({ fixture: undefined, key, was, now });
    }
  }
  return { falls, pathFalls };
}

/** Whether a site's fall needs a row of its own: the fixture total did not fall by exactly what its paths lost (ADR 0995). */
function pathFallNeedsRow(fall, falls, pathFalls) {
  const lost = pathFalls.filter((other) => other.fixture === fall.fixture && other.key === fall.key).reduce((sum, other) => sum + other.was - other.now, 0);
  const total = falls.find((other) => other.fixture === fall.fixture && other.key === fall.key);
  return !(total && total.was - total.now === lost);
}

/**
 * The drops rows `--update` writes for the falls no row answers yet, so an intended change never fails the guard for a forgotten row.
 * Each row names `adr` (the guard still checks it is in docs/decisions.md). The kind follows the snapshots:
 * - another key of the fixture gained at least what this key lost: a `relabel` row (the rows kept their finding under that blocker);
 * - another path of the key gained at least what a site lost: a `moved` row;
 * - otherwise a clean, which names `proof` (`file::symbol`) and its fixtures: `proved` the fixture itself, `open` the first snapshot
 *   that keeps the key (the fixture itself when it still does).
 * `needs` lists what is missing to write them: no `adr` for any row, no `proof` for a clean.
 *
 * @returns {{ rows: object[], needs: string[] }}
 */
export function draftDrops({ ledger, base, computed, baseSnapshots, snapshotOf, baseSnapshotOf, fixtures = [], adr, proof }) {
  const renamed = new Map((ledger.renames ?? []).map((rename) => [rename.from, rename.to]));
  const { falls, pathFalls } = fallsAgainst({ base, computed, baseSnapshots, snapshotOf, renamed });
  const drops = ledger.drops ?? [];
  const keyTotal = (snapshot, key) => [...(keySites(snapshot).get(key)?.values() ?? [])].reduce((a, b) => a + b, 0);
  const rows = [];
  const needs = [];
  const clean = (fall, path) => {
    if (!proof) needs.push(`${fall.fixture ?? "(totals)"} ${fall.key}${path ? ` at ${path}` : ""}: ${fall.was} -> ${fall.now} is a clean (no other key or path gained it): pass --proof <file::symbol>`);
    const open = [fall.fixture, ...fixtures].find((fixture) => fixture && keyTotal(snapshotOf(fixture), fall.key) > 0);
    return { ...(fall.fixture ? { fixture: fall.fixture } : {}), key: fall.key, ...(path ? { path } : {}), from: fall.was, to: fall.now, adr, proof, fixtures: { proved: fall.fixture, open } };
  };
  for (const fall of falls) {
    if (drops.some((drop) => drop.fixture === fall.fixture && drop.path === undefined && drop.key === fall.key && drop.from === fall.was && drop.to === fall.now)) continue;
    const lost = fall.was - fall.now;
    if (fall.fixture) {
      const now = keySites(snapshotOf(fall.fixture));
      const was = keySites(baseSnapshotOf(fall.fixture));
      const gains = [...now.keys()]
        .filter((key) => key !== fall.key)
        .map((key) => ({ key, gained: keyTotal(snapshotOf(fall.fixture), key) - [...(was.get(key)?.values() ?? [])].reduce((a, b) => a + b, 0) }))
        .filter(({ gained }) => gained >= lost)
        .sort((a, b) => b.gained - a.gained || a.key.localeCompare(b.key));
      if (gains.length) {
        rows.push({ fixture: fall.fixture, key: fall.key, from: fall.was, to: fall.now, adr, kind: "relabel", relabeledTo: gains[0].key });
        continue;
      }
    }
    rows.push(clean(fall));
  }
  for (const fall of pathFalls) {
    if (!pathFallNeedsRow(fall, falls, pathFalls)) continue;
    if (drops.some((drop) => drop.fixture === fall.fixture && drop.key === fall.key && drop.path === fall.path && drop.from === fall.was && drop.to === fall.now)) continue;
    const lost = fall.was - fall.now;
    const now = keySites(snapshotOf(fall.fixture)).get(fall.key) ?? new Map();
    const was = keySites(baseSnapshotOf(fall.fixture)).get(fall.key) ?? new Map();
    const movedTo = [...now.keys()].filter((path) => path !== fall.path && (now.get(path) ?? 0) - (was.get(path) ?? 0) >= lost).sort()[0];
    if (movedTo) rows.push({ fixture: fall.fixture, key: fall.key, path: fall.path, from: fall.was, to: fall.now, adr, kind: "moved", movedTo });
    else rows.push(clean(fall, fall.path));
  }
  if (rows.length && !adr) needs.unshift(`${rows.length} drops row(s) to write (${rows.map((row) => `${row.fixture ?? "(totals)"} ${row.key}${row.kind ? ` ${row.kind}` : ""}`).join("; ")}): pass --adr "ADR NNNN" (or COUNT_ADR)`);
  return { rows, needs };
}

/** `key -> path -> uncertifiable rows` of one snapshot (the sites a fall is measured at). */
export function keySites(snapshot) {
  const sites = new Map();
  for (const finding of snapshot?.findings ?? []) {
    if (finding.kind !== "uncertifiable") continue;
    const key = `${finding.code} ${finding.blocker ?? ""}`;
    if (!sites.has(key)) sites.set(key, new Map());
    const paths = sites.get(key);
    paths.set(finding.path, (paths.get(finding.path) ?? 0) + 1);
  }
  return sites;
}

/** The base revision's snapshots, `fixture -> snapshot`, or null when it has none. */
export function snapshotsAt(revision) {
  const listing = git("ls-tree", "-r", revision, `${SNAPSHOTS}/`);
  if (!listing) return null;
  const blobs = listing.split("\n").map((line) => /^\d+ blob ([0-9a-f]+)\t(.+)$/.exec(line)).filter((m) => m && m[2].endsWith(".json"));
  if (!blobs.length) return null;
  const run = spawnSync("git", ["cat-file", "--batch"], { cwd: repo, input: blobs.map((m) => m[1]).join("\n") + "\n", maxBuffer: 1 << 30 });
  if (run.status !== 0) return null;
  const out = run.stdout;
  const snapshots = new Map();
  let at = 0;
  for (const [, , path] of blobs) {
    const header = out.indexOf(10, at);
    const size = Number(out.subarray(at, header).toString().split(" ")[2]);
    const body = out.subarray(header + 1, header + 1 + size).toString("utf8");
    at = header + 1 + size + 1;
    snapshots.set(fixtureOf(path.split("/").pop()), JSON.parse(body));
  }
  return snapshots;
}

export const git = gitIn;

/** The ledger at the base: `{ revision, ledger, problem }`; `problem` is a sentence when there is no base or its ledger is missing (a failure, ADR 0995). */
export function baseLedger() {
  const { revision, text, problem } = baseFile(LEDGER);
  return { revision: revision ?? "(none)", ledger: text ? JSON.parse(text) : null, problem };
}

async function main() {
  if (process.argv.includes("--help")) return console.log(helpOf(import.meta.url));
  const snapshotOf = (fixture) => {
    if (!fixture) return null;
    const path = join(repo, SNAPSHOTS, `${fixture.replace(/^fixtures\//, "").replace("/", "__")}.json`);
    return existsSync(path) ? JSON.parse(readFileSync(path, "utf8")) : null;
  };
  const names = readdirSync(join(repo, SNAPSHOTS)).filter((name) => name.endsWith(".json")).sort();
  const snapshots = names.map((name) => JSON.parse(readFileSync(join(repo, SNAPSHOTS, name), "utf8")));
  const computed = countRows(snapshots);
  const path = join(repo, LEDGER);
  const ledger = existsSync(path) ? JSON.parse(readFileSync(path, "utf8")) : { inRepo: {}, drops: [], renames: [] };
  const { revision, ledger: base, problem } = baseLedger();
  const baseSnapshots = base ? snapshotsAt(revision) : null;
  const baseSnapshotOf = (fixture) => (fixture ? baseSnapshots?.get(fixture.replace(/^fixtures\//, "")) ?? null : null);
  const check = (candidate) => checkLedger({
    ledger: candidate, base, computed, baseSnapshots, baseSnapshotOf,
    decisions: readFileSync(join(repo, "docs/decisions.md"), "utf8"),
    read: (file) => (existsSync(join(repo, file)) ? readFileSync(join(repo, file), "utf8") : null),
    snapshotOf,
  });
  if (process.argv.includes("--update")) {
    // The drops rows a fall needs are written here, from the snapshots, so a fix never fails the guard for a forgotten row; the ADR
    // (and, for a clean, the proof) is the author's to name: `--adr "ADR NNNN"` / COUNT_ADR, `--proof file::symbol` / COUNT_PROOF.
    const flag = (name) => {
      const index = process.argv.indexOf(name);
      return index >= 0 ? process.argv[index + 1] : undefined;
    };
    const adrArg = flag("--adr") ?? process.env.COUNT_ADR;
    const adr = adrArg ? (/^\d{4}$/.test(adrArg) ? `ADR ${adrArg}` : adrArg) : undefined;
    const proof = flag("--proof") ?? process.env.COUNT_PROOF;
    const { rows: drafted, needs } = base
      ? draftDrops({ ledger, base, computed, baseSnapshots, snapshotOf, baseSnapshotOf, fixtures: names.map(fixtureOf), adr, proof })
      : { rows: [], needs: [] };
    if (needs.length) {
      for (const need of needs) console.error(`count --update: ${need}`);
      console.error(`count --update: nothing written`);
      process.exitCode = 1;
      return;
    }
    const updated = { about: ledger.about, inRepo: computed, drops: [...(ledger.drops ?? []), ...drafted], renames: ledger.renames ?? [] };
    writeFileSync(path, `${JSON.stringify(updated, null, 1)}\n`);
    for (const row of drafted) console.log(`count --update: drops row ${row.fixture ?? "(totals)"} ${row.key}${row.path ? ` at ${row.path}` : ""} ${row.from} -> ${row.to} (${row.kind ?? "clean"}${row.relabeledTo ? ` to ${row.relabeledTo}` : row.movedTo ? ` to ${row.movedTo}` : ""}, ${row.adr})`);
    console.log(`${LEDGER}: ${Object.keys(computed).length} keys, ${Object.values(computed).reduce((a, b) => a + b, 0)} rows recorded${drafted.length ? `, ${drafted.length} drops row(s) written` : ""}`);
    // What was written must pass the guard as it stands (an ADR missing from docs/decisions.md, a proof the file does not declare).
    const left = problem ? [problem] : check(updated);
    for (const failure of left) console.error(failure);
    process.exitCode = left.length ? 1 : 0;
    return;
  }
  const failures = problem ? [problem] : [];
  failures.push(...check(ledger));
  for (const failure of failures) console.error(failure);
  if (base) console.log(`count: falls compared ${baseSnapshots ? `per fixture over ${baseSnapshots.size} base snapshots` : "per key (no snapshots at the base)"}`);
  const rows = Object.values(computed).reduce((a, b) => a + b, 0);
  console.log(`count: ${rows} uncertifiable rows in ${Object.keys(computed).length} keys; ${base ? `base ${revision}` : `no base, nothing compared`}; ${(ledger.drops ?? []).length} drops recorded`);
  process.exitCode = failures.length ? 1 : 0;
}

if (import.meta.main ?? process.argv[1] === fileURLToPath(import.meta.url)) await main();
