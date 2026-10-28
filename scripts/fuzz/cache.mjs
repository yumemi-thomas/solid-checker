// The baseline cache of the neutrality regression: the uninstrumented pass is a pure function of the
// program (generator version, seed, profile, component count), the checker (binary and type-facts producer),
// the fast path's install (its lockfile) and the code that judges it (this directory's non-test sources). When
// all of those match, a recorded row is as good as running the program again, so rounds after the first
// reuse it. Anything that changes the key (a rebuilt checker, a new lockfile, an edit to classify.mjs) makes
// the old rows unreachable; nothing is ever patched in place.
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, readdirSync, realpathSync, renameSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { GENERATOR_VERSION } from "./generate.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const repo = join(here, "../..");

export const sha256 = (data) => createHash("sha256").update(data).digest("hex");
/** sha256 of a file's bytes, or `missing` when it is not there. */
export const fileHash = (path) => (path && existsSync(path) ? sha256(readFileSync(path)) : "missing");

/** A hash of the sources that decide a verdict: every non-test module and fast-path file in scripts/fuzz. */
export function codeHash(directory = here) {
  const files = [];
  const visit = (dir, prefix = "") => {
    for (const entry of readdirSync(dir, { withFileTypes: true }).sort((a, b) => (a.name < b.name ? -1 : 1))) {
      if (entry.name === "node_modules" || entry.name.startsWith(".") || entry.name === "install") continue;
      if (entry.isDirectory()) visit(join(dir, entry.name), `${prefix}${entry.name}/`);
      else if (/\.(mjs|tsx?|json)$/.test(entry.name) && !/\.test\.mjs$/.test(entry.name) && entry.name !== "neutrality.mjs") files.push([`${prefix}${entry.name}`, join(dir, entry.name)]);
    }
  };
  visit(directory);
  return sha256(files.map(([name, path]) => `${name}\n${fileHash(path)}`).join("\n"));
}

/**
 * The environment half of a cache key.
 * @param {{ bin?: string, typefacts?: string, fastInstall: string, install: string }} config
 */
export function environmentKey(config, { code = codeHash(), oracle = fileHash(join(repo, "scripts/runtime-oracle.mjs")), installed = installedKey(config) } = {}) {
  return sha256(JSON.stringify({
    // runtime-oracle.mjs: the checker reads its entry (checker.mjs MAIN_SOURCE); tsc: the baseline rows hold the type check's answer.
    generator: GENERATOR_VERSION, code, oracle, installed, tsc: fileHash(config.tsc), virtualClock: Boolean(config.virtualClock),
    checker: fileHash(config.bin), typefacts: fileHash(config.typefacts),
    fastLock: fileHash(join(config.fastInstall, "package-lock.json")), oracleLock: fileHash(join(config.install, "package-lock.json")),
  }));
}

/**
 * The program half of a key. With `files` (a built case's), the hash of its text too: the seed alone does not say which program it
 * is (`--containers`, the misuse pins and the other generator flags draw a different program from the same seed).
 */
export const programKey = ({ seed, profile, components, files }) => `${profile ?? "-"}|${components ?? 1}|${seed}${files ? `|${caseKey(files, "program").slice(0, 32)}` : ""}`;

/** Rows by program key, in one file per environment key. */
export class BaselineCache {
  constructor(directory, key) {
    this.path = join(directory, `baseline-${key.slice(0, 24)}.json`);
    this.key = key;
    this.rows = {};
    this.hits = 0;
    this.misses = 0;
    if (existsSync(this.path)) {
      try {
        const stored = JSON.parse(readFileSync(this.path, "utf8"));
        if (stored.key === key) this.rows = stored.rows ?? {};
      } catch { /* an unreadable cache is an empty one */ }
    }
  }

  get(program) {
    const row = this.rows[programKey(program)];
    if (row) this.hits += 1; else this.misses += 1;
    return row ?? null;
  }

  set(program, row) { this.rows[programKey(program)] = row; }

  save() {
    mkdirSync(dirname(this.path), { recursive: true });
    const temporary = `${this.path}.${process.pid}`;
    writeFileSync(temporary, JSON.stringify({ key: this.key, rows: this.rows }));
    renameSync(temporary, this.path);
  }
}

