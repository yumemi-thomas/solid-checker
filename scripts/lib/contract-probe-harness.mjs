// Shared machinery for the per-dialect contract probe workers.
//
// A worker is copied into the install directory of the dialect it probes and
// run there, so its bare `import "solid-js"` resolves to that dialect's exact
// audited release. Everything that is not dialect-specific lives here: reading
// the installed export surface, recording probe rows, and emitting the report
// `scripts/check-bundled-contracts.mjs` reads back.
//
// What stays in each worker is the part that cannot be shared: the Solid API
// used to drive a probe (1.x has no `flush`) and the probe bodies themselves.
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

function entrypointLeaves(target, conditions = []) {
  if (typeof target === "string") {
    return /\.m?js$/.test(target) ? [{ target, conditions }] : [];
  }
  if (!target || typeof target !== "object") return [];
  return Object.entries(target).flatMap(([condition, value]) =>
    entrypointLeaves(value, condition === "default" ? conditions : [...conditions, condition]),
  );
}

/**
 * The subpath map of a package's `exports` field.
 *
 * Node allows the sugar forms `"exports": "./index.js"` and
 * `"exports": { "import": ... }`, where the whole field describes `"."` and its
 * keys are conditions rather than subpaths. Reading those keys as entrypoints
 * invents an entrypoint named `import` and loses `.`, so the sugar is expanded
 * here before anything enumerates it.
 */
export function exportSubpaths(exports) {
  if (typeof exports === "string") return { ".": exports };
  if (!exports || typeof exports !== "object") return {};
  const keys = Object.keys(exports);
  if (keys.length > 0 && keys.every(key => !key.startsWith("."))) return { ".": exports };
  return exports;
}

/** Reads each requested package's runtime export surface under this mode. */
export async function describePackages(request) {
  const packages = {};
  for (const item of request.packages) {
    const manifest = JSON.parse(readFileSync(join(item.directory, "package.json"), "utf8"));
    const entrypoints = {};
    for (const [entrypoint, target] of Object.entries(exportSubpaths(manifest.exports))) {
      if (entrypoint.includes("*")) continue;
      const kinds = {};
      const conditions = new Set();
      for (const leaf of entrypointLeaves(target)) {
        const path = join(item.directory, leaf.target);
        if (!existsSync(path)) continue;
        leaf.conditions.forEach(condition => conditions.add(condition));
        const module = await import(pathToFileURL(path));
        for (const [name, value] of Object.entries(module)) {
          if (name === "default") continue;
          const kind = typeof value === "function" ? "function" : "value";
          if (kinds[name] && kinds[name] !== kind) {
            throw new Error(
              `${item.name}${entrypoint} exports ${name} with inconsistent runtime kinds`,
            );
          }
          kinds[name] = kind;
        }
      }
      if (Object.keys(kinds).length > 0) {
        entrypoints[entrypoint] = { exports: kinds, conditions: [...conditions].sort() };
      }
    }
    packages[item.name] = { version: manifest.version, entrypoints };
  }
  return packages;
}

/**
 * Creates the probe recorder for one worker run.
 *
 * `runInRoot` is the dialect's way of running a probe body under a disposable
 * reactive owner and settling its effects; a body may be async, so it is
 * awaited before the row is recorded.
 */
export function createRecorder({ mode, runInRoot }) {
  const probes = [];
  async function probe(pkg, entrypoint, name, claim, body, calls = 1) {
    let ok = false;
    let error;
    try {
      ok = Boolean(await runInRoot(body));
    } catch (caught) {
      error = String(caught);
    }
    probes.push({ pkg, entrypoint, name, claim, mode, calls, ok, ...(error ? { error } : {}) });
  }
  return { probes, probe };
}

/** Emits the report `check-bundled-contracts.mjs` parses from stdout. */
export function emit(packages, probes) {
  const discoveredClaims = probes
    .filter(probe => probe.ok)
    .map(({ pkg, entrypoint, name, claim, mode, calls }) => ({
      pkg,
      entrypoint,
      name,
      claim,
      mode,
      calls,
    }));
  process.stdout.write(JSON.stringify({ packages, probes, discoveredClaims }));
}
