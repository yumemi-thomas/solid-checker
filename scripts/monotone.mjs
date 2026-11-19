#!/usr/bin/env node
// The monotonicity gate of a proof change (ADR 1118). A proof only adds verdicts: it turns an uncertifiable (site, code) into a clean or a violation,
// and moves nothing else. Two checks, checker only (no Chrome), on the same programs with two release builds:
//
//   M1, ablation identity. The head build with the change's arms or silences ablated (`SOLID_CHECKER_IR_ABLATE_ARM`, role_log.rs;
//       `SOLID_CHECKER_IR_ABLATE_SILENCE`, site_accounting.rs and the after-await and flush accounting) reports byte-identical findings to the base
//       build. It proves the change is only the arms it names: anything else that moved is a fix or a refactor, and is judged as one.
//   M2, upward moves only. Head against base, (site, code) by (site, code): a move from U to clean or from U to V passes; any other (clean or V to
//       U, V to clean, clean to V) fails. A site is the finding's file and span; its kind is V when a finding of that code there is a violation, U
//       when only obligations, clean when none.
//
// Programs (`--set`, repeatable): `textbook` (fixtures/textbook, the ledger's top tier), `fixtures` (the coverage projects, scripts/lib/fixture-projects.mjs),
// `grid:<grid>[:<n>]` (n programs of a grid, chosen by a hash of the id, regenerated as the grid builds them; all when n is absent). The corpus is not
// a set here yet: its old/new run is `make corpus-delta`, and applying M2's moves to its reports is a later slice of ADR 1118.
//
//   node scripts/monotone.mjs --base <checker> --head <checker> --typefacts <bin> [--ablate-arm a,b] [--ablate-silence s,t] --set textbook
//        [--set fixtures] [--set grid:lg-base:400] [--concurrency 4] --out <report.json>
//
// M2b, the proof log names the arm (with --ablate-arm or --ablate-silence): the head runs with the proof log on, and every (site, code) M2 moved up
// must have, in its file and on a span that overlaps the finding's, a `decision` line whose arms include one of the change's arms or a `silent` line
// whose proof is one of its silences. A move nothing names is a move the change's arms do not explain.
//
// `--base-ablated` (a control of the gate itself): the base runs with the named arms ablated too, standing in for the build before them; head
// against it then must move only up, every up move named by those arms (M1 holds by construction).
//
// Exit 1 when M1, M2 or M2b fails; the report lists every program and (site, code) that did. Without an ablation only M2 runs (a fix is judged by M2's
// list against its named losses, not passed by it).
import { spawn } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { fixtureProjects, runtimeArguments } from "./lib/fixture-projects.mjs";
import { mapPoolSettled } from "./lib/pool.mjs";

const ROOT = resolve(fileURLToPath(new URL(".", import.meta.url)), "..");
/** The arguments the fuzz harness and a user's browser build run with (checker.mjs `ORDINARY_ARGS`). */
const ORDINARY = ["--format", "json", "--runtime-target", "browser", "--runtime-build", "development", "--rendering", "csr"];

/** A finding's (site, code): its file (relative to the project), span and rule id. The harness entry (main.tsx) is the entry's, not the program's. */
export function siteCodes(report, root) {
  const prefix = `${root}/`;
  const out = new Map();
  for (const finding of report.findings ?? []) {
    const location = finding.primaryLocation ?? {};
    const path = (location.path ?? "").startsWith(prefix) ? location.path.slice(prefix.length) : location.path ?? "";
    if (path === "main.tsx" && root.includes(".solid-checker-fuzz")) continue;
    const span = location.startByte !== undefined ? `${location.startByte}-${location.endByte}` : `${location.line}:${location.column}-${location.endLine}:${location.endColumn}`;
    const key = `${path}@${span}|${finding.id}`;
    const kind = finding.kind === "violation" ? "V" : finding.kind === "uncertifiable" ? "U" : null;
    if (!kind) continue;
    out.set(key, out.get(key) === "V" || kind === "V" ? "V" : "U");
  }
  return out;
}

/** M2's moves of one program: `{ up: [...], bad: [...] }`, each `{ site, from, to }` (kinds V, U, clean). */
export function moves(base, head) {
  const up = [], bad = [];
  for (const site of new Set([...base.keys(), ...head.keys()])) {
    const from = base.get(site) ?? "clean", to = head.get(site) ?? "clean";
    if (from === to) continue;
    (from === "U" ? up : bad).push({ site, from, to });
  }
  return { up, bad };
}

/** A site key's file and span (`siteCodes`'s `path@start-end|code`); null for a line/column span. */
export function keySpan(key) {
  const match = /^(.*)@(\d+)-(\d+)\|/.exec(key);
  return match ? { path: match[1], start: Number(match[2]), end: Number(match[3]) } : null;
}

