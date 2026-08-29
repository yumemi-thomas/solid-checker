// A content-addressed result cache for the verification gates.
//
// The governing rule: **a stale green gate is worse than a slow one.** A cache
// that can replay a result the current inputs would not produce does not make
// verification faster, it makes it decorative. So the key here is not a
// heuristic over "files that probably matter"; it is a digest over every input
// class a unit's result can depend on, and anything it cannot see must not be
// an input.
//
// What a key covers, per unit:
//
//   (a) the unit's own tree on disk -- every file under the fixture project,
//       walked from the filesystem rather than from `git ls-files`, because an
//       untracked file changes the answer exactly as much as a tracked one;
//   (b) the binaries the unit executes: the checker (which carries
//       `pkg/contracts/bundled/**` compiled in via `include_bytes!`), the
//       TypeFacts producer, and its `.buildinfo` stamp;
//   (c) the gate script and every local module it can reach, plus all of
//       `scripts/lib/**` unconditionally -- import tracing is an
//       over-approximation on purpose, and an unresolvable dynamic import
//       widens it to every gate script rather than narrowing the key;
//   (d) every `SOLID_*` environment variable, name and value;
//   (e) the JavaScript runtime identity (`Bun.version` plus its compatible
//       `process.version`, or Node's `process.version` for legacy callers);
//   (f) a format-version constant, so a change to what an entry *means*
//       invalidates every entry rather than being misread;
//   (g) `options.trees` -- directory trees the gate *executes* but does not
//       own, the CLI it spawns being the case that matters. The contract
//       corpus's whole soundness argument is this class: its generator lives in
//       `packages/cli`, which no unit tree and no traced import covers;
//   (h) per unit, whatever ancestor chain the unit's *own* answer depends on --
//       `ancestorChainDigest` exists because dialect selection walks upward out
//       of the unit tree, so "the unit's tree" is not by itself the unit's
//       input set.
//
// What a key deliberately does **not** cover: the expected artifact a gate
// compares against -- snapshots, `expected.json`. The cached value is the *raw
// computed result*, never a pass/fail verdict, and comparison always runs fresh
// against the files on disk. So editing a snapshot needs no cache awareness at
// all, and a mismatch still fails on a warm cache. That split is what makes the
// key auditable: it only has to cover what *computes* the result.
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  readlinkSync,
  renameSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, relative, resolve, sep } from "node:path";
import process from "node:process";

/**
 * Bumped when the *meaning* of a stored entry changes -- a different value
 * shape, a different digest composition, a fix to what an input class covers.
 * Every existing entry becomes unreadable, which is the intent: a wrong entry
 * must never be replayed by a correct reader.
 */
export const CACHE_FORMAT_VERSION = 3;

const bunVersion = globalThis.Bun?.version;
export const runtimeIdentity = bunVersion
  ? `bun:${bunVersion}:${process.version}`
  : `node:${process.version}`;

/**
 * The cache's own controls, excluded from the environment digest.
 *
 * These select *whether and how fast* the cache runs, never what a unit
 * computes: the kill switch gates storage, and concurrency only schedules
 * independent child processes. Including them would make an explicit
 * `SOLID_CHECKER_GATE_CACHE=1` a different key from the default, so proving a
 * cached run matches an uncached one would be impossible. Every other
 * `SOLID_*` variable is in the key. Add to this list only for a variable that
 * provably cannot change a result.
 */
export const CACHE_CONTROL_VARIABLES = ["SOLID_CHECKER_GATE_CACHE", "SOLID_CHECKER_GATE_CONCURRENCY"];

const sha256 = (parts) => {
  const hash = createHash("sha256");
  for (const part of parts) {
    hash.update(String(part));
    hash.update("\0");
  }
  return hash.digest("hex");
};

/** A file's content digest, or an explicit `absent` marker. */
export function hashFile(path) {
  if (!existsSync(path)) return "absent";
  return `sha256:${createHash("sha256").update(readFileSync(path)).digest("hex")}`;
}

