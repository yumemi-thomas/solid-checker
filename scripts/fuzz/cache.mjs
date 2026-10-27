// The baseline cache of the neutrality regression: the uninstrumented pass is a pure function of the
// program (generator version, seed, profile, component count), the checker (binary and type-facts producer),
// the fast path's install (its lockfile) and the code that judges it (this directory's non-test sources). When
// all of those match, a recorded row is as good as running the program again, so rounds after the first
// reuse it. Anything that changes the key (a rebuilt checker, a new lockfile, an edit to classify.mjs) makes
// the old rows unreachable; nothing is ever patched in place.
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, readdirSync, renameSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { GENERATOR_VERSION } from "./generate.mjs";

const here = dirname(fileURLToPath(import.meta.url));

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
export function environmentKey(config, { code = codeHash() } = {}) {
  return sha256(JSON.stringify({
    generator: GENERATOR_VERSION, code,
    checker: fileHash(config.bin), typefacts: fileHash(config.typefacts),
    fastLock: fileHash(join(config.fastInstall, "package-lock.json")), oracleLock: fileHash(join(config.install, "package-lock.json")),
  }));
}

/** The program half of a key. */
export const programKey = ({ seed, profile, components }) => `${profile ?? "-"}|${components ?? 1}|${seed}`;

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