/** The proof log's naming lines (ADR 0933): `{ path, start, end, names }` per decision (its arms) and silent line (its proof), paths relative to `root`. */
export function namingLines(text, root) {
  const prefix = `${root}/`;
  const paths = new Map();
  const out = [];
  for (const line of text.split("\n")) {
    if (!line) continue;
    const entry = JSON.parse(line);
    if (entry.k === "path") { paths.set(`${entry.q}:${entry.p}`, entry.f.startsWith(prefix) ? entry.f.slice(prefix.length) : entry.f); continue; }
    const names = entry.k === "decision" ? [entry.arm, ...(entry.arms ?? [])].filter(Boolean) : entry.k === "silent" && entry.proof ? [entry.proof] : [];
    if (names.length) out.push({ path: paths.get(`${entry.q}:${entry.p}`) ?? null, start: entry.s, end: entry.e, names, silent: entry.k === "silent" });
  }
  return out;
}

/** M2b: the up moves no naming line of `arms` explains (same file, overlapping span). */
export function unnamedUp(up, lines, arms) {
  return up.filter((move) => {
    const at = keySpan(move.site);
    if (!at) return true;
    // `all` (the ablation hooks' own word for every silence) is named by any silent line.
    return !lines.some((line) => line.path === at.path && line.start < at.end && at.start < line.end && line.names.some((name) => arms.has(name) || (arms.has("all") && line.silent)));
  });
}

/** The findings of a report as the identity M1 compares: every field, the report's own order. */
export const identity = (report) => JSON.stringify(report.findings ?? []);

/** One checker run of a prepared project: the parsed report. */
export function check(bin, { tsconfig, cwd, args, typefacts, ablate, proofLog }) {
  return new Promise((done, fail) => {
    const env = { ...process.env, SOLID_CHECKER_DAEMON: "0", SOLID_TYPEFACTS_BIN: typefacts, SOLID_CHECKER_RUNTIME_RESOLVER: process.env.SOLID_CHECKER_RUNTIME_RESOLVER ?? join(ROOT, "packages/cli/scripts/runtime-resolver.mjs") };
    delete env.SOLID_CHECKER_IR_ABLATE_ARM;
    delete env.SOLID_CHECKER_IR_ABLATE_SILENCE;
    if (ablate?.arms) env.SOLID_CHECKER_IR_ABLATE_ARM = ablate.arms;
    if (ablate?.silences) env.SOLID_CHECKER_IR_ABLATE_SILENCE = ablate.silences;
    delete env.SOLID_CHECKER_IR_PROOF_LOG;
    if (proofLog) env.SOLID_CHECKER_IR_PROOF_LOG = proofLog;
    const child = spawn(bin, ["--project", tsconfig, ...args], { cwd, env, stdio: ["ignore", "pipe", "ignore"] });
    let stdout = "";
    child.stdout.on("data", (chunk) => { stdout += chunk; });
    child.on("error", fail);
    child.on("close", () => {
      try { done(JSON.parse(stdout)); } catch { fail(new Error(`${bin}: no JSON report for ${tsconfig}`)); }
    });
  });
}

/** The programs of a `--set` value: `{ id, prepare(): { tsconfig, cwd, args, root, cleanup } }`. */
export async function programsOf(set, { install }) {
  if (set === "fixtures") {
    return fixtureProjects(ROOT).map((project) => ({ id: project.id, prepare: () => ({ tsconfig: project.tsconfig, cwd: ROOT, args: ["--format", "json", ...runtimeArguments(project.tsconfig)], root: project.directory, cleanup() {} }) }));
  }
  const { materializeFromDirectory, materializeForChecker } = await import("./fuzz/checker.mjs");
  const work = (dir) => ({ tsconfig: join(dir, "tsconfig.json"), cwd: dir, args: ORDINARY, root: dir, cleanup: () => rmSync(dir, { recursive: true, force: true }) });
  if (set === "textbook") {
    const base = join(ROOT, "fixtures", "textbook");
    return readdirSync(base, { withFileTypes: true }).filter((entry) => entry.isDirectory()).map((entry) => entry.name).sort()
      .map((name) => ({ id: `textbook/${name}`, prepare: () => work(materializeFromDirectory(join(base, name), install, `monotone-textbook-${name}`)) }));
  }
  const grid = /^grid:([a-z]+-[a-z]+)(?::(\d+))?$/.exec(set);
  if (!grid) throw new Error(`unknown --set ${set} (textbook, fixtures, grid:<grid>[:<n>])`);
  const { gridRows } = await import("./fuzz/cover.mjs");
  const { buildRow } = await import("./fuzz/blocker-scan.mjs");
  const { fnv } = await import("./fuzz/rng.mjs");
  let rows = gridRows(grid[1]);
  if (grid[2]) rows = rows.map((row) => ({ row, rank: fnv(`monotone:${row.id}`) })).sort((a, b) => a.rank - b.rank || (a.row.id < b.row.id ? -1 : 1)).slice(0, Number(grid[2])).map(({ row }) => row);
  return rows.map((row) => ({ id: `${grid[1]}/${row.id}`, prepare: () => work(materializeForChecker(buildRow(row), install, `monotone-${row.id}`)) }));
}

