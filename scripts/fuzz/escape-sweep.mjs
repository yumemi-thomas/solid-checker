#!/usr/bin/env node
// ADR 1152: which verdicts change when every name of a program is written with an escape (the `escape` rewrite, escape.mjs).
//
//   node scripts/fuzz/escape-sweep.mjs --checker <solid-checker> --typefacts <solid-typefacts> --set textbook --set fixtures --set grid:lg-base:400 \
//        [--deps] [--keywords] [--specifiers] [--no-attribute] [--concurrency 4] [--install scripts/fuzz/install] --out <report.json>
//
// ECMAScript reads an identifier's StringValue and a string's or a template piece's cooked value, so a program with its names written `a`, `\u{61}`, `\x61` or `\a` does exactly what
// the program does, and no verdict may depend on which spelling it has: a check that compares the source text of a name (`source_text(span) == Some("addEventListener")`) and not its
// value answers differently on the two. This runs the checker on a program and on the program with every name escaped, and compares the whole report (the file, line, code, kind and blocker
// of every finding; no message: a message may quote the name as written), not only the sites of interest: a row that vanishes (a listener-reach obligation) is the signal too.
//
// A program that differs is attributed: a delta-debugging search (`minimize`) over its distinct names finds the smallest set of names whose escape alone reproduces the difference, so the
// count is by the name a check reads wrongly, not by the program. A name is a cooked text (an identifier, a string or a template piece).
//
// `--set` as monotone.mjs: `textbook`, `fixtures`, `grid:<grid>[:<n>]`. `--deps` also escapes the project's dependencies' sources (node_modules, but not Solid's own runtime packages);
// `--keywords` escapes only the identifiers spelled like a TypeScript keyword (`type`, `get`, `async`, `of`...: a TS1260 to the type checker, so a pass of their own);
// `--specifiers` also escapes module specifiers.
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { mapPoolSettled } from "../lib/pool.mjs";
import { check, programsOf } from "../monotone.mjs";
import { ignoredRules } from "./codes.mjs";
import { lastLine } from "./finding-lines.mjs";
import { TS_KEYWORDS, escapable, escapeSource, sameProgram } from "./escape.mjs";

const SKIPPED_DIRECTORIES = new Set([".git", ".cache", ".solid-checker-fuzz-fast", "target"]);
/** Solid's own runtime packages: read by no check (ADR 0027), so an escape there answers nothing. */
const RUNTIME_PACKAGES = ["solid-js", "@solidjs"];

/** Every file under `directory`, relative, skipping what no source is in. */
function* filesUnder(directory, base = directory) {
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    if (entry.isDirectory()) { if (!SKIPPED_DIRECTORIES.has(entry.name)) yield* filesUnder(join(directory, entry.name), base); }
    else if (entry.isFile()) yield relative(base, join(directory, entry.name));
  }
}

const inRuntimePackage = (path) => RUNTIME_PACKAGES.some((name) => path.includes(`node_modules/${name}/`));
const isDependency = (path) => path.split("/").includes("node_modules");

/**
 * A copy of a project directory. A project inside the fuzz install's work area resolves Solid's packages from the install above it, so its copy is a sibling (the same parent): a copy
 * in the temporary directory would resolve none of them and every verdict would move. A fixture carries its own `node_modules`: its copy is in the temporary directory.
 */
export function copyProject(root) {
  const inInstall = root.includes(`${join("scripts", "fuzz", "install")}/`);
  const copy = inInstall ? mkdtempSync(join(dirname(root), `${basename(root)}-esc-`)) : mkdtempSync(join(tmpdir(), "escape-sweep-"));
  cpSync(root, copy, { recursive: true, filter: (source) => source === root || !SKIPPED_DIRECTORIES.has(basename(source)) });
  return copy;
}

/**
 * Writes the escape into the sources of the directory `root`: `names` the cooked texts to escape (default every one), `deps` whether the dependencies' sources too.
 * Returns `{ names: Map<cooked text, count>, files, unparsed: [path], changed }` (the names found in the files that were read, whether or not escaped).
 */
