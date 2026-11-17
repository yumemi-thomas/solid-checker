// A content-addressed cache for the browser half of a live feedback run
// (`feedback run --execution-cache <dir>`).
//
// The browser half -- Vite, Chromium, the scenario, frame attribution -- is
// most of a run's cost, and it does not read the native checker except
// through the development models (`feedbackFacts`) that instrument the
// sources, and the TypeScript error count that switches that instrumentation
// off. So the key is everything the half reads: the project's manifest and
// sources, those models and that count, the scenario and supplied responses,
// the runtime options, the browser binary, the Vite/Playwright tooling, this
// harness's own code, and Node. What it cannot name in advance -- every file
// Vite served or the configuration loaded -- it recorded as runtime-input pins,
// and a read re-hashes each one: a changed or missing file is a miss.
//
// What reads the checker's findings (the report, the observations, automatic
// warnings) is not cached: the caller recomputes it from the fresh analysis
// and the cached capture. A run whose scenario failed is not stored, so a
// timeout under load is never replayed.
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, readdirSync, realpathSync, renameSync, statSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

/** Bumped when what an entry means changes; older entries become misses. */
export const EXECUTION_CACHE_FORMAT = 2;

const sha256 = bytes => `sha256:${createHash("sha256").update(bytes).digest("hex")}`;

/** Every file of this harness (the CLI's scripts directory), by name and content. */
export function harnessDigest(directory = dirname(fileURLToPath(import.meta.url))) {
  const hash = createHash("sha256");
  for (const name of readdirSync(directory).sort()) {
    const path = join(directory, name);
    if (!statSync(path).isFile()) continue;
    hash.update(name).update("\0").update(readFileSync(path)).update("\0");
  }
  return `sha256:${hash.digest("hex")}`;
}

/** The exact tooling packages a run loads from `root`, by manifest content. */
export function toolingIdentity(root, names = ["vite", "@solidjs/vite-plugin", "playwright", "playwright-core", "@jridgewell/trace-mapping"]) {
  const require = createRequire(join(root, "package.json"));
  return names.map(name => {
    let resolved;
    try { resolved = require.resolve(name); } catch { return { name, missing: true }; }
    for (let directory = dirname(resolved); dirname(directory) !== directory; directory = dirname(directory)) {
      const metadata = join(directory, "package.json");
      if (existsSync(metadata) && JSON.parse(readFileSync(metadata, "utf8")).name === name) {
        // ADR 1029: where the tooling sits is not what it is: the manifest's path relative to the tooling root, so every checkout of the
        // same tooling keys the same. Relative to the root's real path: `require.resolve` answers real paths, so a root under a symlink (macOS
        // `/var` is `/private/var`) would otherwise name a path out of the root.
        return { name, path: relative(realpathSync(root), metadata), sha256: sha256(readFileSync(metadata)) };
      }
    }
    return { name, unresolved: resolved };
  });
}

/** The key: a digest over every input class the browser half reads. */
export function executionCacheKey(parts) {
  return sha256(JSON.stringify({ format: EXECUTION_CACHE_FORMAT, node: process.version, ...parts }));
}

/** An entry's runtime-input pins that no longer match the files on disk. */
export function stalePins(pins) {
  return pins.filter(pin => !existsSync(pin.path) || sha256(readFileSync(pin.path)) !== pin.sha256);
}

/**
 * ADR 1029: a body's absolute paths made portable. `roots` maps a placeholder to this run's absolute directory (the project root, the
 * tooling root); every occurrence of a root in the body is stored as its placeholder and put back on read, so a browser half recorded in
 * one checkout answers the same case in another, and its runtime-input pins are re-hashed at this checkout's paths.
 */
export function portableBody(body, roots = {}) {
  let text = JSON.stringify(body);
  for (const [token, root] of Object.entries(roots).sort(([, a], [, b]) => b.length - a.length)) if (root) text = text.split(root).join(token);
  return JSON.parse(text);
}
export function localBody(body, roots = {}) {
  let text = JSON.stringify(body);
  for (const [token, root] of Object.entries(roots)) if (root) text = text.split(token).join(root);
  return JSON.parse(text);
}

export function createExecutionCache(directory) {
  mkdirSync(directory, { recursive: true });
  const file = key => join(directory, `${key.replace(/^sha256:/, "")}.json`);
  return {
    directory,
    /** The stored body at this run's paths (`roots`), or null on a miss (absent, other format, a stale pin). */
    read(key, roots = {}) {
      const path = file(key);
      if (!existsSync(path)) return null;
      let entry;
      try { entry = JSON.parse(readFileSync(path, "utf8")); } catch { return null; }
      if (entry.format !== EXECUTION_CACHE_FORMAT || entry.key !== key) return null;
      const body = localBody(entry.body, roots);
      if (!Array.isArray(body?.runtimeInputs) || stalePins(body.runtimeInputs).length) return null;
      return body;
    },
    /** Stores a body whole or not at all, its paths under `roots` made portable. */
    write(key, body, roots = {}) {
      const path = file(key), temporary = `${path}.${process.pid}.tmp`;
      writeFileSync(temporary, JSON.stringify({ format: EXECUTION_CACHE_FORMAT, key, body: portableBody(body, roots) }));
      renameSync(temporary, path);
    },
  };
}