/** Where a round remembers what mismatched last time (profiles and programs). */
export class MismatchHistory {
  constructor(directory) {
    this.path = join(directory, "mismatch-history.json");
    this.data = { profiles: {}, programs: {} };
    if (existsSync(this.path)) {
      try { this.data = { profiles: {}, programs: {}, ...JSON.parse(readFileSync(this.path, "utf8")) }; } catch { /* start empty */ }
    }
  }

  /** Items first that mismatched last time, then the rest in their given order (a stable sort). */
  order(items, keyOf, bucket) {
    const failed = (item) => (this.data[bucket][keyOf(item)] ? 0 : 1);
    return items.map((item, index) => ({ item, index })).sort((a, b) => failed(a.item) - failed(b.item) || a.index - b.index).map(({ item }) => item);
  }

  record(bucket, key, mismatched) {
    if (mismatched) this.data[bucket][key] = true; else delete this.data[bucket][key];
  }

  save() {
    mkdirSync(dirname(this.path), { recursive: true });
    writeFileSync(this.path, JSON.stringify(this.data, null, 2));
  }
}

// ---------------------------------------------------------------------------------------------------------------------
// The runtime and Chrome caches
//
// Solid's answer on a program does not depend on the checker: the type check (tsrs against the published typings), the fast path's
// record and Chrome's runtime record are functions of the program's case files, the pinned installs (their lockfiles), the type checker,
// the browser and the sources that drive and judge the run. A run after a checker fix reuses them and reruns only the checker. One
// file per entry (`<dir>/<kind>-<environment key>/<ab>/<key>.json`, written by rename), so parallel processes never lose each other's
// entries. A record the machine produced (a runner that wrote nothing, a type check that timed out, a browser that would not close)
// is never stored: it says nothing about the program.

/** sha256 over the files below `directory` whose names pass `keep` (sorted relative paths and their bytes). */
export function treeHash(directory, keep = () => true) {
  const files = [];
  const visit = (dir, prefix = "") => {
    if (!existsSync(dir)) return;
    for (const entry of readdirSync(dir, { withFileTypes: true }).sort((a, b) => (a.name < b.name ? -1 : 1))) {
      if (entry.name === "node_modules" || entry.name.startsWith(".")) continue;
      if (entry.isDirectory()) visit(join(dir, entry.name), `${prefix}${entry.name}/`);
      else if (keep(`${prefix}${entry.name}`)) files.push(`${prefix}${entry.name}\n${fileHash(join(dir, entry.name))}`);
    }
  };
  visit(directory);
  return sha256(files.join("\n"));
}

/**
 * What is really installed below `directory` (an install's node_modules, a browser build): every file's path, size and modification
 * time, symlinks followed. A lockfile says what should be there; this says what is, so a reinstall, a half-finished `npm ci` or an
 * edited dist file all move the key. A stat walk, not a byte hash: about a tenth of the time, and any npm write moves an mtime.
 */
export function treeStat(directory) {
  if (!directory || !existsSync(directory)) return "missing";
  const hash = createHash("sha256");
  const seen = new Set();
  const visit = (dir, prefix) => {
    const real = realpathSync(dir);
    if (seen.has(real)) return;
    seen.add(real);
    for (const name of readdirSync(dir).sort()) {
      const path = join(dir, name);
      let info;
      try { info = statSync(path); } catch { hash.update(`${prefix}${name}\0dangling\n`); continue; }
      if (info.isDirectory()) visit(path, `${prefix}${name}/`);
      else hash.update(`${prefix}${name}\0${info.size}\0${info.mtimeMs}\n`);
    }
  };
  visit(directory, "");
  return hash.digest("hex");
}

/** The runner's settings from the environment (fast/runner.tsx reads them): they change what a record holds. */
const RUNNER_ENV = ["FUZZ_PAUSE_MS", "FUZZ_FINAL_MS"];

/**
 * The part of every cache key that says what the run executes with, beyond the lockfiles: Node's version, the runner's environment
 * settings, and what is installed in the fast and oracle installs.
 */
export function installedKey(config, { env = process.env, node = process.version } = {}) {
  return sha256(JSON.stringify({
    node, env: Object.fromEntries(RUNNER_ENV.map((name) => [name, env[name] ?? null])),
    fast: config.fastInstall ? treeStat(join(config.fastInstall, "node_modules")) : "none",
    oracle: config.install ? treeStat(join(config.install, "node_modules")) : "none",
  }));
}

