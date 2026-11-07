// node scripts/guard/count.mjs [--update]: the one-way uncertifiable count (endstage-report.md 1b).
// Counts the uncertifiable rows of every coverage snapshot by `code blocker` and checks them against scripts/guard/counts.json:
// - the ledger's `inRepo` must equal the snapshots exactly; a rise is recorded with --update, in the same commit;
// - against the ledger at the base (COUNT_BASE, else the merge base with origin/main, else with origin/claude/flip-census while the
//   flip runs), a count that falls, or a key that goes, needs a `drops` row { fixture, key, from, to, adr, proof, fixtures: { proved,
//   open } }: counted per fixture against the base's own snapshots, so a fall in one fixture is not netted by a rise in another (a base
//   with no snapshots falls back to the totals, rows without `fixture`); the ADR exists in docs/decisions.md, `proof` is `file::symbol`
//   with the symbol declared in that file, the `proved` fixture's snapshot held the key at the base and holds fewer now, and the `open`
//   one holds it now. A rename needs a `renames` row { from, to, adr } with equal counts.
// - `drops` and `renames` are append-only against the base.
// A clean needs a proof: removing an obligation to gain cleans is what this makes visible. Exit 1 on any failure.
import { existsSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
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
  const keyCount = (snapshot, key) => (snapshot?.findings ?? []).filter((finding) => finding.kind === "uncertifiable" && `${finding.code} ${finding.blocker ?? ""}` === key).length;
  // Falls per fixture against the base's snapshots; with none at the base, per key against its totals.
  const falls = [];
  if (baseSnapshots) {
    for (const [fixture, snapshot] of baseSnapshots) {
      for (const [key, was] of Object.entries(countRows([snapshot]))) {
        if (renamed.has(key)) continue;
        const now = keyCount(snapshotOf(fixture), key);
        if (now < was) falls.push({ fixture, key, was, now });
      }
    }
  } else {
    for (const [key, was] of Object.entries(base.inRepo ?? {})) {
      if (renamed.has(key)) continue;
      const now = computed[key] ?? 0;
      if (now < was) falls.push({ fixture: undefined, key, was, now });
    }
  }
  for (const { fixture, key, was, now } of falls) {
    const row = (ledger.drops ?? []).find((drop) => drop.fixture === fixture && drop.key === key && drop.from === was && drop.to === now);
    const why = `${fixture ? `${fixture} ` : ""}${key}: ${was} -> ${now}`;
    if (!row) {
      failures.push(`${why}, no drops row: name the fixture, an ADR, a proof, and a fixture on each side`);
      continue;
    }
    if (!adrExists(row.adr)) failures.push(`${why}: ${row.adr ?? "no ADR"} is not in docs/decisions.md`);
    const [file, symbol, ...rest] = String(row.proof ?? "").split("::");
    const text = file ? read(file) : null;
    if (!text || rest.length || !declares(text, symbol)) failures.push(`${why}: proof ${row.proof ?? "(none)"} is no file::symbol whose file declares the symbol`);
    const provedAt = baseSnapshotOf(row.fixtures?.proved);
    const provedNow = snapshotOf(row.fixtures?.proved);
    if (!provedAt || !provedNow || !keyCount(provedAt, key) || keyCount(provedNow, key) >= keyCount(provedAt, key)) failures.push(`${why}: fixtures.proved ${row.fixtures?.proved ?? "(none)"} is no snapshot that held a ${key} row at the base and holds fewer now`);
    const open = snapshotOf(row.fixtures?.open);
    if (!open || !keyCount(open, key)) failures.push(`${why}: fixtures.open ${row.fixtures?.open ?? "(none)"} is no snapshot that keeps a ${key} row`);
  }
  return failures;
}

/** The base revision's snapshots, `fixture -> snapshot`, or null when it has none. */
function snapshotsAt(revision) {
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

function git(...args) {
  const run = spawnSync("git", args, { cwd: repo, encoding: "utf8", maxBuffer: 1 << 28 });
  return run.status === 0 ? run.stdout.trim() : null;
}

function baseLedger() {
  const revisions = process.env.COUNT_BASE ? [process.env.COUNT_BASE] : ["origin/main", "origin/claude/flip-census"].map((ref) => git("merge-base", "HEAD", ref)).filter(Boolean);
  for (const revision of revisions) {
    const text = git("show", `${revision}:${LEDGER}`);
    if (text) return { revision, ledger: JSON.parse(text) };
  }
  return { revision: revisions.join(", ") || "(none)", ledger: null };
}

async function main() {
  const snapshotOf = (fixture) => {
    if (!fixture) return null;
    const path = join(repo, SNAPSHOTS, `${fixture.replace(/^fixtures\//, "").replace("/", "__")}.json`);
    return existsSync(path) ? JSON.parse(readFileSync(path, "utf8")) : null;
  };
  const snapshots = readdirSync(join(repo, SNAPSHOTS)).filter((name) => name.endsWith(".json")).map((name) => JSON.parse(readFileSync(join(repo, SNAPSHOTS, name), "utf8")));
  const computed = countRows(snapshots);
  const path = join(repo, LEDGER);
  const ledger = existsSync(path) ? JSON.parse(readFileSync(path, "utf8")) : { inRepo: {}, drops: [], renames: [] };
  if (process.argv.includes("--update")) {
    writeFileSync(path, `${JSON.stringify({ about: ledger.about, inRepo: computed, drops: ledger.drops ?? [], renames: ledger.renames ?? [] }, null, 1)}\n`);
    console.log(`${LEDGER}: ${Object.keys(computed).length} keys, ${Object.values(computed).reduce((a, b) => a + b, 0)} rows recorded`);
    return;
  }
  const { revision, ledger: base } = baseLedger();
  const baseSnapshots = base && !revision.includes(",") ? snapshotsAt(revision) : null;
  const failures = checkLedger({
    ledger, base, computed, baseSnapshots,
    baseSnapshotOf: (fixture) => (fixture ? baseSnapshots?.get(fixture.replace(/^fixtures\//, "")) ?? null : null),
    decisions: readFileSync(join(repo, "docs/decisions.md"), "utf8"),
    read: (file) => (existsSync(join(repo, file)) ? readFileSync(join(repo, file), "utf8") : null),
    snapshotOf,
  });
  for (const failure of failures) console.error(failure);
  if (base) console.log(`count: falls compared ${baseSnapshots ? `per fixture over ${baseSnapshots.size} base snapshots` : "per key (no snapshots at the base)"}`);
  const rows = Object.values(computed).reduce((a, b) => a + b, 0);
  console.log(`count: ${rows} uncertifiable rows in ${Object.keys(computed).length} keys; ${base ? `base ${revision}` : `no ledger at base ${revision}, no fall compared`}; ${(ledger.drops ?? []).length} drops recorded`);
  process.exitCode = failures.length ? 1 : 0;
}

if (import.meta.main ?? process.argv[1] === fileURLToPath(import.meta.url)) await main();
