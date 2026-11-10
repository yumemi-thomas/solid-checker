// The boundary inventories (ADR 0935): every silence-deciding item of the checker, read from its source, with its file and line.
//
// A silent clean is only as sound as the premise of the item that answered it. Phase C pins one boundary shape (a pair of programs that differ
// only in the fact the premise claims) to each of these items; `boundary.mjs` holds the registry and `boundary.test.mjs` fails when an item has
// no shape and no stated gap. The lists are *derived*, never typed twice: a source scan like `roles.mjs`'s `armInventory`, so adding an arm, a
// named end, a premise kind or a table row to the Rust makes the test fail until it is covered or stated.
//
//   silentArms()    the tagged arms (`role_arm!`) that can answer a silent role: a literal silent role, or a computed role (transported)
//   namedEnds()     the producers' named early ends (`proof_log::end_at` / `end_with` and the store rules' `end` closure): `<file>.<fn>.<end>`
//   premiseKinds()  the variants of `proof_log::PremiseKind`, the typed facts a silence rests on
//   dropPasses()    the variants of `proof_log::DropPass` with the premise kinds each rests on (the passes that remove or demote a finding)
//   tableEntries()  the rows of the checker's own tables that decide a role or a silence, per table (TABLES below)
//   dialectQuestions()  the questions `impl Dialect for Solid2` answers per primitive and argument (the dialect API tables)
import { readFileSync, readdirSync, statSync } from "node:fs";
import { basename, dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
export const REPO = resolve(here, "../..");
export const IR_SRC = join(REPO, "rust/crates/solid-reactive-ir/src");
export const DIALECT_SRC = join(REPO, "rust/crates/solid-dialect/src");

/** Reads a source file once. */
const cache = new Map();
export function source(path) {
  if (!cache.has(path)) cache.set(path, readFileSync(path, "utf8"));
  return cache.get(path);
}

/** The 1-based line of a character offset. */
export const lineAt = (text, offset) => text.slice(0, offset).split("\n").length;

/** The source without its `#[cfg(test)]` module (tests name roles and arms the checker never runs). */
function withoutTests(text) {
  const cut = text.search(/#\[cfg\(test\)\]\s*\n\s*mod tests/);
  return cut < 0 ? text : text.slice(0, cut);
}

/** The index just past the bracket that closes the one opened at `open` (strings and comments skipped). */
function closing(text, open) {
  const pairs = { "(": ")", "[": "]", "{": "}" };
  const stack = [pairs[text[open]]];
  for (let at = open + 1; at < text.length; at += 1) {
    const char = text[at];
    if (char === '"') { at = skipString(text, at); continue; }
    if (char === "/" && text[at + 1] === "/") { at = text.indexOf("\n", at); if (at < 0) return text.length; continue; }
    if (char === "'" && /^'(?:\\.|[^\\'])'/.test(text.slice(at, at + 5))) { at += text.slice(at, at + 5).match(/^'(?:\\.|[^\\'])'/)[0].length - 1; continue; }
    if (pairs[char]) stack.push(pairs[char]);
    else if (char === stack[stack.length - 1]) { stack.pop(); if (stack.length === 0) return at + 1; }
  }
  return text.length;
}

function skipString(text, start) {
  for (let at = start + 1; at < text.length; at += 1) {
    if (text[at] === "\\") { at += 1; continue; }
    if (text[at] === '"') return at;
  }
  return text.length;
}

/** Every `.rs` file under `path`, sorted (tests excluded). */
export function rustFiles(path) {
  if (!statSync(path).isDirectory()) return [path];
  return readdirSync(path).sort().flatMap((name) => {
    const full = join(path, name);
    if (statSync(full).isDirectory()) return name === "tests" ? [] : rustFiles(full);
    return name.endsWith(".rs") && name !== "tests.rs" ? [full] : [];
  });
}

// ---------------------------------------------------------------------------------------------------------------------------------------
// Tagged arms

/** The roles a read in which nothing is reported (ExecutionRole::reports_untracked_read false, and not Unknown). */
const SILENT_READ = new Set(["TrackedJsx", "DeferredCallback", "EventCallback", "DirectiveApply", "Unreported", "DiscardedRendering"]);
/** The roles a write or an action in which nothing is reported (ExecutionRole::reports_disallowed_write false, and not Unknown). */
const SILENT_WRITE = new Set(["Unreported", "ModuleInitialization", "DeferredCallback", "UntrackedCallback", "UntrackedUnwindowed", "EffectApply", "EventCallback", "DirectiveApply", "DiscardedRendering"]);

/**
 * Every tagged arm of the classifier: `{ id, file, line, default, roles, kind }`. `kind`:
 *   `read-silent`        a literal role that is silent for reads (and so for the rule that reads it);
 *   `write-silent-only`  a literal role silent for writes and actions only;
 *   `computed`           the role is an expression: the arm transports another decision's answer, silent iff that is;
 *   `reporting`          a literal role that reports (a read, a write or both): a wrong answer is a false positive, which the differential grids' own
 *                        judge finds, so it needs no boundary shape here;
 *   `unknown`            `Unknown`: an obligation, never a clean.
 */
export function taggedArms(root = IR_SRC) {
  const arms = [];
  for (const dir of ["execution_role.rs", "execution_role", "owners"]) {
    let full;
    try { full = join(root, dir); statSync(full); } catch { continue; }
    for (const path of rustFiles(full)) {
      const text = withoutTests(source(path));
      const stem = basename(path, ".rs");
      for (const match of text.matchAll(/\b(?:role_arm|arm_id)!\(\s*(default\s+)?"([^"]+)"\s*(,)?/g)) {
        const id = match[2];
        let expression = "";
        if (match[3]) {
          const open = text.indexOf("(", match.index);
          expression = text.slice(match.index + match[0].length, closing(text, open) - 1);
        }
        const roles = [...new Set([...expression.matchAll(/ExecutionRole::(\w+)/g)].map((role) => role[1]))].sort();
        let kind;
        if (!match[3]) kind = "owner"; // `arm_id!` at an owner answer (owners/owner_emission.rs): an owner context, not a role
        else if (roles.length === 0) kind = "computed";
        else if (roles.some((role) => SILENT_READ.has(role))) kind = "read-silent";
        else if (roles.some((role) => SILENT_WRITE.has(role))) kind = "write-silent-only";
        else if (roles.every((role) => role === "Unknown")) kind = "unknown";
        else kind = "reporting";
        if (!id.startsWith(`${stem}.`)) throw new Error(`arm ${id} in ${relative(root, path)}: an arm id starts with its file's stem (${stem}.)`);
        arms.push({ id, file: relative(root, path), line: lineAt(text, match.index), default: Boolean(match[1]), roles, kind });
      }
    }
  }
  return arms;
}

/** The arms a wrong silent answer would make a false clean: literal silent roles and computed (transported) ones. */
export const silentArms = (root) => taggedArms(root).filter((arm) => ["read-silent", "write-silent-only", "computed"].includes(arm.kind));

// ---------------------------------------------------------------------------------------------------------------------------------------
// Named ends (ADR 0933)

/**
 * The producers' named early ends: literal ids `<file stem>.<function>.<end>` given to `proof_log::end_at` / `end_with` (and the store rules' `end`
 * closure). Every literal of that shape in a file that calls them is one: an arm id starts with the stem of its own directory's file
 * (`callee_roles.`), never with these files' (`local_access.`, `shared_reactivity.`).
 */
export function namedEnds(root = IR_SRC) {
  const ends = [];
  for (const path of rustFiles(root)) {
    const text = withoutTests(source(path));
    if (!/proof_log::end_(?:at|with)\b/.test(text)) continue;
    const stem = basename(path, ".rs");
    const seen = new Set();
    for (const match of text.matchAll(new RegExp(`"(${stem}\\.[a-z_0-9]+\\.[a-z0-9-]+)"`, "g"))) {
      if (seen.has(match[1])) continue;
      seen.add(match[1]);
      ends.push({ id: match[1], file: relative(root, path), line: lineAt(text, match.index) });
    }
  }
  return ends;
}

// ---------------------------------------------------------------------------------------------------------------------------------------
// Premise kinds and drop passes (proof_log.rs)

/** The variants of `enum PremiseKind`: `{ id, line, doc }`. */
export function premiseKinds(root = IR_SRC) {
  const path = join(root, "proof_log.rs");
  const text = source(path);
  const start = text.indexOf("pub(crate) enum PremiseKind");
  const open = text.indexOf("{", start);
  const body = text.slice(open + 1, closing(text, open) - 1);
  const base = lineAt(text, open);
  const kinds = [];
  let doc = [];
  body.split("\n").forEach((line, index) => {
    const trimmed = line.trim();
    if (trimmed.startsWith("///")) { doc.push(trimmed.slice(3).trim()); return; }
    const variant = /^(\w+),$/.exec(trimmed);
    if (variant) kinds.push({ id: variant[1], line: base + index, doc: doc.join(" ") });
    doc = [];
  });
  return kinds;
}

/** The variants of `enum DropPass` with their premise kinds (`premise_kinds`): `{ id, line, premises }`. */
export function dropPasses(root = IR_SRC) {
  const path = join(root, "proof_log.rs");
  const text = source(path);
  const start = text.indexOf("pub enum DropPass");
  const open = text.indexOf("{", start);
  const body = text.slice(open + 1, closing(text, open) - 1);
  const base = lineAt(text, open);
  const passes = [];
  body.split("\n").forEach((line, index) => {
    const variant = /^\s*(\w+),$/.exec(line);
    if (variant) passes.push({ id: variant[1], line: base + index, premises: [] });
  });
  const kinds = text.indexOf("const fn premise_kinds");
  const kopen = text.indexOf("{", kinds);
  const kbody = text.slice(kopen + 1, closing(text, kopen) - 1);
  for (const match of kbody.matchAll(/Self::(\w+)\s*=>\s*&\[([^\]]*)\]/g)) {
    const pass = passes.find((candidate) => candidate.id === match[1]);
    if (pass) pass.premises = [...match[2].matchAll(/PremiseKind::(\w+)/g)].map((kind) => kind[1]);
  }
  return passes;
}