/** The oracle and the product scripts it drives Chrome with: they decide what Chrome's record holds. */
export const ORACLE_SOURCES = () => sha256(JSON.stringify([
  fileHash(join(repo, "scripts/runtime-oracle.mjs")),
  // `feedback run`: the CLI's entry, its library and its scripts (the browser driver), and what it has installed.
  ...["bin", "lib", "scripts"].map((dir) => treeHash(join(repo, "packages/cli", dir), (name) => /\.(m?js|cjs|json)$/.test(name) && !/\.test\./.test(name))),
  fileHash(join(repo, "packages/cli/package.json")), fileHash(join(repo, "packages/cli/bun.lock")), fileHash(join(repo, "packages/cli/package-lock.json")),
  treeStat(join(repo, "packages/cli/node_modules")),
]));

/**
 * The environment of the fast path's record and the type check: the generator, the judging sources (`codeHash`), the oracle's entry
 * (the type check compiles its main.tsx), the clock, the type checker binary and both lockfiles. Not the checker.
 */
export function runtimeEnvironmentKey(config, { code = codeHash(), oracle = fileHash(join(repo, "scripts/runtime-oracle.mjs")), installed = installedKey(config) } = {}) {
  return sha256(JSON.stringify({
    kind: "runtime", generator: GENERATOR_VERSION, code, oracle, installed, virtualClock: Boolean(config.virtualClock), tsc: fileHash(config.tsc),
    fastLock: fileHash(join(config.fastInstall, "package-lock.json")), oracleLock: fileHash(join(config.install, "package-lock.json")),
  }));
}

/** The environment of a Chrome record: the judging sources, the oracle and the product scripts it runs, the browser, the oracle install's lockfile. Not the checker. */
export function chromeEnvironmentKey(config, { code = codeHash(), oracle = ORACLE_SOURCES(), browser = browserKey(config.browser), installed = installedKey({ install: config.install }) } = {}) {
  return sha256(JSON.stringify({ kind: "chrome", code, oracle, browser, installed, oracleLock: fileHash(join(config.install, "package-lock.json")) }));
}

/** The browser: `--browser`'s file, else the build the oracle picks; its path and every file of its build directory (treeStat). */
export function browserKey(explicit, home = process.env.HOME) {
  if (explicit) return JSON.stringify([explicit, treeStat(dirname(explicit))]);
  const name = defaultBrowserName(home);
  return JSON.stringify([name, name === "none" ? "none" : treeStat(join(home, "Library/Caches/ms-playwright", name))]);
}

/** The browser the oracle picks without `--browser` (scripts/runtime-oracle.mjs `defaultChromium`): its build directory names it. */
export function defaultBrowserName(home = process.env.HOME) {
  const cache = join(home ?? "", "Library/Caches/ms-playwright");
  if (!existsSync(cache)) return "none";
  const builds = readdirSync(cache).filter((name) => /^chromium_headless_shell-\d+$/.test(name)).sort((a, b) => Number(b.split("-")[1]) - Number(a.split("-")[1]));
  return builds[0] ?? "none";
}

/** The program half of a runtime or Chrome key: what is asked (`part`) of which files (a case's, by name and text). */
export const caseKey = (files, part) => sha256(JSON.stringify([part, Object.entries(files).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))]));

/** The files of a case directory as `{ relative path: text }` (what `caseKey` hashes for a staged case). */
export function caseFiles(directory) {
  const files = {};
  const visit = (dir, prefix = "") => {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      if (entry.isDirectory()) visit(join(dir, entry.name), `${prefix}${entry.name}/`);
      else files[`${prefix}${entry.name}`] = readFileSync(join(dir, entry.name), "utf8");
    }
  };
  visit(directory);
  return files;
}

/** Spot checks per process and per part: that many cache hits are also run fresh and compared (`EntryCache.takeSpot`). */
export const SPOT_CHECKS = { typecheck: 3, fast: 3, chrome: 1 };

/** A cache hit that a fresh run contradicts: something the key does not cover changed. The run stops rather than use it. */
export class StaleCacheError extends Error {}

/**
 * What a runtime record says, for comparing a stored record with a fresh one: every diagnostic with its positions, every throw, the
 * witnesses, the hit counts and their order, quiescence, the failure's phase and the unattributed codes. Wall times are left out.
 */
