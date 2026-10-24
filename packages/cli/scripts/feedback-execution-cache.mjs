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
import { existsSync, mkdirSync, readFileSync, readdirSync, renameSync, statSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

/** Bumped when what an entry means changes; older entries become misses. */
export const EXECUTION_CACHE_FORMAT = 1;

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
        return { name, path: metadata, sha256: sha256(readFileSync(metadata)) };
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

export function createExecutionCache(directory) {
  mkdirSync(directory, { recursive: true });
  const file = key => join(directory, `${key.replace(/^sha256:/, "")}.json`);
  return {
    directory,
    /** The stored body, or null on a miss (absent, other format, a stale pin). */
    read(key) {
      const path = file(key);
      if (!existsSync(path)) return null;
      let entry;
      try { entry = JSON.parse(readFileSync(path, "utf8")); } catch { return null; }
      if (entry.format !== EXECUTION_CACHE_FORMAT || entry.key !== key) return null;
      if (!Array.isArray(entry.body?.runtimeInputs) || stalePins(entry.body.runtimeInputs).length) return null;
      return entry.body;
    },
    /** Stores a body whole or not at all. */
    write(key, body) {
      const path = file(key), temporary = `${path}.${process.pid}.tmp`;
      writeFileSync(temporary, JSON.stringify({ format: EXECUTION_CACHE_FORMAT, key, body }));
      renameSync(temporary, path);
    },
  };
}