/**
 * A digest over an entire directory tree: sorted repository-relative paths,
 * each with its content digest.
 *
 * Walked from the filesystem, so an untracked or ignored file -- a `node_modules`
 * dialect stub, a `.solid-checker/runtime.json`, a stray edit -- counts.
 *
 * A symlink contributes its target text rather than being followed, and what
 * makes that safe is not that the gates' trees contain none -- they do:
 * `packages/cli/node_modules/.bin/` holds five (`tsc`, `tsserver`, `eslint`,
 * `acorn`, `node-which`). It is that every one of them points *inside the same
 * hashed tree*, so the content behind the link is walked anyway, while the link
 * text still distinguishes a re-pointed link from an unchanged one. Following
 * links instead would risk a cycle or an unbounded walk out of the tree while
 * pretending to be exhaustive. A link that escaped the tree would be a genuine
 * hole, so keep the trees self-contained.
 *
 * Ordering is by UTF-8 bytes, not by `localeCompare`: a locale-sensitive
 * comparator makes the digest depend on `LANG`/`LC_ALL`, which are not in the
 * key, and can return 0 for two distinct names -- at which point the order
 * falls back to whatever `readdir` returned.
 *
 * One input class the walk cannot express: an **empty directory** contributes
 * nothing, so creating or deleting one does not move the digest. Stated rather
 * than fixed, because a directory has no content to hash and its emptiness is
 * exactly what the consumers treat as absence -- `dialect.rs` reads an empty
 * `node_modules/solid-js/` the same way it reads a missing one.
 */
export function hashTree(directory, { skip } = {}) {
  if (!existsSync(directory)) return "absent";
  const entries = [];
  const byBytes = (a, b) => Buffer.compare(Buffer.from(a.name, "utf8"), Buffer.from(b.name, "utf8"));
  const walk = (current) => {
    for (const entry of readdirSync(current, { withFileTypes: true }).sort(byBytes)) {
      const full = join(current, entry.name);
      const id = relative(directory, full).split(sep).join("/");
      if (skip?.(id)) continue;
      if (entry.isSymbolicLink()) entries.push(`${id}link:${readlinkSync(full)}`);
      else if (entry.isDirectory()) walk(full);
      else if (entry.isFile()) entries.push(`${id}${hashFile(full)}`);
      else entries.push(`${id}other`);
    }
  };
  walk(directory);
  return `tree:${sha256(entries)}`;
}

/**
 * A digest over one file's existence and content at *every* ancestor of `start`,
 * from `start` itself up to the filesystem root.
 *
 * This exists for dialect selection, and the reason is worth stating in full
 * because "the unit's tree is the unit's input set" reads as obviously true and
 * is not. `resolved_solid_version`
 * (rust/crates/solid-facts-backend/src/dialect.rs) walks `start.ancestors()` --
 * unbounded, past the repository root, to `/` -- looking for the nearest
 * `node_modules/solid-js/package.json`, and the version it finds there decides
 * which dialect catalog runs. Roughly half the fixture projects ship no stub and
 * depend on the *absence* of one above them. That absence is an input, and a
 * tree digest of the project directory cannot see it: a stray
 * `bun install solid-js` one directory above the checkout, or in `$HOME`, flips
 * every stub-less project to the v1 catalog while leaving every key untouched.
 * A cold run would fail loudly; a warm one would replay 83 hits and print green.
 *
 * Ancestors are identified by *depth* rather than by absolute path, so the key
 * survives moving the checkout -- the shape of the chain above a unit is the
 * input, not where that chain happens to live.
 *
 * Cost is one `stat` per ancestor per unit, so a few hundred for a whole gate.
 */
export function ancestorChainDigest(start, relativePath) {
  const segments = relativePath.split("/");
  const parts = [];
  let current = resolve(start);
  for (let depth = 0; ; depth += 1) {
    parts.push(`${depth}:${hashFile(join(current, ...segments))}`);
    const parent = dirname(current);
    if (parent === current) break;
    current = parent;
  }
  return `chain:${relativePath}:${sha256(parts)}`;
}

const RELATIVE_SPECIFIER = /["'](\.\.?\/[^"'\n]*)["']/g;
const DYNAMIC_IMPORT = /\bimport\s*\(\s*([^)]*)\)/g;
const LITERAL_ARGUMENT = /^\s*["'][^"']*["']\s*$/;

/**
 * Every local module a gate script's result can depend on.
 *
 * Deliberately an over-approximation. It collects *any* relative-looking string
 * literal that resolves to a file, not just the ones a parser would call an
 * import, and it always seeds with all of `scripts/lib/**`. Over-collecting
 * only invalidates entries that would have stayed valid; under-collecting
 * replays a result the current code would not produce, which is the one
 * outcome this module exists to prevent.
 *
 * A dynamic `import()` whose argument is not a string literal makes tracing
 * uncertain, and uncertainty fails closed: the closure widens to every
 * non-test script under `scripts/`.
 */
