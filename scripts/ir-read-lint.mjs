#!/usr/bin/env bun
// Raw cross-file reads in the Reactive IR: a ratchet, not a rewrite.
//
// An incremental IR is exact only if every read of another file is one the
// reuse logic knows about (docs/decisions.md; the study behind this is
// tsrs-plan/p5-ir-incremental-study.md, section 3.4). The raw ways to read
// "the rest of the project" are the ones a recorded-read layer will replace:
//
//   facts.files       the project's file list
//   files()           SemanticLookup::files(), the same list behind the lookup
//   file_by_path      another file's facts by path
//   hashmap-iteration iterating a HashMap/HashSet in non-test code, whose order
//                     is the insertion history (so a patched map and a fresh
//                     one can answer differently). A lexical heuristic: it
//                     finds names declared with a HashMap/HashSet type in the
//                     same file and flags iterating them.
//
// scripts/ir-read-lint.allowlist.json records today's sites as a count per
// file and kind. A new site, or a new file with one, fails this check; a site
// removed makes the recorded count stale, which also fails so the allow-list
// only ever shrinks (--update rewrites it; a growing diff in review is the
// signal). Test code (tests.rs, tests/, a trailing `#[cfg(test)]` module) is
// not scanned.
//
//   bun scripts/ir-read-lint.mjs            check, exit 1 on any difference
//   bun scripts/ir-read-lint.mjs --update   rewrite the allow-list
import { existsSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

export const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
export const SOURCE_ROOT = "rust/crates/solid-reactive-ir/src";
export const ALLOWLIST = join(ROOT, "scripts", "ir-read-lint.allowlist.json");

const ITERATORS = "iter|iter_mut|keys|values|values_mut|into_iter|into_keys|into_values|drain";

/** Rust files under `directory`, without test modules. */
export function sourceFiles(directory) {
  const found = [];
  for (const name of readdirSync(directory).sort()) {
    const path = join(directory, name);
    if (statSync(path).isDirectory()) {
      if (name !== "tests") found.push(...sourceFiles(path));
    } else if (name.endsWith(".rs") && name !== "tests.rs") {
      found.push(path);
    }
  }
  return found;
}

/** The non-test, non-comment lines of a Rust source. */
export function codeLines(text) {
  const lines = text.split("\n");
  const end = lines.findIndex((line) => /^\s*#\[cfg\(test\)\]/.test(line));
  return (end === -1 ? lines : lines.slice(0, end)).map((line) => line.replace(/\/\/.*$/, ""));
}

/** Counts of each raw-read kind in one source text. */
export function countReads(text) {
  const lines = codeLines(text);
  const counts = { "facts.files": 0, "files()": 0, file_by_path: 0, "hashmap-iteration": 0 };
  const names = new Set();
  const declarations = [
    /\b([a-z_][a-z0-9_]*)\s*:\s*&?(?:mut\s+)?(?:std::collections::)?Hash(?:Map|Set)\s*</,
    /\blet\s+(?:mut\s+)?([a-z_][a-z0-9_]*)\s*(?::[^=]*)?=\s*(?:std::collections::)?Hash(?:Map|Set)::/,
    /\blet\s+(?:mut\s+)?([a-z_][a-z0-9_]*)\s*(?::[^=]*)?=[^;]*collect::<\s*(?:std::collections::)?Hash(?:Map|Set)\b/
  ];
  for (const line of lines) {
    for (const pattern of declarations) {
      const found = pattern.exec(line);
      if (found) names.add(found[1]);
    }
  }
  const iterated = names.size
    ? [
        new RegExp(`\\b(?:${[...names].join("|")})\\s*\\.\\s*(?:${ITERATORS})\\s*\\(`),
        new RegExp(`\\bfor\\b[^\\n{]*\\bin\\s+&?(?:mut\\s+)?(?:self\\.)?(?:${[...names].join("|")})\\b`)
      ]
    : [];
  for (const line of lines) {
    counts["facts.files"] += (line.match(/\bfacts\s*\.\s*files\b/g) ?? []).length;
    counts["files()"] += (line.match(/\.\s*files\s*\(\s*\)/g) ?? []).length;
    counts.file_by_path += (line.match(/\bfile_by_path\b/g) ?? []).length;
    if (iterated.some((pattern) => pattern.test(line))) counts["hashmap-iteration"] += 1;
  }
  return counts;
}

/** `{ relative path: { kind: count } }` for the IR's non-test sources. */
export function measure(root = ROOT) {
  const measured = {};
  const base = join(root, SOURCE_ROOT);
  for (const path of sourceFiles(base)) {
    const counts = countReads(readFileSync(path, "utf8"));
    const nonzero = Object.fromEntries(Object.entries(counts).filter(([, count]) => count > 0));
    if (Object.keys(nonzero).length > 0) measured[relative(root, path)] = nonzero;
  }
  return measured;
}

/** Human-readable differences between the allow-list and the measurement. */
export function violations(allowed, measured) {
  const problems = [];
  for (const path of [...new Set([...Object.keys(allowed), ...Object.keys(measured)])].sort()) {
    const before = allowed[path] ?? {};
    const after = measured[path] ?? {};
    for (const kind of [...new Set([...Object.keys(before), ...Object.keys(after)])].sort()) {
      const was = before[kind] ?? 0;
      const now = after[kind] ?? 0;
      if (now > was) {
        problems.push(
          `${path}: ${now - was} new raw cross-file read(s) of kind ${kind} (${was} allowed, ${now} found). ` +
            "Read through a recorded query, or, for a base index or assembly module, record the site with --update and justify it in review."
        );
      } else if (now < was) {
        problems.push(
          `${path}: ${kind} fell from ${was} to ${now}; run 'bun scripts/ir-read-lint.mjs --update' so the allow-list ratchets down.`
        );
      }
    }
  }
  return problems;
}

function main() {
  const measured = measure();
  if (process.argv.includes("--update")) {
    writeFileSync(ALLOWLIST, `${JSON.stringify(measured, null, 2)}\n`);
    const total = Object.values(measured).flatMap((counts) => Object.values(counts)).reduce((a, b) => a + b, 0);
    console.log(`ir-read-lint: allow-list rewritten (${Object.keys(measured).length} files, ${total} sites)`);
    return 0;
  }
  const allowed = existsSync(ALLOWLIST) ? JSON.parse(readFileSync(ALLOWLIST, "utf8")) : {};
  const problems = violations(allowed, measured);
  for (const problem of problems) console.error(problem);
  if (problems.length > 0) return 1;
  const total = Object.values(measured).flatMap((counts) => Object.values(counts)).reduce((a, b) => a + b, 0);
  console.log(`ir-read-lint: ok (${total} allow-listed raw cross-file reads in ${Object.keys(measured).length} files)`);
  return 0;
}

if (process.argv[1] && process.argv[1].endsWith("ir-read-lint.mjs")) process.exit(main());
