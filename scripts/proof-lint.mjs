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
//   primitive-name-match  a primitive identified by its enum name outside the dialect crate: `== Some(Primitive::Untrack)`,
//                         `!= Primitive::CreateSignal`, `matches!(kind, Primitive::OnCleanup | ..)` (ADR 0945, G3). The call is
//                         resolved exactly, but which primitives behave alike is the dialect's answer (AGENTS.md invariant 4),
//                         not a list the analysis spells; each hit moves behind a `Dialect` question or states why it may stay.
//   silent-unmatched-slot a `_` or `None` arm that does nothing, `continue`s, `break`s or returns `None`: the walk over slots,
//                         arguments or facts ends silently for the case no arm names (ADR 0945, G4). Whether a hit-less walk means
//                         "clean" or "unknown" is the caller's, so each hit states what the unmatched case is, or becomes an
//                         uncertifiable result.
//   prose-hand-off        a comment that hands a case to another check ("SC2006's claim", "that function's to judge", "left to the ... pass", "not
//                         this walk's") with a drop in the next twenty code lines (`continue;`, `return None`, `return false`, `=> None`, a
//                         `.retain(`, or a kind filter beside a `record(` or `push(`) and no `Delegate::` there (ADR 0942, batch 11 slice 10b):
//                         the case is dropped on a sentence. A hand-off to another rule's claim is the typed `Delegate`, which ends the site only
//                         where that rule has a row; each hit states the proof that makes the drop sound (the other check does answer, and
//                         where), or becomes a `Delegate` or an uncertifiable row.
//   finding-filter        a `retain`, `retain_mut`, `extract_if`, a `dedup*` of rows or sites or a rows-assigned-from-`filter` over a collection that holds
//                         findings, sites, rows or the facts they stand on (ADR 0958): code that discards findings or sites by path,
//                         kind, rule or location. A finding leaves the report only through the attribution seam
//                         (`package_drop::hand_off`, `supersede_claims`) or a typed `DropPass` premise written to the proof log;
//                         every other hit states what the collection is (a memo table, a work list, a row replaced by its own
//                         duplicate) and why nothing of the report is lost, or becomes a drop with a premise.
//
// And two checks of the typed premises themselves (rust/crates/solid-reactive-ir/src/proof_log.rs):
//
//   dead-premise-kind     a `PremiseKind` no producer, drop pass or stage names: a claim nothing makes.
//   named ends            the `site_silences!` list (ADR 0936): `endIds` throws on a missing list, a duplicate id, or an id with no producer file.
//
// Limits: the scan is line-based over comment-stripped sources (the `prose-hand-off` pass reads the comments themselves, one line at a
// time: a phrase split across two comment lines is not seen). A text chain split across lines or through a binding (`let each =
// file.source_text(..)?; each.strip_prefix('[')`), a spelling compared through a variable, and a default written as an `if let ... else
// { true }` are not seen; and a pattern it flags may be a false positive (a role argument, a match pattern). The baseline therefore counts
// shapes, and a stated reason covers every hit of its file and kind: it is stated only after each of them was read.
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

import { ROOT, SOURCE_ROOT, codeLines, sourceFiles, testItemLines } from "./ir-read-lint.mjs";
import { EMPTY_REASONS } from "./lib/proof-log.mjs";

export const ALLOWLIST = join(ROOT, "scripts", "proof-lint.allowlist.json");
export const PENDING = "pending-review";
export const SCANNED = [SOURCE_ROOT, "rust/crates/solid-dialect/src", "rust/dialects/solid-v2/rules/src"];
export const PROOF_LOG = `${SOURCE_ROOT}/proof_log.rs`;
/** The backend files where findings leave the report (ADR 0958): scanned for `finding-filter` only, the other kinds are the analysis crates'. */
export const FINDING_FILTER_ONLY = ["rust/crates/solid-facts-backend/src/diagnostics.rs", "rust/crates/solid-facts-backend/src/package_drop.rs"];