export function scriptClosure(entry, { root = resolve(import.meta.dirname, "..", "..") } = {}) {
  const libraryDirectory = join(root, "scripts", "lib");
  const files = new Set();
  const seed = (path) => {
    if (existsSync(path) && statSync(path).isFile()) files.add(resolve(path));
  };
  const collectDirectory = (directory, accept) => {
    if (!existsSync(directory)) return;
    for (const found of readdirSync(directory, { withFileTypes: true })) {
      const full = join(directory, found.name);
      if (found.isDirectory()) collectDirectory(full, accept);
      else if (found.isFile() && accept(found.name)) seed(full);
    }
  };
  collectDirectory(libraryDirectory, () => true);

  let uncertain = false;
  const pending = [resolve(entry)];
  const visited = new Set();
  while (pending.length > 0) {
    const current = pending.pop();
    if (visited.has(current)) continue;
    visited.add(current);
    if (!existsSync(current) || !statSync(current).isFile()) continue;
    seed(current);
    let source;
    try {
      source = readFileSync(current, "utf8");
    } catch {
      uncertain = true;
      continue;
    }
    for (const match of source.matchAll(DYNAMIC_IMPORT)) {
      // An empty argument list is not code -- `import()` takes a specifier --
      // so it is prose about dynamic imports, including this file's own. A
      // *comment* that names one still widens the closure, which is the safe
      // direction and not worth a comment-stripping parser to avoid.
      if (match[1].trim() === "") continue;
      if (!LITERAL_ARGUMENT.test(match[1])) uncertain = true;
    }
    for (const match of source.matchAll(RELATIVE_SPECIFIER)) {
      const candidate = resolve(dirname(current), match[1]);
      if (existsSync(candidate) && statSync(candidate).isFile()) pending.push(candidate);
    }
  }
  if (uncertain) {
    collectDirectory(join(root, "scripts"), (name) => name.endsWith(".mjs") && !name.endsWith(".test.mjs"));
  }
  return { files: [...files].sort(), uncertain };
}

/** Every `SOLID_*` variable, name and value, minus the cache's own controls. */
export function environmentDigest(env = process.env) {
  const names = Object.keys(env)
    .filter((name) => name.startsWith("SOLID_") && !CACHE_CONTROL_VARIABLES.includes(name))
    .sort();
  return names.map((name) => `${name}=${env[name]}`);
}

/** Whether the cache reads and writes at all. */
export function cacheEnabled(env = process.env) {
  const raw = env.SOLID_CHECKER_GATE_CACHE;
  if (raw === undefined) return true;
  return !["0", "false", "off", "no"].includes(raw.trim().toLowerCase());
}

/** Write JSON so a reader never observes a partial file, and never a torn one. */
export function writeJsonAtomic(path, value) {
  mkdirSync(dirname(path), { recursive: true });
  const temporary = `${path}.${process.pid}.${Math.random().toString(36).slice(2)}.tmp`;
  writeFileSync(temporary, `${JSON.stringify(value)}\n`);
  renameSync(temporary, path);
}

/**
 * Open one gate's cache.
 *
 * @param {object} options
 * @param {string} options.gate      Namespace; one subdirectory per gate.
 * @param {string} options.scriptPath The gate script, for input class (c).
 * @param {string[]} options.binaries Executables and stamps, for class (b).
 * @param {string[]} options.trees    Directory trees whose content the gate
 *                                    executes but does not own (the CLI it
 *                                    spawns, for instance).
 * @param {string[]} options.extra    Any further run-wide digest material.
 * @param {{files: string[], uncertain: boolean}} [options.closure]
 *        The traced script closure. Injectable so a test can hold `files` fixed
 *        and vary only `uncertain`, which is the only way to prove the
 *        uncertainty flag is really in the digest rather than being shadowed by
 *        the file set that produced it.
 */
