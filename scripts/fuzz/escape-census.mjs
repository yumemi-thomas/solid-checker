#!/usr/bin/env node
// ADR 1152: how many files write a name with an escape.
//
//   node scripts/fuzz/escape-census.mjs --root <label>=<dir> [--root ...] [--shard i/n] --out <report.json>
//   node scripts/fuzz/escape-census.mjs --merge <report.json> <report.json> ... --out <merged.json>
//
// A name is its cooked value (`eval` is `eval`), so a program that writes one with an escape is read wrongly by any check that compares source text. This counts, per root and
// per kind of file (a project's own sources, a dependency's under `node_modules`), the files that write:
//   - an identifier with an escape (`eval`): the one case where the escape is always a name;
//   - a string or template used as a name (a property key, a computed member's literal key, an import or export name) with an escape (`{ "value": 1 }`);
//   - a string or template whose whole cooked value is an identifier-like word, with an escape (`"value"`), wherever it stands: it can be read as a name;
//   - any other string or template with an escape (`"\n"`, `"café"`, `"a\tb"`): common, and it matters only where a string is read as a name, so it is counted apart.
// A file is parsed (Babel, the fuzzer's pinned install) only when its text holds `\u` or `\x` followed by hex digits: an identifier can be written with an escape only as `\u...`, and a
// string almost only as `\u`, `\x` or one of the escapes of one character (`\n`, `\"`) that no name holds. A file with an escape of a character that needs none (`"\e"`) or an octal one
// (`"\141"`) is not counted (it is how no minifier and almost no person writes a name). `files with a backslash` is the cheap upper bound (regular expressions and `\n` included).
import { spawnSync } from "node:child_process";
import { readFileSync, statSync, writeFileSync } from "node:fs";
import { relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { escapeCandidates } from "./escape.mjs";

const GLOBS = ["*.{js,mjs,cjs,jsx,ts,tsx,mts,cts}"];
const MAX_BYTES = 6 * 1024 * 1024;
const NAME_LIKE = /^[A-Za-z_$][A-Za-z0-9_$]*$/;

/** The files under `directory` that match, by `rg` (a measurement tool, not a gate: ripgrep is a development tool of this repository). */
function rg(args, directory) {
  const run = spawnSync("rg", ["--no-ignore", "--hidden", "-g", GLOBS[0], "-g", "!**/.git/**", ...args, directory], { encoding: "utf8", maxBuffer: 1 << 30 });
  if (run.status !== 0 && run.status !== 1) throw new Error(`rg failed: ${run.stderr}`);
  return run.stdout.split("\n").filter(Boolean);
}

const isDependency = (path) => path.split("/").includes("node_modules");
const isBuildOutput = (path) => !isDependency(path) && /\/(dist|build|\.output|\.next|\.vinxi|\.solid|\.svelte-kit|\.turbo|coverage)\//.test(path);
/** The kind of file: a dependency's, a build output's, or a project's own source. */
export const kindOf = (path) => (isDependency(path) ? "dependency" : isBuildOutput(path) ? "build-output" : "project");

/** A file that is minified by its shape: a very long line. */
const minified = (text) => text.length > 2000 && (text.length / Math.max(1, text.split("\n").length) > 300 || text.split("\n", 400).some((line) => line.length > 5000));

/** What a source writes with an escape: token counts by kind. */
export function censusOf(source, filename) {
  const places = escapeCandidates(source, { filename, keywords: true });
  if (!places) return null;
  const count = { identifier: 0, keyString: 0, nameString: 0, otherString: 0, specifier: 0 };
  const names = new Set();
  for (const place of places) {
    const raw = source.slice(place.start, place.end);
    if (!raw.includes("\\")) continue;
    if (place.role === "identifier") { count.identifier += 1; names.add(place.name); continue; }
    if (place.role === "specifier") { count.specifier += 1; continue; }
    if (place.keyed) { count.keyString += 1; names.add(place.name); continue; }
    if (NAME_LIKE.test(place.name)) { count.nameString += 1; names.add(place.name); continue; }
    count.otherString += 1;
  }
  return { count, names: [...names].slice(0, 6) };
}

function emptyKind() {
  return { files: 0, withBackslash: 0, candidates: 0, parsed: 0, unparsed: 0, skippedLarge: 0, minifiedHits: 0, identifier: { files: 0, tokens: 0 }, keyString: { files: 0, tokens: 0 }, nameString: { files: 0, tokens: 0 }, otherString: { files: 0, tokens: 0 }, specifier: { files: 0, tokens: 0 }, anyName: { files: 0 }, examples: { identifier: [], keyString: [], nameString: [] } };
}

async function census(roots, shard) {
  const report = {};
  for (const { label, directory } of roots) {
    const kinds = { project: emptyKind(), "build-output": emptyKind(), dependency: emptyKind() };
    for (const path of rg(["--files"], directory)) kinds[kindOf(path)].files += 1;
    for (const path of rg(["-l", "-e", "\\\\"], directory)) kinds[kindOf(path)].withBackslash += 1;
    const candidates = [...new Set([...rg(["-l", "-e", String.raw`\\u[0-9A-Fa-f{]`], directory), ...rg(["-l", "-e", String.raw`\\x[0-9A-Fa-f]{2}`], directory)])].sort();
    const mine = candidates.filter((_, index) => index % shard.of === shard.index);
    for (const path of mine) {
      const kind = kinds[kindOf(path)];
      kind.candidates += 1;
      let text;
      try { if (statSync(path).size > MAX_BYTES) { kind.skippedLarge += 1; continue; } text = readFileSync(path, "utf8"); } catch { kind.unparsed += 1; continue; }
      const found = censusOf(text, path);
      if (!found) { kind.unparsed += 1; continue; }
      kind.parsed += 1;
      let any = false;
      for (const name of ["identifier", "keyString", "nameString", "otherString", "specifier"]) {
        if (!found.count[name]) continue;
        kind[name].files += 1;
        kind[name].tokens += found.count[name];
        if (name !== "otherString" && name !== "specifier") any = true;
        if (kind.examples[name] && kind.examples[name].length < 12) kind.examples[name].push({ file: relative(directory, path), tokens: found.count[name], names: found.names, minified: minified(text) });
      }
      if (any) { kind.anyName.files += 1; if (minified(text)) kind.minifiedHits += 1; }
    }
    report[label] = kinds;
  }
  return report;
}

/** Adds the shards' reports: the counts are sums (the file totals are the same in every shard and are taken once). */
export function merge(reports) {
  const out = {};
  reports.forEach((report, at) => {
    for (const [label, kinds] of Object.entries(report)) {
      out[label] ??= {};
      for (const [kind, row] of Object.entries(kinds)) {
        const target = (out[label][kind] ??= emptyKind());
        if (at === 0) { target.files = row.files; target.withBackslash = row.withBackslash; }
        for (const key of ["candidates", "parsed", "unparsed", "skippedLarge", "minifiedHits"]) target[key] += row[key];
        for (const key of ["identifier", "keyString", "nameString", "otherString", "specifier"]) { target[key].files += row[key].files; target[key].tokens += row[key].tokens; }
        target.anyName.files += row.anyName.files;
        for (const key of Object.keys(target.examples)) target.examples[key] = [...target.examples[key], ...row.examples[key]].slice(0, 12);
      }
    }
  });
  return out;
}

function print(report) {
  for (const [label, kinds] of Object.entries(report)) {
    for (const [kind, row] of Object.entries(kinds)) {
      if (!row.files) continue;
      console.log(`${label} / ${kind}: ${row.files} source files; ${row.withBackslash} with a backslash; ${row.candidates} with \\u or \\x (${row.parsed} parsed, ${row.unparsed} unparsed, ${row.skippedLarge} over ${MAX_BYTES >> 20} MB)`);
      console.log(`    an identifier written with an escape: ${row.identifier.files} files (${row.identifier.tokens} tokens)`);
      console.log(`    a string/template used as a key or export/import name, with an escape: ${row.keyString.files} files (${row.keyString.tokens} tokens)`);
      console.log(`    a string/template whose whole value is a word, with an escape: ${row.nameString.files} files (${row.nameString.tokens} tokens)`);
      console.log(`    any of those three: ${row.anyName.files} files (${row.minifiedHits} of them minified by shape)`);
      console.log(`    another string/template with an escape (\\n, \\u00e9, ...): ${row.otherString.files} files (${row.otherString.tokens} tokens); a module specifier with an escape: ${row.specifier.files} files`);
    }
  }
}

async function main(argv) {
  const values = (name) => argv.flatMap((arg, at) => (arg === name ? [argv[at + 1]] : []));
  const out = values("--out")[0];
  if (argv.includes("--merge")) {
    const files = argv.slice(argv.indexOf("--merge") + 1).filter((arg) => arg !== "--out" && arg !== out);
    const merged = merge(files.map((file) => JSON.parse(readFileSync(file, "utf8"))));
    if (out) writeFileSync(out, `${JSON.stringify(merged, null, 1)}\n`);
    print(merged);
    return;
  }
  const roots = values("--root").map((spec) => { const [label, directory] = spec.split("="); return { label, directory: resolve(directory) }; });
  if (!roots.length || !out) throw new Error("usage: escape-census.mjs --root <label>=<dir> [--root ...] [--shard i/n] --out <report.json>");
  const [index, of] = (values("--shard")[0] ?? "0/1").split("/").map(Number);
  const report = await census(roots, { index, of });
  writeFileSync(out, `${JSON.stringify(report, null, 1)}\n`);
  if (of === 1) print(report);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main(process.argv.slice(2)).catch((error) => { console.error(error.stack ?? error.message); process.exitCode = 2; });
