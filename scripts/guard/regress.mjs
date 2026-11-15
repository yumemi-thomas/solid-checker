// node scripts/guard/regress.mjs [--append "ADR NNNN" --why "<text>"]: a proved result must not silently become unproven (ADR 0995).
//
// count.mjs holds the uncertifiable count one way and rises.mjs names its rises, but neither sees a VIOLATION leave: a veto or a
// fail-closed change that turns a proved violation into an uncertifiable row (V -> U), or loses it altogether, moves the snapshot, raises the
// count by one at most, and says nothing of what was proved before. This lists, per EXISTING fixture (a snapshot the base also has) and per
// (fixture, path, code), the violations the base held that the snapshots no longer hold:
//   - where the path's file is unchanged since the base, a violation is removed when no violation of its code stands at its span any more;
//     spans are byte offsets, so they are compared only for a file whose bytes did not change;
//   - where the file changed, a span means nothing, so the violations of that code in that file are counted: a fall is a removal.
// Each removal is `became uncertifiable` (an uncertifiable row of the same code stands at the span, or, for a changed file, rose in the file)
// or `gone` (nothing stands). A removal FAILS unless a row of scripts/guard/regress.json names it:
//   { fixture, path, code, removed, adr, why }   `removed` is how many violations of the code left that path; the ADR is in docs/decisions.md;
//   `why` says what proved the violation wrong or why it is now withheld (a fail-closed fact: name it as scripts/u-blockers.mjs does).
// A deleted fixture's violations are `gone` too. Plain U rises stay print-only (rises.mjs); a fixture that was clean (no findings, status
// `certified`) and holds uncertifiable rows now is printed as `clean -> U`, never a failure. `regress` is append-only against the base.
// No base is a failure, never a pass (COUNT_BASE=<rev> names one).
//
// Recording an intended change: `node scripts/guard/regress.mjs --append "ADR NNNN" --why "<text>"` appends a row for every unnamed
// removal into scripts/guard/regress.json (the ADR must be in docs/decisions.md and say why the proved violation is withdrawn); commit it
// with the change that moved the snapshot. Only an intended change gets a row: a removal nobody meant is the bug this finds.
import { existsSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

import { baseFile, git, helpOf, repo } from "./base-revision.mjs";
import { fixtureOf, snapshotsAt } from "./count.mjs";

const REGRESS = "scripts/guard/regress.json";
const SNAPSHOTS = "fixtures/findings-snapshots";
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
const span = (finding) => `${finding.start}-${finding.end}`;

/**
 * The removals: `fixture -> snapshot` maps at the base and now (`snapshots`), and `changed(path)`, whether the path's bytes differ from the
 * base. `[{ fixture, path, code, removed, uncertifiable, gone, blockers, mode }]`, sorted.
 */
export function measureRegress(baseSnapshots, snapshots, changed) {
  const found = [];
  for (const [fixture, before] of [...baseSnapshots].sort(([a], [b]) => (a < b ? -1 : 1))) {
    const now = snapshots.get(fixture) ?? { findings: [] };
    const groups = new Map();
    for (const finding of before.findings ?? []) {
      if (finding.kind !== "violation") continue;
      const key = `${finding.path}\0${finding.code}`;
      groups.set(key, [...(groups.get(key) ?? []), finding]);
    }
    for (const [key, held] of groups) {
      const [path, code] = key.split("\0");
      const nowViolations = (now.findings ?? []).filter((finding) => finding.kind === "violation" && finding.path === path && finding.code === code);
      const nowUncertifiable = (now.findings ?? []).filter((finding) => finding.kind === "uncertifiable" && finding.path === path && finding.code === code);
      const wasUncertifiable = (before.findings ?? []).filter((finding) => finding.kind === "uncertifiable" && finding.path === path && finding.code === code);
      if (!changed(path)) {
        const left = [...held];
        for (const finding of nowViolations) {
          const at = left.findIndex((candidate) => span(candidate) === span(finding));
          if (at >= 0) left.splice(at, 1);
        }
        if (!left.length) continue;
        const spans = new Set(left.map(span));
        const became = nowUncertifiable.filter((finding) => spans.has(span(finding)));
        found.push({ fixture, path, code, removed: left.length, uncertifiable: new Set(became.map(span)).size, gone: left.length - new Set(became.map(span)).size, blockers: [...new Set(became.map((finding) => finding.blocker ?? ""))].sort(), mode: "span" });
      } else {
        const removed = held.length - nowViolations.length;
        if (removed <= 0) continue;
        const rose = Math.max(0, nowUncertifiable.length - wasUncertifiable.length);
        const gained = nowUncertifiable.map((finding) => finding.blocker ?? "");
        for (const finding of wasUncertifiable) {
          const at = gained.indexOf(finding.blocker ?? "");
          if (at >= 0) gained.splice(at, 1);
        }
        found.push({ fixture, path, code, removed, uncertifiable: Math.min(removed, rose), gone: removed - Math.min(removed, rose), blockers: [...new Set(gained)].sort(), mode: "count" });
      }
    }
  }
  return found;
}

/** A fixture clean at the base (no findings) that holds uncertifiable rows now: `[{ fixture, rows }]`. Detectable only for fixtures both have. */
export function cleanToU(baseSnapshots, snapshots) {
  const found = [];
  for (const [fixture, before] of [...baseSnapshots].sort(([a], [b]) => (a < b ? -1 : 1))) {
    const now = snapshots.get(fixture);
    if (!now || (before.findings ?? []).length > 0 || before.status !== "certified") continue;
    const rows = (now.findings ?? []).filter((finding) => finding.kind === "uncertifiable").length;
    if (rows > 0) found.push({ fixture, rows });
  }
  return found;
}

/** Whether a row names a removal. */
export const names = (row, item) => row.fixture === item.fixture && row.path === item.path && row.code === item.code && row.removed === item.removed;

/** The failures, and the removals a row names or none does. `base` is the ledger at the base (or `{ regress: [] }`). */
export function checkRegress({ ledger, base, measured, decisions }) {
  const failures = [];
  const rows = ledger.regress ?? [];
  for (const row of base?.regress ?? []) if (!rows.some((now) => same(now, row))) failures.push(`regress is append-only: a row at the base is gone or changed (${row.fixture} ${row.path} ${row.code})`);
  rows.forEach((row, index) => {
    const label = `regress[${index}] ${row.fixture ?? "(no fixture)"} ${row.code ?? "(no code)"}`;
    for (const field of ["fixture", "path", "code", "why"]) if (typeof row[field] !== "string" || row[field].trim() === "") failures.push(`${label}: ${field} is empty`);
    if (!Number.isInteger(row.removed) || row.removed < 1) failures.push(`${label}: removed is a count of at least 1 (${row.removed})`);
    const number = /^ADR (\d{4})$/.exec(row.adr ?? "")?.[1];
    if (!number || !decisions.includes(`<a id="adr-${number}"></a>`)) failures.push(`${label}: ${row.adr ?? "no ADR"} is not in docs/decisions.md`);
  });
  const unnamed = measured.filter((item) => !rows.some((row) => names(row, item)));
  for (const item of unnamed) {
    const becomes = `${item.uncertifiable} became uncertifiable${item.blockers.length ? ` (${item.blockers.join(", ")})` : ""}, ${item.gone} gone`;
    failures.push(`${item.fixture} ${item.path} ${item.code}: ${item.removed} violation(s) left (${becomes}; ${item.mode === "span" ? "by span" : "file changed: by count"}) with no regress row: a proved result must not become unproven unrecorded; if intended, \`node scripts/guard/regress.mjs --append "ADR NNNN" --why "..."\``);
  }
  return { failures, named: measured.filter((item) => rows.some((row) => names(row, item))), unnamed };
}

const readSnapshots = () => new Map(readdirSync(join(repo, SNAPSHOTS)).filter((name) => name.endsWith(".json")).map((name) => [fixtureOf(name), JSON.parse(readFileSync(join(repo, SNAPSHOTS, name), "utf8"))]));

/** The paths whose bytes differ from `revision` (working tree against it), or null when git cannot say. */
export function changedPaths(revision) {
  const out = git("diff", "--name-only", "--no-renames", revision, "--", "fixtures");
  return out === null ? null : new Set(out.split("\n").filter(Boolean));
}

function main(argv) {
  if (argv.includes("--help")) return console.log(helpOf(import.meta.url));
  const value = (name) => { const at = argv.indexOf(name); return at < 0 ? undefined : argv[at + 1]; };
  const path = join(repo, REGRESS);
  const ledger = existsSync(path) ? JSON.parse(readFileSync(path, "utf8")) : { about: "", regress: [] };
  const decisions = readFileSync(join(repo, "docs/decisions.md"), "utf8");
  const counts = baseFile("scripts/guard/counts.json");
  const failures = [];
  let measured = [];
  let changeCount = "(no base)";
  let cleans = [];
  let baseLedgerRows = { regress: [] };
  if (counts.problem) failures.push(counts.problem);
  else {
    const baseSnapshots = snapshotsAt(counts.revision);
    const changed = changedPaths(counts.revision);
    if (!baseSnapshots) failures.push(`no snapshots at the base ${counts.revision}: nothing to compare the violations with`);
    else if (!changed) failures.push(`git cannot list the paths changed since the base ${counts.revision}`);
    else {
      const snapshots = readSnapshots();
      measured = measureRegress(baseSnapshots, snapshots, (file) => changed.has(file));
      cleans = cleanToU(baseSnapshots, snapshots);
      changeCount = `${changed.size} fixture files changed`;
    }
    const baseText = git("show", `${counts.revision}:${REGRESS}`);
    if (baseText) baseLedgerRows = JSON.parse(baseText);
  }
  const { failures: problems, named, unnamed } = checkRegress({ ledger, base: baseLedgerRows, measured, decisions });
  if (argv.includes("--append")) {
    const adr = value("--append");
    const why = value("--why");
    if (!adr || !why) throw new Error('--append "ADR NNNN" --why "<text>"');
    const added = unnamed.map((item) => ({ fixture: item.fixture, path: item.path, code: item.code, removed: item.removed, adr, why }));
    writeFileSync(path, `${JSON.stringify({ about: ledger.about, regress: [...(ledger.regress ?? []), ...added] }, null, 1)}\n`);
    console.log(`${REGRESS}: ${added.length} rows appended`);
    return;
  }
  failures.push(...problems);
  for (const failure of failures) console.error(failure);
  for (const item of cleans) console.log(`clean -> U (print-only): ${item.fixture} was certified with no findings and holds ${item.rows} uncertifiable row(s)`);
  console.log(`regress: ${measured.length} removal(s) of proved violations against ${counts.revision ?? "(no base)"} (${changeCount}), ${named.length} named, ${unnamed.length} unnamed; ${cleans.length} clean -> U; ${(ledger.regress ?? []).length} rows in ${REGRESS}`);
  process.exitCode = failures.length ? 1 : 0;
}

if (import.meta.main ?? process.argv[1] === fileURLToPath(import.meta.url)) main(process.argv.slice(2));
