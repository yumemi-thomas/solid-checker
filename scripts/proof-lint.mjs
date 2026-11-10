#!/usr/bin/env bun
// The proof lint (ADR 0933, phase B): a ratchet on the shapes of code that decide a silence without a fact the proof log can name.
//
// AGENTS.md invariants 2 and 3 say a clean needs a proof: a missing fact is an uncertifiable result, never a silent clean, and a
// callee, member or binding is resolved to an exact declaration, never identified by its spelling. The proof gate
// (scripts/proof-gate.mjs) finds what a fixture shows at run time; this finds the code shapes that make those holes possible,
// whether or not a fixture reaches them:
//
//   untagged-silent-role  a role no read or write rule reports (`TrackedJsx`, `DeferredCallback`, `EventCallback`, ...) produced in the
//                         classifier or the placement code outside a `role_arm!`: the role log cannot say which arm gave it, so C3
//                         cannot tell a proof from a default.
//   text-premise          a decision read from source text: a slice, a prefix, a suffix, a `contains` or a `find` of `source_text(..)`.
//   name-only-match       a callee, member or binding identified by its spelling: `source_text(..) == Some("name")`.
//   default-on-missing    a predicate that answers yes when a fact is missing: `is_none_or(..)`, `unwrap_or(true)`, `None => true`.
//                         Whether yes means "reports" or "silent" depends on the predicate, so each hit needs its polarity stated.
//
// And two checks of the typed premises themselves (rust/crates/solid-reactive-ir/src/proof_log.rs):
//
//   dead-premise-kind     a `PremiseKind` no producer, drop pass or stage names: a claim nothing makes.
//   duplicate-end-id      two named ends (`end_at` / `end_with`) with one id, or an id that is not `<file stem>.<function>.<end>`.
//
// scripts/proof-lint.allowlist.json is a baseline, as scripts/ir-read-lint.allowlist.json is: per file and kind, how many hits stand
// and why. A hit the baseline does not count fails; a count that fell fails until `--update` lowers it (the file only shrinks). An
// entry's `why` is a stated reason (the fact that makes the shape sound, at least 20 characters) or `pending-review`: counted,
// frozen, and not yet audited. The summary prints the pending count, which is the progress metric of phase B.
//
//   bun scripts/proof-lint.mjs             check
//   bun scripts/proof-lint.mjs --list      print every hit, grouped by file and kind
//   bun scripts/proof-lint.mjs --update    rewrite the baseline from the sources (keeps every stated reason; a new entry is pending-review)
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { basename, join, relative } from "node:path";
import process from "node:process";

import { ROOT, SOURCE_ROOT, codeLines, sourceFiles } from "./ir-read-lint.mjs";
import { EMPTY_REASONS } from "./lib/proof-log.mjs";

export const ALLOWLIST = join(ROOT, "scripts", "proof-lint.allowlist.json");
export const PENDING = "pending-review";
export const SCANNED = [SOURCE_ROOT, "rust/crates/solid-dialect/src", "rust/dialects/solid-v2/rules/src"];
export const PROOF_LOG = `${SOURCE_ROOT}/proof_log.rs`;

/** The files that produce roles: the classifier and the placement code. The rest read roles, and compare or match them. */
export const PRODUCERS = [
  /^rust\/crates\/solid-reactive-ir\/src\/execution_role\.rs$/,
  /^rust\/crates\/solid-reactive-ir\/src\/execution_role\//,
  /^rust\/crates\/solid-reactive-ir\/src\/local_access\.rs$/,
  /^rust\/crates\/solid-reactive-ir\/src\/owners(?:\.rs|\/)/,
];

/** Roles no read rule reports, and the write-only ones (silent for a write, reporting for a read). */
const SILENT_ROLES = [
  "TrackedJsx",
  "DeferredCallback",
  "EventCallback",
  "DirectiveApply",
  "Unreported",
  "DiscardedRendering",
  "ModuleInitialization",
  "UntrackedCallback",
  "UntrackedUnwindowed",
  "EffectApply",
];