export function escapeProject(root, { names = null, deps = false, keywords = false, specifiers = false } = {}) {
  const found = new Map();
  const unparsed = [];
  let files = 0;
  let changed = 0;
  const roles = ["identifier", "string", "template", ...(specifiers ? ["specifier"] : [])];
  for (const path of filesUnder(root)) {
    if (!escapable(path) || (isDependency(path) && (!deps || inRuntimePackage(path)))) continue;
    const full = join(root, path);
    const text = readFileSync(full, "utf8");
    // The names each file offers, then the escape of those chosen: one parse for the pass that only counts.
    const read = escapeSource(text, { filename: path, names: names ? new Set(names) : null, roles, keywords });
    files += 1;
    if (!read.parsed) { unparsed.push(path); continue; }
    if (read.text !== text) {
      if (!sameProgram(text, read.text, { filename: path })) throw new Error(`the escape changed the program: ${path}`);
      writeFileSync(full, read.text);
      changed += 1;
    }
    for (const [name, count] of read.escaped) found.set(name, (found.get(name) ?? 0) + count);
  }
  return { names: found, files, unparsed, changed };
}

/** The finding keys of a report: file (relative), line, code, kind, what blocks it (the blocker of an obligation, the rule of a violation) and how many fixes it offers, sorted. No message, no column. */
export function reportKeys(report, root) {
  // The checker reports canonical paths (`/private/var/...`), a temporary directory's name may be a link to one (`/var/...`): both sides are resolved.
  const real = (path) => { try { return realpathSync(path); } catch { return path; } };
  const base = real(root);
  return (report.findings ?? []).map((finding) => {
    const location = finding.primaryLocation ?? {};
    const file = location.path ? relative(base, real(location.path)) : "";
    const blocker = String(finding.analysisContext ?? "").split(":")[0];
    // The fixes a finding offers are part of it (how many: their text spells the name as the program does).
    const fixes = Array.isArray(finding.fixes) && finding.fixes.length ? `|fixes${finding.fixes.length}` : "";
    return `${file}:${location.line ?? 0}|${finding.id}|${finding.kind}|${finding.kind === "uncertifiable" ? blocker : finding.rule ?? ""}${fixes}`;
  }).sort();
}

/** The context text of each finding (`function-run-unplaced:Probe`): a name in it may be the source's spelling instead of its value. */
export function contextTexts(report) {
  return (report.findings ?? []).map((finding) => String(finding.analysisContext ?? "")).sort();
}

/** The findings of a report as sites: `{ file (relative to `root`), line, endLine, id, kind }` with kind `V` (a violation) or `U` (an obligation); the rules the harness ignores are left out. */
export function findingRows(report, root) {
  const real = (path) => { try { return realpathSync(path); } catch { return path; } };
  const base = real(root);
  return (report.findings ?? []).flatMap((finding) => {
    const kind = finding.kind === "violation" ? "V" : finding.kind === "uncertifiable" ? "U" : null;
    // A style rule the oracle never holds against the runtime is no verdict of the fuzz harness (`siteVerdicts`): it is no site's.
    if (!kind || ignoredRules.includes(finding.rule)) return [];
    const location = finding.primaryLocation ?? {};
    const line = location.line ?? 0;
    return [{ file: location.path ? relative(base, real(location.path)) : "", line, endLine: lastLine(finding), id: finding.id, kind }];
  });
}

/** Whether two sites share a line of a file (a finding's range holds the lines of its span: the fuzz harness reads a site's verdict from the findings that hold its line). */
const overlaps = (a, b) => a.file === b.file && a.line <= b.endLine && b.line <= a.endLine;

/** The moves that a veto must leave none of: a violation lost silently, a violation gained. */
const MUST_BE_EMPTY = new Set(["V->clean", "U->V", "none->V"]);