/** The files that produce roles: the classifier and the placement code. The rest read roles, and compare or match them. */
export const PRODUCERS = [
  /^rust\/crates\/solid-reactive-ir\/src\/execution_role\.rs$/,
  /^rust\/crates\/solid-reactive-ir\/src\/execution_role\//,
  /^rust\/crates\/solid-reactive-ir\/src\/local_access\.rs$/,
  /^rust\/crates\/solid-reactive-ir\/src\/owners(?:\.rs|\/)/,
];

/** The files that own the primitive vocabulary: the dialect's questions are stated in its enum, so a name match there is the answer, not a copy of it (ADR 0945). */
export const VOCABULARY_OWNERS = [/^rust\/crates\/solid-dialect\//];

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
  // ADR 0945 (G3). A comparison or a `matches!` over a `Primitive` variant; `EffectApplyPrimitive::Several` is another type, so the name starts at a word boundary.
  "primitive-name-match": [/(?:==|!=)\s*(?:Some\(\s*)?(?:(?:crate::)?solid_dialect::)?\bPrimitive::[A-Z]/, /\bmatches!\s*\([^;]*\bPrimitive::[A-Z]/],
  // ADR 0945 (G4). An arm for the unmatched case (`_`, `None`) whose body ends the walk with no result: empty, `continue`, `break`, `return`, `return None`.
  "silent-unmatched-slot": [/(?:^|[\s{,(])(?:_|None)\s*=>\s*(?:\{\s*\}|continue\b|break\b|return\s*(?:None\b)?\s*(?:[,;}]|$))/],
  // ADR 0958. A collection loses members by a predicate (`retain`, `retain_mut`, `extract_if`), by equality (`dedup`, `dedup_by`, `dedup_by_key`)
  // or is rebuilt from a filtered copy of itself (`findings = findings.into_iter().filter(..)`).
  "finding-filter": [
    /\.\s*(?:retain|retain_mut|extract_if)\s*\(/,
    /\b(?:findings|violations|defects|rows|operations|requirements|reads|writes|actions|async_reads|sites|replacements)\s*\.\s*dedup(?:_by(?:_key)?)?\s*\(/,
    /\.\s*dedup_by(?:_key)?\s*\([^;]*(?:location|finding|\.id\b|\.rule\b|\.kind\b)/,
    /\b(?:findings|violations|defects|rows|operations|requirements|reads|writes|actions|async_reads)\s*=\s*[^;]*\.\s*filter\s*\(/,
  ],
};

/** ADR 0942 (batch 11, slice 10b): the comment phrases that hand a case to another check. A phrase is a claim in prose; the typed form is `Delegate`. */
export const HAND_OFF_PHRASE = /\b(?:SC\d{4}|[a-z][a-z-]+)'s (?:claim|to judge|to report|to answer)\b|left to the [\w -]+ pass|not this walk's/;
/** How many code lines after the comment the drop (or the `Delegate`) is looked for in: twenty, because the `SC2006's claim` hand-off of cleanup.rs has its kind filter 19 code lines below its comment. */
export const HAND_OFF_WINDOW = 20;
const HAND_OFF_DROPS = [/\bcontinue\s*;/, /\breturn\s+None\b/, /\breturn\s+false\b/, /=>\s*None\b/, /\.\s*retain(?:_mut)?\s*\(/];
/** A kind filter (`!= ..Kind::X`, `matches!(kind, ..)`) is a drop only beside a `record(` or `push(` it guards. */
const HAND_OFF_KIND_FILTER = [/(?:!=|==)\s*[\w:]*Kind::/, /\bmatches!\s*\(\s*[\w.]*kind\b/];
const HAND_OFF_RECORD = /\b(?:record|push)\s*\(/;

export const KIND_NAMES = ["untagged-silent-role", ...Object.keys(KINDS), "prose-hand-off"];

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

/**
 * The comments of one source text that hand a case to another check above code that drops it: `[{ line (1-based), kind, text }]`. A comment
 * line matching [`HAND_OFF_PHRASE`] outside a test item is a hit when one of the next [`HAND_OFF_WINDOW`] code lines (comments and blank
 * lines do not count) is a drop and none of them names a `Delegate::`.
 */
export function proseHandOffs(text) {
  const lines = text.split("\n");
  const inTest = testItemLines(text);
  const code = codeLines(text);
  const hits = [];
  lines.forEach((line, index) => {
    if (inTest[index] || !/^\s*\/\/[/!]?/.test(line) || !HAND_OFF_PHRASE.test(line)) return;
    const window = [];
    for (let at = index + 1; at < lines.length && window.length < HAND_OFF_WINDOW; at += 1) if (code[at].trim()) window.push(code[at]);
    if (window.some((candidate) => /\bDelegate::/.test(candidate))) return;
    const drops = window.some((candidate) => HAND_OFF_DROPS.some((pattern) => pattern.test(candidate)));
    const filtered = window.some((candidate) => HAND_OFF_KIND_FILTER.some((pattern) => pattern.test(candidate))) && window.some((candidate) => HAND_OFF_RECORD.test(candidate));
    if (drops || filtered) hits.push({ line: index + 1, kind: "prose-hand-off", text: normal(line) });
  });
  return hits;
}

/** The hits of one source text: `[{ line (1-based), kind, text }]`. Test modules are excluded; `producer` says whether roles are produced here, `owner` whether the file owns the primitive vocabulary. */
export function hitsOf(text, producer = true, owner = false, only = null) {
  const lines = codeLines(text);
  const tagged = taggedLines(lines);
  const hits = [];
  lines.forEach((line, index) => {
    if (!line.trim()) return;
    if (producer && !only && !tagged.has(index)) {
      for (const _ of silentRoleValues(line)) hits.push({ line: index + 1, kind: "untagged-silent-role", text: normal(line) });
    }
    for (const [kind, patterns] of Object.entries(KINDS)) {
      if (owner && kind === "primitive-name-match") continue;
      if (only && kind !== only) continue;
      if (patterns.some((pattern) => pattern.test(line))) hits.push({ line: index + 1, kind, text: normal(line) });
    }
  });
  if (!only || only === "prose-hand-off") hits.push(...proseHandOffs(text));
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
      for (const hit of hitsOf(readFileSync(path, "utf8"), PRODUCERS.some((pattern) => pattern.test(file)), VOCABULARY_OWNERS.some((pattern) => pattern.test(file)))) {
        counts[file] ??= {};
        counts[file][hit.kind] = (counts[file][hit.kind] ?? 0) + 1;
        hits.push({ file, ...hit });
      }
    }
  }
  for (const file of FINDING_FILTER_ONLY) {
    const path = join(root, file);
    if (!existsSync(path)) continue;
    for (const hit of hitsOf(readFileSync(path, "utf8"), false, false, "finding-filter")) {
      counts[file] ??= {};
      counts[file][hit.kind] = (counts[file][hit.kind] ?? 0) + 1;
      hits.push({ file, ...hit });
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

/** The file the ends are declared in (ADR 0936). */
export const SITE_ACCOUNTING = `${SOURCE_ROOT}/site_accounting.rs`;

/**
 * The ids of the named ends: the `site_silences!` list of site_accounting.rs (ADR 0936), one variant per producer return,
 * `<producer file stem>.<function>.<end>`. `stems` is the set of the producer files' stems. Throws on a missing list, a duplicate id
 * or an id whose stem names no producer file, so the check cannot pass on nothing. `[{ id, line, variant }]`.
 */
export function endIds(text, stems) {
  const start = text.indexOf("site_silences! {");
  if (start < 0) throw new Error("proof-lint: no `site_silences!` list in site_accounting.rs");
  const open = text.indexOf("{", start);
  const close = text.indexOf("\n}\n", open);
  if (close < 0) throw new Error("proof-lint: the `site_silences!` list does not close");
  const found = [];
  const seen = new Set();
  for (const match of text.slice(open, close).matchAll(/^\s*([A-Z][A-Za-z0-9]*)\s*=>\s*"([a-z0-9_]+\.[a-z0-9_]+\.[a-z0-9_-]+)"/gm)) {
    const [, variant, id] = match;
    if (seen.has(id)) throw new Error(`proof-lint: the silence id ${id} is listed twice`);
    seen.add(id);
    const stem = id.split(".")[0];
    if (!stems.has(stem)) throw new Error(`proof-lint: ${id} (${variant}): no producer file ${stem}.rs among the scanned sources`);
    found.push({ id, line: text.slice(0, open + match.index).split("\n").length + 1, variant });
  }
  return found;
}

/** Problems of the typed premises across the scanned sources: `[{ kind, key, message }]`. */
export function premiseProblems(root = ROOT) {
  const problems = [];
  const log = readFileSync(join(root, PROOF_LOG), "utf8");
  const declared = premiseKinds(log);
  const others = [];
  const stems = new Set();
  for (const directory of SCANNED) {
    const base = join(root, directory);
    if (!existsSync(base)) continue;
    for (const path of sourceFiles(base)) {
      const text = readFileSync(path, "utf8");
      stems.add(basename(path, ".rs"));
      if (relative(root, path) !== PROOF_LOG) others.push(text);
    }
  }
  endIds(readFileSync(join(root, SITE_ACCOUNTING), "utf8"), stems);
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

/** Builtins that change what they are given. A closed "seen" list that exempts one hides the mutation from the census (ADR 0803). */
export const MUTATORS = new Set([
  "assign", "defineProperty", "defineProperties", "setPrototypeOf", "set", "deleteProperty", "freeze", "seal", "preventExtensions",
  "push", "pop", "shift", "unshift", "splice", "sort", "reverse", "fill", "copyWithin",
]);
const QUALIFIED = /^[A-Za-z_][A-Za-z0-9_]*\.[A-Za-z_][A-Za-z0-9_]*$/;

/** The closed `*SEEN*` lists of one source text, with the qualified names (`ObjectConstructor.keys`) each lists: `[{ name, line, entries: [{ qualified, line }] }]`. */
export function seenLists(text) {
  const lines = codeLines(text);
  const found = [];
  for (let index = 0; index < lines.length; index += 1) {
    const head = lines[index].match(/\bconst\s+([A-Z0-9_]*SEEN[A-Z0-9_]*)\s*:[^=]*=\s*&?\[/);
    if (!head) continue;
    const entries = [];
    let depth = 0;
    let started = false;
    for (let at = index; at < lines.length; at += 1) {
      const scan = at === index ? lines[at].slice(lines[at].indexOf("[", lines[at].indexOf("="))) : lines[at];
      for (const literal of scan.matchAll(/"([^"\\]*)"/g)) if (QUALIFIED.test(literal[1])) entries.push({ qualified: literal[1], line: at + 1 });
      for (const character of scan.replace(/"(?:\\.|[^"\\])*"/g, '""')) {
        if (character === "[") {
          depth += 1;
          started = true;
        } else if (character === "]") depth -= 1;
      }
      if (started && depth <= 0) break;
    }
    found.push({ name: head[1], line: index + 1, entries });
  }
  return found;
}

/** The mutating builtins a closed `*SEEN*` list exempts in one source text: `[{ name, qualified, line }]`. */
export function seenMutators(text) {
  return seenLists(text).flatMap((list) =>
    list.entries.filter((entry) => MUTATORS.has(entry.qualified.split(".").pop())).map((entry) => ({ name: list.name, qualified: entry.qualified, line: entry.line })),
  );
}

/**
 * ADR 0803's cross-deferral: the census called a mutating builtin "seen" (it neither calls nor keeps what it is given) and left
 * its target to the visible-write scan, which read only what is spelled at the mutation. A closed `*SEEN*` list may exempt no
 * mutator; and the check must find a list at all, or it protects nothing.
 */
export function seenMutatorProblems(root = ROOT) {
  const problems = [];
  let lists = 0;
  for (const directory of SCANNED) {
    const base = join(root, directory);
    if (!existsSync(base)) continue;
    for (const path of sourceFiles(base)) {
      const file = relative(root, path);
      const text = readFileSync(path, "utf8");
      lists += seenLists(text).length;
      for (const { name, qualified, line } of seenMutators(text)) {
        problems.push({ kind: "seen-mutator", key: `${file}|${name}|${qualified}`, message: `${file}:${line}: ${name} exempts ${qualified}, a builtin that changes what it is given: its target is the census's question` });
      }
    }
  }
  if (lists === 0) problems.push({ kind: "seen-list-missing", key: "*SEEN*", message: "no closed `*SEEN*` list was found in the scanned sources: the check protects nothing" });
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
  found.push(...premiseProblems(), ...seenMutatorProblems());
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