const ROLE = new RegExp(`\\bExecutionRole::(?:${SILENT_ROLES.join("|")})\\b`, "g");

const NAMED = String.raw`(?:source_text|static_callee|text)\s*\([^()]*(?:\([^()]*\)[^()]*)*\)`;

/** Each pattern marks a line of its kind. */
const KINDS = {
  "text-premise": [
    new RegExp(
      String.raw`\b(?:source_text\s*\([^;]*?\)|source\.get\s*\([^;]*?\))\s*(?:\??\.\s*)?(?:and_then|map|is_some_and|map_or)?[^;]*?\.\s*(?:starts_with|ends_with|contains|find|rfind|strip_prefix|strip_suffix|split|trim_start|trim_end|char_indices|chars)\s*\(`,
    ),
  ],
  "name-only-match": [new RegExp(`${NAMED}\\s*(?:==|!=)\\s*(?:Some\\()?"[^"]*"`), new RegExp(String.raw`(?:==|!=)\s*(?:Some\()?"[^"]*"\)?\s*[^;]*${NAMED}`)],
  "default-on-missing": [/\.is_none_or\s*\(/, /\.unwrap_or\(\s*true\s*\)/, /\bNone\s*=>\s*true\b/, /\b_\s*=>\s*true\b/],
};

export const KIND_NAMES = ["untagged-silent-role", ...Object.keys(KINDS)];

/** The normalized text of a line: whitespace collapsed, so a reflow is the same hit. */
const normal = (line) => line.replace(/\s+/g, " ").trim();

/**
 * The silent role literals of `line` that PRODUCE a role: returned, yielded by a match arm, wrapped in `Some`, bound, or handed to
 * `then_some`. A comparison (`==`, `!=`), a pattern (left of `=>`, after `|`, inside `matches!` or `if let`), an import and a
 * `role_arm!` argument produce none.
 */
export function silentRoleValues(line) {
  const hits = [];
  for (const match of line.matchAll(ROLE)) {
    const before = line.slice(0, match.index);
    const after = line.slice(match.index + match[0].length);
    if (/(?:==|!=)\s*(?:Some\(\s*)*(?:crate::)?$/.test(before)) continue;
    if (/^\s*[,)]*\s*(?:\|(?!\|)|=>|if\b)/.test(after)) continue;
    if (/\|\s*(?:crate::)?$/.test(before)) continue;
    if (/\bmatches!\s*\(/.test(before) || /\b(?:if|while)\s+let\b[^=]*$/.test(before)) continue;
    if (/^\s*use\b/.test(line)) continue;
    if (/\brole_arm!\s*\(/.test(line)) continue;
    hits.push(match[0]);
  }
  return hits;
}

/** The lines of `lines` inside a `role_arm!(...)` call, which may span several. */
function taggedLines(lines) {
  const tagged = new Set();
  for (let index = 0; index < lines.length; index += 1) {
    if (!/\brole_arm!\s*\(/.test(lines[index])) continue;
    let depth = 0;
    for (let at = index; at < lines.length; at += 1) {
      const scan = at === index ? lines[at].slice(lines[at].search(/\brole_arm!\s*\(/)) : lines[at];
      for (const character of scan.replace(/"(?:\\.|[^"\\])*"/g, '""')) {
        if (character === "(") depth += 1;
        else if (character === ")") depth -= 1;
      }
      tagged.add(at);
      if (depth <= 0) break;
    }
  }
  return tagged;
}

/** The hits of one source text: `[{ line (1-based), kind, text }]`. Test modules are excluded; `producer` says whether roles are produced here. */
export function hitsOf(text, producer = true) {
  const lines = codeLines(text);
  const tagged = taggedLines(lines);
  const hits = [];
  lines.forEach((line, index) => {
    if (!line.trim()) return;
    if (producer && !tagged.has(index)) {
      for (const _ of silentRoleValues(line)) hits.push({ line: index + 1, kind: "untagged-silent-role", text: normal(line) });
    }
    for (const [kind, patterns] of Object.entries(KINDS)) {
      if (patterns.some((pattern) => pattern.test(line))) hits.push({ line: index + 1, kind, text: normal(line) });
    }
  });
  return hits;
}

/** `{ file: { kind: count } }` and the hits behind them, for the scanned sources. */
export function measure(root = ROOT) {
  const counts = {};
  const hits = [];
  for (const directory of SCANNED) {
    const base = join(root, directory);
    if (!existsSync(base)) continue;
    for (const path of sourceFiles(base)) {
      const file = relative(root, path);
      for (const hit of hitsOf(readFileSync(path, "utf8"), PRODUCERS.some((pattern) => pattern.test(file)))) {
        counts[file] ??= {};
        counts[file][hit.kind] = (counts[file][hit.kind] ?? 0) + 1;
        hits.push({ file, ...hit });
      }
    }
  }
  return { counts, hits };
}

/** The variants of `enum PremiseKind` in a proof_log.rs text. */
export function premiseKinds(text) {
  const start = text.search(/enum PremiseKind\s*\{/);
  if (start < 0) return [];
  const body = text.slice(text.indexOf("{", start) + 1);
  const end = body.indexOf("\n}\n");
  return [...body.slice(0, end).matchAll(/^    ([A-Z][A-Za-z0-9]+),?\s*$/gm)].map((match) => match[1]);
}

/** The ids of the named ends (`end_at`, `end_with`, and the producers' wrappers of them) in one source text, with the line each stands on. */
export function endIds(text) {
  const found = [];
  const lines = codeLines(text);
  for (let index = 0; index < lines.length; index += 1) {
    if (!/\b(?:end_(?:at|with)|dormant_callback_end)\s*\(/.test(lines[index])) continue;
    // the id is the first string literal within the next few lines of the call
    for (let at = index; at < Math.min(lines.length, index + 6); at += 1) {
      const literal = lines[at].match(/"([a-z0-9_]+\.[a-z0-9_]+\.[a-z0-9_-]+)"/);
      if (literal) {
        found.push({ id: literal[1], line: at + 1 });
        break;
      }
    }
  }
  return found;
}

/** Problems of the typed premises across the scanned sources: `[{ kind, key, message }]`. */
export function premiseProblems(root = ROOT) {
  const problems = [];
  const log = readFileSync(join(root, PROOF_LOG), "utf8");
  const declared = premiseKinds(log);
  const others = [];
  const ends = new Map();
  for (const directory of SCANNED) {
    const base = join(root, directory);
    if (!existsSync(base)) continue;
    for (const path of sourceFiles(base)) {
      const file = relative(root, path);
      const text = readFileSync(path, "utf8");
      if (file !== PROOF_LOG) others.push(text);
      for (const { id, line } of endIds(text)) {
        const stem = basename(file, ".rs");
        if (!id.startsWith(`${stem}.`)) {
          problems.push({ kind: "duplicate-end-id", key: id, message: `${file}:${line}: an end id is \`<file stem>.<function>.<end>\`, here the stem is \`${stem}\`` });
        }
        if (ends.has(id)) problems.push({ kind: "duplicate-end-id", key: id, message: `${file}:${line} repeats the id of ${ends.get(id)}` });
        else ends.set(id, `${file}:${line}`);
      }
    }
  }
  // A kind is used where a producer, a pass or a stage names it; the log's own tables (a drop pass's kinds) count.
  const usedInLog = log.replace(/enum PremiseKind\s*\{[\s\S]*?\n\}\n/, "");
  for (const kind of declared) {
    const pattern = new RegExp(`\\bPremiseKind::${kind}\\b`);
    if (!pattern.test(usedInLog) && !others.some((text) => pattern.test(text))) {
      problems.push({ kind: "dead-premise-kind", key: kind, message: "no producer, drop pass or stage names this premise kind: remove it or use it" });
    }
  }
  return problems;
}

/** The problems of a baseline against a measurement: `[{ kind, key, message }]`; `pending` is how many hits are still unaudited. */
export function problems(allowed, measured) {
  const found = [];
  let pending = 0;
  for (const file of [...new Set([...Object.keys(allowed), ...Object.keys(measured)])].sort()) {
    const before = allowed[file] ?? {};
    const after = measured[file] ?? {};
    for (const kind of [...new Set([...Object.keys(before), ...Object.keys(after)])].sort()) {
      const was = before[kind]?.count ?? 0;
      const now = after[kind] ?? 0;
      const why = before[kind]?.why;
      if (before[kind]) {
        const stated = typeof why === "string" && !EMPTY_REASONS.test(why.trim()) && why.trim().length >= 20;
        if (!stated && why !== PENDING) found.push({ kind: "invalid", key: `${file}|${kind}`, message: `why is a stated fact (at least 20 characters) or ${PENDING}` });
        if (why === PENDING) pending += Math.min(was, now);
      }
      if (now > was) found.push({ kind: "new", key: `${file}|${kind}`, message: `${now - was} new ${kind} hit(s) (${was} counted, ${now} found)` });
      else if (now < was) found.push({ kind: "stale", key: `${file}|${kind}`, message: `${kind} fell from ${was} to ${now}: run --update so the baseline shrinks` });
    }
  }
  return { found, pending };
}

/** The baseline for `counts`: stated reasons are kept, anything new is pending review. */
export function baselineFor(counts, previous = {}) {
  const files = {};
  for (const [file, kinds] of Object.entries(counts).sort(([a], [b]) => a.localeCompare(b))) {
    files[file] = {};
    for (const [kind, count] of Object.entries(kinds).sort(([a], [b]) => a.localeCompare(b))) {
      files[file][kind] = { count, why: previous[file]?.[kind]?.why ?? PENDING };
    }
  }
  return files;
}

function main(argv) {
  const { counts, hits } = measure();
  if (argv.includes("--list")) {
    const byKind = {};
    for (const hit of hits) byKind[hit.kind] = (byKind[hit.kind] ?? 0) + 1;
    console.log(`proof-lint: ${Object.entries(byKind).map(([kind, count]) => `${kind} ${count}`).join(", ")}`);
    for (const hit of hits.sort((a, b) => a.file.localeCompare(b.file) || a.line - b.line)) console.log(`  ${hit.file}:${hit.line} ${hit.kind} ${hit.text.slice(0, 140)}`);
    return 0;
  }
  const previous = existsSync(ALLOWLIST) ? JSON.parse(readFileSync(ALLOWLIST, "utf8")) : {};
  if (argv.includes("--update")) {
    const baseline = { about: previous.about ?? "", files: baselineFor(counts, previous.files) };
    writeFileSync(ALLOWLIST, `${JSON.stringify(baseline, null, 2)}\n`);
    const total = Object.values(counts).flatMap((kinds) => Object.values(kinds)).reduce((a, b) => a + b, 0);
    console.log(`proof-lint: baseline rewritten (${Object.keys(counts).length} files, ${total} hits)`);
    return 0;
  }
  const { found, pending } = problems(previous.files ?? {}, counts);
  found.push(...premiseProblems());
  for (const problem of found) console.error(`${problem.kind}: ${problem.key} -- ${problem.message}`);
  if (found.length > 0) {
    console.error(`${found.length} problem(s): a new hit needs a fact in place of the shape, or a stated reason in scripts/proof-lint.allowlist.json`);
    return 1;
  }
  const total = Object.values(counts).flatMap((kinds) => Object.values(kinds)).reduce((a, b) => a + b, 0);
  console.log(`proof-lint: ok (${total} baseline hits in ${Object.keys(counts).length} files, ${pending} pending review)`);
  return 0;
}

if (process.argv[1] && process.argv[1].endsWith("proof-lint.mjs")) process.exit(main(process.argv.slice(2)));