/**
 * How the sites of a program move when its names are written with escapes, read as the fuzz harness reads a verdict (the findings whose range holds the site's line):
 * an original violation is kept (`V->V`: a violation of its code still holds the site), obligated (`V->U`: only an obligation holds it, a file-wide notice included), or clean
 * (`V->clean`: nothing holds it: a violation lost silently); an original obligation is `U->U` or `U->clean`; a violation of the escaped program that no violation of its code
 * held in the original is gained, `U->V` where the original held an obligation there and `none->V` where it held nothing (a possible false positive). `bad` lists, by class, a few
 * instances of the classes that must be empty (`V->clean`, `U->V`, `none->V`).
 */
export function moves(original, escaped) {
  const counts = { "V->V": 0, "V->U": 0, "V->clean": 0, "U->U": 0, "U->clean": 0, "U->V": 0, "none->V": 0 };
  const bad = {};
  const note = (name, row) => {
    counts[name] += 1;
    if (!MUST_BE_EMPTY.has(name)) return;
    const list = (bad[name] ??= []);
    if (list.length < 3) list.push(`${row.file}:${row.line}|${row.id}`);
  };
  for (const row of original) {
    const held = escaped.filter((other) => overlaps(row, other));
    if (row.kind === "V") note(held.some((other) => other.kind === "V" && other.id === row.id) ? "V->V" : held.length ? "V->U" : "V->clean", row);
    else note(held.length ? "U->U" : "U->clean", row);
  }
  for (const row of escaped.filter((candidate) => candidate.kind === "V")) {
    if (original.some((other) => other.kind === "V" && other.id === row.id && overlaps(row, other))) continue;
    note(original.some((other) => overlaps(row, other)) ? "U->V" : "none->V", row);
  }
  return { counts, bad };
}

/** The multiset difference of two sorted key lists: what the first has that the second lacks, and the reverse. */
export function difference(a, b) {
  const count = (keys) => keys.reduce((map, key) => map.set(key, (map.get(key) ?? 0) + 1), new Map());
  const left = count(a);
  const right = count(b);
  const lost = [];
  const gained = [];
  for (const [key, n] of left) for (let i = 0; i < n - (right.get(key) ?? 0); i += 1) lost.push(key);
  for (const [key, n] of right) for (let i = 0; i < n - (left.get(key) ?? 0); i += 1) gained.push(key);
  return { lost, gained };
}

/** What one key says: `violation` or `uncertifiable`. */
const kindOf = (key) => key.split("|")[2];
/** The code of one key. */
export const codeOf = (key) => key.split("|")[1];

/**
 * Classifies a difference. `lost-violation`: an original violation the escaped program lacks (a defect the escape hid); `lost-obligation`: an uncertifiable row it lacks (a proof the
 * checker claims only for the escaped spelling: a possible silent clean); `gained-violation`: a violation only the escaped program has (a possible false positive);
 * `gained-obligation`: an obligation only it has (the stricter reading).
 */
export function classify({ lost, gained }) {
  const classes = new Set();
  for (const key of lost) classes.add(kindOf(key) === "violation" ? "lost-violation" : "lost-obligation");
  for (const key of gained) classes.add(kindOf(key) === "violation" ? "gained-violation" : "gained-obligation");
  return [...classes].sort();
}

/**
 * Delta debugging over `names`: the smallest subset (1-minimal: no name can be left out) for which `diverges(subset)` is still true. `diverges` is asked of the whole list first by
 * the caller; a search that finds no smaller subset returns it whole. At most `limit` questions (the search stops and returns what it has).
 */
export async function minimize(names, diverges, { limit = 60 } = {}) {
  let current = [...names];
  let granularity = 2;
  let asked = 0;
  const ask = async (subset) => { asked += 1; return diverges(subset); };
  while (current.length >= 2 && asked < limit) {
    const size = Math.ceil(current.length / granularity);
    const chunks = [];
    for (let at = 0; at < current.length; at += size) chunks.push(current.slice(at, at + size));
    let reduced = false;
    for (const chunk of chunks) {
      if (asked >= limit) break;
      if (await ask(chunk)) { current = chunk; granularity = 2; reduced = true; break; }
    }
    if (!reduced && granularity > 2) {
      for (const chunk of chunks) {
        if (asked >= limit) break;
        const complement = current.filter((name) => !chunk.includes(name));
        if (complement.length && await ask(complement)) { current = complement; granularity = Math.max(granularity - 1, 2); reduced = true; break; }
      }
    }
    if (!reduced) {
      if (granularity >= current.length) break;
      granularity = Math.min(current.length, granularity * 2);
    }
  }
  return { names: current, asked };
}