export function openGateCache({
  gate,
  scriptPath,
  binaries = [],
  trees = [],
  extra = [],
  env = process.env,
  root = resolve(import.meta.dirname, "..", ".."),
  enabled = cacheEnabled(env),
  closure = scriptClosure(scriptPath, { root }),
}) {
  const shared = sha256([
    `format:${CACHE_FORMAT_VERSION}`,
    `gate:${gate}`,
    runtimeIdentity,
    ...binaries.map((path) => `binary:${relative(root, path)}:${hashFile(path)}`),
    ...trees.map((path) => `tree:${relative(root, path)}:${hashTree(path)}`),
    ...closure.files.map((path) => `script:${relative(root, path)}:${hashFile(path)}`),
    `script-closure-uncertain:${closure.uncertain}`,
    ...environmentDigest(env).map((pair) => `env:${pair}`),
    ...extra.map((part) => `extra:${part}`),
  ]);
  const directory = join(root, "rust", "target", "gate-cache", gate);
  let hits = 0;
  let misses = 0;
  let skipped = 0;

  const keyFor = (parts) => sha256([`shared:${shared}`, ...parts.map((part) => `unit:${part}`)]);

  // A part that is a digest of something on disk cannot be handed over as a
  // fixed array: it was computed before `compute()` ran, so nothing can tell
  // whether the tree it described is the tree the unit was actually run
  // against. Recognizing the digest shapes this module produces turns "pass a
  // thunk" from advice into a rule -- the mistake is not available.
  const FILESYSTEM_DIGEST = /^(tree:|chain:|sha256:|absent$)/;
  const refuseEagerDigest = (parts) => {
    const offender = parts.find((part) => FILESYSTEM_DIGEST.test(String(part)));
    if (offender === undefined) return;
    throw new Error(
      `gate cache (${gate}): unit part ${JSON.stringify(String(offender).slice(0, 24))} is a ` +
        `filesystem digest, so it must be passed as a thunk -- \`cache.run(() => [...], compute)\` ` +
        `-- not as a fixed array. An eagerly-built digest is taken before compute() reads the ` +
        `files, so a tree edited mid-run is stored under the pre-edit key with the post-edit ` +
        `result, and reverting the edit replays it.`,
    );
  };
  const pathFor = (key) => join(directory, `${key}.json`);

  const read = (key) => {
    const file = pathFor(key);
    if (!existsSync(file)) return undefined;
    try {
      const entry = JSON.parse(readFileSync(file, "utf8"));
      // Anything but an entry of exactly the expected shape is a miss. A
      // mismatched format or key is a stale or foreign entry; a *missing*
      // `value` is a truncated or hand-edited one, and treating it as a hit
      // replays `undefined` as a result and fails as a `TypeError` in the
      // comparison loop, far from the cause.
      if (entry === null || typeof entry !== "object" || Array.isArray(entry)) return undefined;
      if (entry.formatVersion !== CACHE_FORMAT_VERSION) return undefined;
      if (entry.key !== key || entry.gate !== gate) return undefined;
      if (!Object.hasOwn(entry, "value")) return undefined;
      return entry;
    } catch {
      return undefined;
    }
  };

  return {
    enabled,
    gate,
    sharedDigest: shared,
    directory,
    key: keyFor,
    get hits() {
      return hits;
    },
    get misses() {
      return misses;
    },
    get skipped() {
      return skipped;
    },
    /**
     * The unit's result: replayed when every input digest matches, otherwise
     * computed and stored.
     *
     * `compute` throwing stores nothing. That is the rule "never cache a unit
     * whose process exited unexpectedly", implemented by construction rather
     * than by remembering to check.
     *
     * `unitParts` may be an array or a **thunk returning one**, and the
     * difference is a correctness one, not a convenience:
     *
     *   * An array is a promise from the caller that these parts do not depend
     *     on mutable state. Nothing can be re-verified about it, and nothing
     *     needs to be -- and an array carrying a `hashTree`/`hashFile`/
     *     `ancestorChainDigest` result is refused outright rather than trusted,
     *     because that promise would be false.
     *   * A thunk is what a caller must pass when a part is a digest of
     *     something on disk. The digest is taken *before* `compute()` reads the
     *     files, so an edit that lands mid-run would otherwise be stored under
     *     the pre-edit key with the post-edit result -- and reverting the edit
     *     makes that entry a hit, replaying a result the current bytes do not
     *     produce. This change makes the loop fast, so it will be run more
     *     often mid-edit, which is exactly the condition that triggers it. So a
     *     thunk is re-evaluated after `compute()` and the entry is stored only
     *     if the key is unchanged; a moved key means the run observed a tree
     *     nobody can name, and the honest record of it is no record at all.
     */
    async run(unitParts, compute) {
      const evaluate = typeof unitParts === "function" ? unitParts : () => unitParts;
      if (typeof unitParts !== "function") refuseEagerDigest(unitParts);
      const key = keyFor(evaluate());
      if (enabled) {
        const entry = read(key);
        if (entry !== undefined) {
          hits += 1;
          return { value: entry.value, hit: true, key };
        }
      }
      const value = await compute();
      misses += 1;
      const after = keyFor(evaluate());
      const stable = after === key;
      if (!stable) skipped += 1;
      if (enabled && stable) {
        writeJsonAtomic(pathFor(key), {
          formatVersion: CACHE_FORMAT_VERSION,
          gate,
          key,
          createdAt: new Date().toISOString(),
          value,
        });
      }
      return { value, hit: false, key, stored: enabled && stable };
    },
    summary() {
      if (!enabled) return `gate cache: disabled (SOLID_CHECKER_GATE_CACHE)`;
      const unstable = skipped > 0 ? `, ${skipped} not stored (inputs changed mid-run)` : "";
      return `gate cache: ${hits} hit(s), ${misses} miss(es)${unstable}`;
    },
  };
}