// ---------------------------------------------------------------------------------------------------------------------------------------
// Tables: the rows of the checker's own tables that decide a role or a silence

/** The function or constant `name` in `text`: `{ body, line }` of its braces (a `fn`) or brackets (a `const` array). */
function item(text, pattern) {
  const found = pattern.exec(text);
  if (!found) throw new Error(`boundary-inventory: no ${pattern} in source`);
  const open = text.slice(found.index).search(/[{[]/) + found.index;
  // A `fn name(...) -> Type {`: skip the parameter list and the return type to the body's brace.
  let start = open;
  if (/\bfn\s/.test(found[0])) {
    const parameters = text.indexOf("(", found.index);
    const afterParameters = closing(text, parameters);
    start = text.indexOf("{", afterParameters);
  } else if (/&\[/.test(found[0])) {
    start = text.indexOf("[", text.indexOf("=", found.index));
  }
  const end = closing(text, start);
  return { body: text.slice(start, end), line: lineAt(text, start), offset: start };
}

const QUALIFIED = /^[A-Za-z_][A-Za-z_0-9]*(?:\.[A-Za-z_][A-Za-z_0-9]*)*$/;
/** Words callback_host.rs matches that are option names of a call, not APIs. */
const CALLBACK_HOST_OPTIONS = new Set(["defer", "effect", "children", "when", "false", "each", "count", "capture"]);

/** The string literals of `body` that look like qualified names, each with its line and (if any) the `RuntimeArgumentBehavior` its arm returns. */
function qualifiedNames(text, region, { behavior = false } = {}) {
  const names = [];
  for (const match of region.body.matchAll(/"([^"\\]+)"/g)) {
    if (!QUALIFIED.test(match[1])) continue;
    const offset = region.offset + match.index;
    const entry = { name: match[1], line: lineAt(text, offset) };
    if (behavior) entry.behavior = /Some\(RuntimeArgumentBehavior::(\w+)\)/.exec(region.body.slice(match.index))?.[1] ?? null;
    names.push(entry);
  }
  return names;
}

/**
 * The tables, by id. Each has a `file` (under rust/crates/solid-reactive-ir/src) and `scan(text) -> [{ name, line, ... }]`. A table is listed here
 * once it decides a role or a silence: the hand-off class of a standard-library argument, the schedulers and carriers, the dispatch lists, the
 * event-delivery lists. A `name` is the row's key: the compiler-selected qualified name (`Set.add`), a `lib.dom` member (`Document.execCommand`),
 * an event name, or a schedule.
 */
export const TABLES = {
  // runtime_semantics.rs `timing_behavior`: what each standard-library API does with a function argument (ADR 0531, 0535, 0927)
  "argument-behavior": { file: "runtime_semantics.rs", scan: (text) => qualifiedNames(text, item(text, /fn timing_behavior\(/), { behavior: true }) },
  // runtime_semantics.rs FRESH_STACK_SCHEDULERS: the schedulers that run a callback from a task or microtask (ADR 0438's premise for `FreshStack`)
  "fresh-stack-schedulers": { file: "runtime_semantics.rs", scan: (text) => qualifiedNames(text, item(text, /const FRESH_STACK_SCHEDULERS: &\[&str\]/)) },
  // runtime_semantics.rs `scheduler_throw_reaches_host` (ADR 0893): a callback whose throw nothing in the program can take
  "throw-reaches-host": { file: "runtime_semantics.rs", scan: (text) => qualifiedNames(text, item(text, /fn scheduler_throw_reaches_host\(/)) },
  // carrier_runs.rs: the carriers whose callback runs once per element, or once (ADRs 0563, 0566, 0580)
  "carrier-runs": { file: "carrier_runs.rs", scan: (text) => qualifiedNames(text, item(text, /fn [a-z_]*(?:carrier|runs)[a-z_]*\(/)) },
  // callback_host.rs: the host registrations and their removals (ADR 0535)
  "callback-host": { file: "callback_host.rs", scan: (text) => qualifiedNames(text, { body: text, offset: 0 }).filter((entry) => !CALLBACK_HOST_OPTIONS.has(entry.name)) },
  // cleanup.rs `synchronously_dispatching_dom_member` (ADR 0329, 0642, 0705): `lib.dom` members that run listeners before they return
  "dispatching-members": { file: "cleanup.rs", scan: (text) => dispatchMembers(text) },
  // runtime_semantics.rs `delivers_synchronously` (ADR 0642): the same list as the event-delivery index reads
  "delivers-synchronously": { file: "runtime_semantics.rs", scan: (text) => deliverMembers(text) },
};

/**
 * `synchronously_dispatching_dom_member`: the member names that dispatch on any interface (`focus`, `blur`, `click`, ...), and the exact
 * `Owner.member` pairs of its tuple patterns (`("HTMLFormElement", "reset" | "requestSubmit" | "submit")`). An empty owner is the global.
 */
function dispatchMembers(text) {
  const region = item(text, /fn synchronously_dispatching_dom_member\(/);
  const entries = [];
  const literals = (list) => [...list.matchAll(/"([A-Za-z]*)"/g)].map((literal) => literal[1]);
  const anyOwner = /matches!\(\s*member,\s*((?:"[A-Za-z]+"\s*\|?\s*)+)\)/.exec(region.body);
  if (anyOwner) for (const name of literals(anyOwner[1])) entries.push({ name, line: lineAt(text, region.offset + anyOwner.index) });
  for (const match of region.body.matchAll(/\(\s*((?:"[A-Za-z]*"\s*\|?\s*)+),\s*((?:"[A-Za-z]+"\s*\|?\s*)+)\)/g)) {
    const line = lineAt(text, region.offset + match.index);
    for (const owner of literals(match[1])) for (const member of literals(match[2])) entries.push({ name: owner ? `${owner}.${member}` : member, line });
  }
  return entries;
}

/** `delivers_synchronously`: `"member" => owner == "Owner"` rows. */
function deliverMembers(text) {
  const region = item(text, /pub\(crate\) fn delivers_synchronously\(/);
  const entries = [];
  for (const match of region.body.matchAll(/^\s*((?:"[A-Za-z]+"(?:\s*\|\s*)?)+)\s*=>/gm)) {
    for (const name of match[1].matchAll(/"([A-Za-z]+)"/g)) entries.push({ name: name[1], line: lineAt(text, region.offset + match.index) });
  }
  return entries;
}

/** The rows of every table: `{ table, name, file, line, ...extra }`. */
export function tableEntries(root = IR_SRC) {
  const rows = [];
  for (const [table, spec] of Object.entries(TABLES)) {
    const path = join(root, spec.file);
    const text = withoutTests(source(path));
    const seen = new Set();
    for (const entry of spec.scan(text)) {
      if (seen.has(entry.name)) continue;
      seen.add(entry.name);
      rows.push({ table, file: spec.file, ...entry });
    }
  }
  return rows;
}

// ---------------------------------------------------------------------------------------------------------------------------------------
// The dialect API tables

/** The questions `impl Dialect for Solid2` answers: `{ id, line }` per `fn` (the trait's methods are the dialect's tables). */
export function dialectQuestions(root = DIALECT_SRC) {
  const path = join(root, "solid_2.rs");
  const text = withoutTests(source(path));
  const start = text.indexOf("impl Dialect for Solid2");
  const open = text.indexOf("{", start);
  const body = text.slice(open + 1, closing(text, open) - 1);
  const base = lineAt(text, open);
  const questions = [];
  for (const match of body.matchAll(/^    fn ([a-z_0-9]+)\(/gm)) questions.push({ id: match[1], line: base + body.slice(0, match.index).split("\n").length - 1 + 1 });
  return questions;
}

// ---------------------------------------------------------------------------------------------------------------------------------------
// Deferrals: the comments in which one check hands a case to another

/** The phrases a comment uses to say that another check decides a case. A hit is not yet a seam: boundary-seams.mjs classifies each. */
export const DEFERRAL_PHRASES = [
  /is the visible side's/, /which \(\d\) decides/, /\bis the census'?s?\b/, /\bcensus's (question|case|job)\b/, /\bhandled by (the|this|that|a) [a-z `\-\[\]:_]+(walk|pass|scan|census|veto|check)\b/,
  /\bleft to (the|other|another|a later) /, /\bdefers? to (the|its|another|a) /, /\bis answered by\b/, /\bis decided by (the|its|another|a) /, /\bthe other side\b/, /\bbelongs to (the|its|another) /,
  /\bis the [a-z-]+ (side|check|pass|walk|scan)'s\b/,
];

const sourceRoots = () => [join(REPO, "rust/crates"), join(REPO, "rust/dialects")];
function checkerSources(path) {
  if (!statSync(path).isDirectory()) return [path];
  return readdirSync(path).sort().flatMap((name) => {
    const full = join(path, name);
    if (statSync(full).isDirectory()) return ["target", "node_modules", "tests"].includes(name) ? [] : checkerSources(full);
    return name.endsWith(".rs") && name !== "tests.rs" && !name.endsWith("_tests.rs") ? [full] : [];
  });
}

/** The comment lines of the checker's own Rust sources (tests left out) that hand a case to another check: `{ file, line, text }`, `file` relative to the repository. */
export function deferrals() {
  const found = [];
  for (const root of sourceRoots()) {
    for (const path of checkerSources(root)) {
      withoutTests(source(path)).split("\n").forEach((text, index) => {
        if (/^\s*\/\/[/!]?/.test(text) && DEFERRAL_PHRASES.some((phrase) => phrase.test(text))) found.push({ file: relative(REPO, path), line: index + 1, text: text.trim() });
      });
    }
  }
  return found;
}