/** A prepared project with its directory replaced: the same layout, in `directory`. */
function relocate(project, directory) {
  const inside = (path) => (path.startsWith(`${project.root}/`) || path === project.root ? join(directory, relative(project.root, path)) : path);
  return { ...project, tsconfig: inside(project.tsconfig), cwd: inside(project.cwd), root: directory, cleanup: () => rmSync(directory, { recursive: true, force: true }) };
}

async function sweepProgram(program, options) {
  const project = program.prepare();
  const variants = [];
  const row = { id: program.id, status: "ok" };
  try {
    const run = (target) => check(options.checker, { ...target, typefacts: options.typefacts, args: target.args });
    // A project the checker could not read (no report) is not a program of this sweep.
    // The pass of every name but the TypeScript keywords' (type-clean), or the pass of those alone (`--keywords`).
    const universe = options.keywords ? [...TS_KEYWORDS] : null;
    const variantOf = (names) => {
      const directory = copyProject(project.root);
      const target = relocate(project, directory);
      variants.push(target);
      return { target, ...escapeProject(directory, { names: names ?? universe, deps: options.deps, keywords: options.keywords, specifiers: options.specifiers }) };
    };
    // The baseline is the program as a copy with nothing escaped, not the program in place: a project that reads its surroundings (a fixture's shared files) moves by being copied, and
    // that is no escape. Both sides are copies of the same layout (`--control` compares the baseline with another such copy).
    const baseline = variantOf([]);
    const original = await run(baseline.target);
    const originalKeys = reportKeys(original, baseline.target.root);
    const originalRows = findingRows(original, baseline.target.root);
    rmSync(baseline.target.root, { recursive: true, force: true });
    const all = variantOf(options.control ? [] : null);
    row.names = all.names.size;
    row.escapedFiles = all.changed;
    if (all.unparsed.length) row.unparsed = all.unparsed;
    // `--control`: the copy with nothing escaped, to show that the copy is no change (every program `same`).
    if (!all.changed && !options.control) return { ...row, status: "nothing-escaped" };
    // `--trace <dir>`: a build with the raw-name trace says which Rust sites read an escaped name as written (every program, whether or not a verdict moved).
    const traceFile = options.trace ? join(options.trace, `${program.id.replace(/[^A-Za-z0-9_.-]/g, "_")}.tsv`) : undefined;
    // `check` builds the child's environment and spawns it before it returns its promise, so the trace file of this run is set around the call and no other run reads it.
    if (traceFile) process.env.SOLID_CHECKER_RAW_NAME_TRACE = traceFile;
    const traced = check(options.checker, { ...all.target, typefacts: options.typefacts, args: all.target.args });
    delete process.env.SOLID_CHECKER_RAW_NAME_TRACE;
    const escaped = await traced;
    if (traceFile) row.sites = readTrace(traceFile);
    const delta = difference(originalKeys, reportKeys(escaped, all.target.root));
    row.moves = moves(originalRows, findingRows(escaped, all.target.root));
    row.status = delta.lost.length || delta.gained.length ? "diverges" : (JSON.stringify(contextTexts(original)) === JSON.stringify(contextTexts(escaped)) ? "same" : "context-only");
    row.originalStatus = original.status;
    row.escapedStatus = escaped.status;
    if (row.status === "context-only") row.contexts = { original: contextTexts(original), escaped: contextTexts(escaped) };
    if (row.status !== "diverges") return row;
    row.classes = classify(delta);
    row.lost = delta.lost;
    row.gained = delta.gained;
    // Attribution costs a dozen runs a program: the first `--attribute-max` diverging programs of a run are attributed (a grid's thousands move for a few names).
    if (options.attribute && (options.attributed.count += 1) <= options.attributeMax) {
      const names = [...all.names.keys()].sort();
      const diverges = async (subset) => {
        const probe = variantOf(subset);
        const report = await run(probe.target);
        const d = difference(originalKeys, reportKeys(report, probe.target.root));
        rmSync(probe.target.root, { recursive: true, force: true });
        return d.lost.length > 0 || d.gained.length > 0;
      };
      const minimal = await minimize(names, diverges, { limit: options.limit });
      row.minimal = minimal.names;
      row.asked = minimal.asked;
    }
    return row;
  } finally {
    for (const target of variants) rmSync(target.root, { recursive: true, force: true });
    project.cleanup?.();
  }
}