async function main(argv) {
  const values = (name) => argv.flatMap((arg, at) => (arg === name ? [argv[at + 1]] : []));
  const value = (name) => values(name)[0];
  const base = value("--base"), head = value("--head"), typefacts = value("--typefacts"), out = value("--out");
  if (!base || !head || !typefacts || !out) throw new Error("usage: monotone.mjs --base <checker> --head <checker> --typefacts <bin> --set <set> [--set ...] [--ablate-arm a,b] [--ablate-silence s,t] --out <report.json>");
  if (existsSync(out)) throw new Error(`use a fresh --out path (${out} exists)`);
  const ablate = value("--ablate-arm") || value("--ablate-silence") ? { arms: value("--ablate-arm"), silences: value("--ablate-silence") } : null;
  const baseAblated = argv.includes("--base-ablated");
  if (baseAblated && !ablate) throw new Error("--base-ablated needs --ablate-arm or --ablate-silence");
  const arms = new Set([...(value("--ablate-arm") ?? "").split(","), ...(value("--ablate-silence") ?? "").split(",")].filter(Boolean));
  const install = resolve(value("--install") ?? join(ROOT, "scripts", "fuzz", "install"));
  const programs = [];
  for (const set of values("--set")) programs.push(...(await programsOf(set, { install })));
  if (!programs.length) throw new Error("no programs: name a --set");
  const started = Date.now();
  const results = await mapPoolSettled(programs, async (program) => {
    const project = program.prepare();
    try {
      const run = (bin, ablation, proofLog) => check(bin, { ...project, typefacts, ablate: ablation, proofLog });
      const before = await run(base, baseAblated ? ablate : null);
      // M2b: the head's own run writes the proof log when the change names its arms.
      const log = ablate ? mkdtempSync(join(tmpdir(), "monotone-log-")) : null;
      let after, lines = [];
      try {
        after = await run(head, null, log);
        if (log) for (const name of readdirSync(log)) lines.push(...namingLines(readFileSync(join(log, name), "utf8"), project.root));
      } finally {
        if (log) rmSync(log, { recursive: true, force: true });
      }
      const ablated = ablate ? await run(head, ablate) : null;
      const m2 = moves(siteCodes(before, project.root), siteCodes(after, project.root));
      const unnamed = ablate ? unnamedUp(m2.up, lines, arms) : [];
      return { id: program.id, m1: ablated ? identity(ablated) === identity(before) : null, up: m2.up, bad: m2.bad, unnamed, status: [before.status, after.status] };
    } finally {
      project.cleanup();
    }
  }, { concurrency: Number(value("--concurrency") ?? 4) });
  const report = { base, head, ablate, sets: values("--set"), programs: programs.length, seconds: Math.round((Date.now() - started) / 1000), errors: [], m1Fails: [], m2Fails: [], m2bFails: [], up: 0, statusUp: 0 };
  for (const result of results) {
    if (result.error) { report.errors.push({ id: programs[result.index].id, error: String(result.error.message ?? result.error).slice(0, 300) }); continue; }
    const { id, m1, up, bad, unnamed, status } = result.value;
    if (unnamed.length) report.m2bFails.push({ id, unnamed });
    if (m1 === false) report.m1Fails.push(id);
    if (bad.length) report.m2Fails.push({ id, bad });
    report.up += up.length;
    if (status[0] === "uncertifiable" && status[1] === "certified") report.statusUp += 1;
  }
  writeFileSync(out, `${JSON.stringify(report, null, 2)}\n`);
  console.log(`monotone: ${report.programs} programs in ${report.seconds} s; M1 ${ablate ? `${report.m1Fails.length} not identical` : "not run (no ablation)"}; M2 ${report.m2Fails.length} programs with a downward or sideways move, ${report.up} (site, code) moved up, ${report.statusUp} programs U -> certified; M2b ${ablate ? `${report.m2bFails.length} programs with an up move no named arm explains` : "not run (no arms named)"}; ${report.errors.length} errors`);
  for (const { id, unnamed } of report.m2bFails.slice(0, 10)) console.log(`  M2b: ${id}: ${unnamed.slice(0, 3).map((move) => `${move.site} ${move.from} -> ${move.to}`).join("; ")}`);
  for (const id of report.m1Fails.slice(0, 10)) console.log(`  M1: ${id}: the ablated head is not the base`);
  for (const { id, bad } of report.m2Fails.slice(0, 10)) console.log(`  M2: ${id}: ${bad.slice(0, 3).map((move) => `${move.site} ${move.from} -> ${move.to}`).join("; ")}`);
  for (const { id, error } of report.errors.slice(0, 5)) console.log(`  error: ${id}: ${error}`);
  process.exitCode = report.m1Fails.length || report.m2Fails.length || report.m2bFails.length || report.errors.length ? 1 : 0;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main(process.argv.slice(2)).catch((error) => { console.error(error.message); process.exitCode = 2; });