export function recordDigest(runtime) {
  if (!runtime) return "none";
  const lines = (positions) => (positions ?? []).map((position) => `${position.file ?? "-"}:${position.line ?? "-"}`).join(",");
  return JSON.stringify({
    failure: runtime.failure?.phase ?? null,
    diagnostics: (runtime.diagnostics ?? []).map((d) => `${d.code}@${lines(d.positions)}`).sort(),
    errors: (runtime.errors ?? []).map((e) => `${e.kind}:${String(e.message ?? "").slice(0, 200)}@${lines(e.positions)}`).sort(),
    witnesses: (runtime.witnesses ?? []).map((w) => `${w.siteId}:${w.present}:${w.actual ?? null}:${w.reached}`).sort(),
    hits: runtime.hits ?? null, order: runtime.order ?? null, quiescent: runtime.quiescent ?? null,
    unattributed: [...(runtime.unattributed ?? [])].map(String).sort(),
  });
}

/** What a type check says, for the same comparison: its answer and its error codes (paths are the work directory's). */
export const typecheckDigest = (typed) => JSON.stringify({ ok: Boolean(typed?.ok), codes: (typed?.errors ?? []).map((line) => /error (TS\d+)/.exec(line)?.[1] ?? line).sort() });

/** Entries by key, one file each, under one environment key. `kind` names the cache (`runtime`, `chrome`). */
export class EntryCache {
  constructor(directory, kind, environment, { spots = SPOT_CHECKS } = {}) {
    this.root = join(directory, `${kind}-${environment.slice(0, 24)}`);
    this.environment = environment;
    this.hits = 0;
    this.misses = 0;
    this.stored = 0;
    this.spots = { ...spots };
    this.spotChecked = 0;
    this.spotMismatches = [];
  }

  /** Whether this hit of `part` is one to run fresh and compare (the first few per process). */
  takeSpot(part) {
    if (!(this.spots[part] > 0)) return false;
    this.spots[part] -= 1;
    this.spotChecked += 1;
    return true;
  }

  pathOf(key) { return join(this.root, key.slice(0, 2), `${key}.json`); }

  get(key) {
    const path = this.pathOf(key);
    if (existsSync(path)) {
      try {
        const stored = JSON.parse(readFileSync(path, "utf8"));
        if (stored.environment === this.environment && stored.key === key) { this.hits += 1; return stored.value; }
      } catch { /* an unreadable entry is a miss */ }
    }
    this.misses += 1;
    return undefined;
  }

  /** Whether an entry is stored under `key` (counts no hit or miss). */
  has(key) { return existsSync(this.pathOf(key)); }

  set(key, value) {
    const path = this.pathOf(key);
    mkdirSync(dirname(path), { recursive: true });
    const temporary = `${path}.${process.pid}`;
    writeFileSync(temporary, JSON.stringify({ environment: this.environment, key, value }));
    renameSync(temporary, path);
    this.stored += 1;
  }

  stats() { return { hits: this.hits, misses: this.misses, stored: this.stored, spotChecked: this.spotChecked, spotMismatches: this.spotMismatches, root: this.root }; }
}

/** A type check that says something about the program (not a missing binary, not a timeout: a killed tsrs prints no error and a non-zero status). */
export const cacheableTypecheck = (typed) => Boolean(typed) && !typed.missing && (typed.ok || typed.errors?.length > 0);
/** A fast-path record that says something about the program: the runner wrote it (a vitest record is the harness's). */
export const cacheableRecord = (record) => Boolean(record) && record.failure?.phase !== "vitest";

/** Where the caches live: `--cache-dir`, else `$TMPDIR/solid-checker-fuzz-cache` (the neutrality baseline's default too). */
export const cacheDirectory = (options) => resolve(options.cacheDir ?? join(tmpdir(), "solid-checker-fuzz-cache"));

/** The Chrome cache of a run, or null with `--no-chrome-cache`. */
export const chromeCacheFor = (config, options) => (options.noChromeCache ? null : new EntryCache(cacheDirectory(options), "chrome", chromeEnvironmentKey({ ...config, browser: options.browser ? resolve(options.browser) : undefined })));

/**
 * The runtime cache of a run (matrix.mjs, run.mjs): Solid's answer on a program (its type checks and fast-path records) does not depend on
 * the checker, so a run after a checker fix takes them from the cache and reruns only the checker. Off with `--no-cache` and on the real
 * clock (whose records depend on the machine).
 */
export function withRuntimeCache(config, options) {
  if (options.noCache || !config.virtualClock) return config;
  return { ...config, runtimeCache: new EntryCache(cacheDirectory(options), "runtime", runtimeEnvironmentKey(config)) };
}