/** The distinct reads a trace file holds: `[{ site, text }]` (a site is the Rust file and line of the read). */
export function readTrace(file) {
  if (!existsSync(file)) return [];
  const seen = new Map();
  for (const line of readFileSync(file, "utf8").split("\n")) {
    const [site, , text] = line.split("\t");
    if (site && text !== undefined) seen.set(`${site}\t${text}`, { site, text });
  }
  return [...seen.values()];
}

/** The sites' moves of every program summed (`moves`), and the programs of each class that must be empty. */
export function moveTotals(rows) {
  const totals = {};
  const bad = {};
  for (const row of rows) {
    if (!row.moves) continue;
    for (const [name, n] of Object.entries(row.moves.counts)) totals[name] = (totals[name] ?? 0) + n;
    for (const [name, examples] of Object.entries(row.moves.bad)) (bad[name] ??= []).push({ id: row.id, examples });
  }
  return { totals, bad };
}

/** The report's tallies: programs by status, divergences by class and code, and by the names that reproduce them. */
export function summarize(rows) {
  const count = (list) => list.reduce((map, key) => map.set(key, (map.get(key) ?? 0) + 1), new Map());
  const sorted = (map) => [...map].sort((a, b) => b[1] - a[1] || (a[0] < b[0] ? -1 : 1));
  const diverging = rows.filter((row) => row.status === "diverges");
  const byName = new Map();
  for (const row of diverging) for (const name of row.minimal ?? []) byName.set(name, [...(byName.get(name) ?? []), row.id]);
  return {
    programs: rows.length,
    byStatus: sorted(count(rows.map((row) => row.status))),
    moves: moveTotals(rows),
    byClass: sorted(count(diverging.flatMap((row) => row.classes))),
    byCode: sorted(count(diverging.flatMap((row) => [...row.lost, ...row.gained].map(codeOf)))),
    byMinimalSet: sorted(count(diverging.map((row) => (row.minimal ?? ["?"]).join(" + ")))),
    bySite: siteTable(rows),
    byName: [...byName].map(([name, ids]) => ({ name, programs: ids.length, examples: ids.slice(0, 3) })).sort((a, b) => b.programs - a.programs || (a.name < b.name ? -1 : 1)),
  };
}

/** The Rust sites that read an escaped name as written: programs that read at each (and how many of those moved a verdict), with a few of the texts read. */
export function siteTable(rows) {
  const sites = new Map();
  for (const row of rows) {
    for (const { site, text } of row.sites ?? []) {
      const slot = sites.get(site) ?? { site, programs: new Set(), diverging: new Set(), texts: new Set() };
      slot.programs.add(row.id);
      if (row.status === "diverges") slot.diverging.add(row.id);
      if (slot.texts.size < 6) slot.texts.add(text);
      sites.set(site, slot);
    }
  }
  return [...sites.values()].map((slot) => ({ site: slot.site, programs: slot.programs.size, diverging: slot.diverging.size, texts: [...slot.texts], examples: [...slot.programs].slice(0, 3) }))
    .sort((a, b) => b.programs - a.programs || (a.site < b.site ? -1 : 1));
}

async function main(argv) {
  const values = (name) => argv.flatMap((arg, at) => (arg === name ? [argv[at + 1]] : []));
  const value = (name) => values(name)[0];
  const checker = value("--checker"), typefacts = value("--typefacts"), out = value("--out");
  if (!checker || !typefacts || !out || !values("--set").length) throw new Error("usage: escape-sweep.mjs --checker <bin> --typefacts <bin> --set <set> [--set ...] [--deps] [--keywords] [--specifiers] [--no-attribute] --out <report.json>");
  const options = {
    checker: resolve(checker), typefacts: resolve(typefacts), deps: argv.includes("--deps"), specifiers: argv.includes("--specifiers"), attribute: !argv.includes("--no-attribute"),
    limit: Number(value("--limit") ?? 60), attributeMax: Number(value("--attribute-max") ?? 150), attributed: { count: 0 },
    // The keyword pass escapes only the identifiers a TypeScript keyword is spelled like.
    keywords: argv.includes("--keywords"), control: argv.includes("--control"),
    trace: value("--trace") ? resolve(value("--trace")) : undefined,
  };
  if (options.trace) { rmSync(options.trace, { recursive: true, force: true }); mkdirSync(options.trace, { recursive: true }); }
  const install = resolve(value("--install") ?? join(dirname(fileURLToPath(import.meta.url)), "install"));
  const programs = [];
  for (const set of values("--set")) programs.push(...(await programsOf(set, { install })));
  const started = Date.now();
  const settled = await mapPoolSettled(programs, (program) => sweepProgram(program, options), { concurrency: Number(value("--concurrency") ?? 4) });
  const rows = settled.map((result, index) => (result.error ? { id: programs[index].id, status: "error", error: String(result.error.message ?? result.error).slice(0, 300) } : result.value));
  const summary = summarize(rows);
  writeFileSync(out, `${JSON.stringify({ checker: options.checker, sets: values("--set"), deps: options.deps, keywords: argv.includes("--keywords"), specifiers: options.specifiers, seconds: Math.round((Date.now() - started) / 1000), summary, rows }, null, 1)}\n`);
  console.log(`escape-sweep: ${summary.programs} programs in ${Math.round((Date.now() - started) / 1000)} s`);
  console.log(`  by status: ${summary.byStatus.map(([status, n]) => `${status} ${n}`).join(", ")}`);
  console.log(`  site moves: ${Object.entries(summary.moves.totals).map(([name, n]) => `${name} ${n}`).join(", ") || "none"}`);
  for (const [name, list] of Object.entries(summary.moves.bad)) console.log(`  ${name}: ${list.length} program(s), e.g. ${list.slice(0, 4).map((entry) => `${entry.id} ${entry.examples.join(" ")}`).join("; ")}`);
  // `--expect-veto` (ADR 1152): a program that writes a name with an escape is not proven, so it must hold no gained violation and lose no violation silently.
  if (argv.includes("--expect-veto") && Object.keys(summary.moves.bad).length) { console.log("  expected no V->clean and no gained violation"); process.exitCode = 1; }
  if (summary.byClass.length) console.log(`  diverging, by class: ${summary.byClass.map(([name, n]) => `${name} ${n}`).join(", ")}`);
  if (summary.byCode.length) console.log(`  rows moved, by code: ${summary.byCode.map(([name, n]) => `${name} ${n}`).join(", ")}`);
  for (const entry of summary.bySite.slice(0, 60)) console.log(`  site ${entry.site}: ${entry.programs} program(s) (${entry.diverging} diverging), reads ${entry.texts.slice(0, 4).map((text) => JSON.stringify(text)).join(" ")}`);
  for (const entry of summary.byName.slice(0, 25)) console.log(`  name ${JSON.stringify(entry.name)}: ${entry.programs} program(s), e.g. ${entry.examples.join(", ")}`);
  for (const row of rows.filter((candidate) => candidate.status === "error").slice(0, 5)) console.log(`  error: ${row.id}: ${row.error}`);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main(process.argv.slice(2)).catch((error) => { console.error(error.stack ?? error.message); process.exitCode = 2; });
