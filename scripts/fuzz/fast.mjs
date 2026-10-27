// The fast runtime path's driver: copies generated cases into a work directory inside
// the pinned install, writes one Vitest test per case (one isolated Happy DOM per case),
// runs Vitest once for the batch, and returns each case's record.
//
// The work directory sits inside the install because the Solid plugin, the dev runtime
// and the diagnostics harness resolve from its node_modules. The install is the one
// scripts/fuzz/install/package.json describes (the pinned 2.0.0-rc.17 Solid packages,
// the same pins and overrides as scripts/runtime-oracle-install, plus vitest, happy-dom
// and @solidjs/diagnostics); `--install` takes another directory that has the same.
import { spawn } from "node:child_process";
import { cpSync, existsSync, mkdirSync, readFileSync, readdirSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
export const DEFAULT_FAST_INSTALL = join(here, "install");
/** Invariant 7: the Solid prerelease under test. Never a newer one. */
export const PINNED_SOLID = "2.0.0-rc.17";

/** Problems with a name -> version table of the Solid packages (empty when the install is the pinned one). */
export function pinnedProblems(versions, pinned = PINNED_SOLID) {
  const problems = [];
  for (const name of ["solid-js", "@solidjs/web", "@solidjs/signals", "@solidjs/diagnostics", "@solidjs/compiler", "@solidjs/babel-plugin"]) {
    if (versions[name] === undefined) problems.push(`${name} is not installed`);
    else if (versions[name] !== pinned) problems.push(`${name} is ${versions[name]}, not ${pinned}`);
  }
  return problems;
}

export function installedVersions(install) {
  const root = resolve(install);
  const version = (name) => {
    try { return JSON.parse(readFileSync(realpathSync(join(root, "node_modules", name, "package.json")), "utf8")).version; } catch { return undefined; }
  };
  const versions = Object.fromEntries(["solid-js", "@solidjs/web", "@solidjs/signals", "@solidjs/diagnostics"].map((name) => [name, version(name)]));
  // The compiler and babel plugin the Solid Vite plugin loads, resolved from the plugin as Node does.
  try {
    const from = createRequire(join(realpathSync(join(root, "node_modules/@solidjs/vite-plugin")), "package.json"));
    for (const name of ["@solidjs/compiler", "@solidjs/babel-plugin"]) {
      let directory = dirname(from.resolve(name));
      while (!existsSync(join(directory, "package.json")) || JSON.parse(readFileSync(join(directory, "package.json"), "utf8")).name !== name) directory = dirname(directory);
      versions[name] = JSON.parse(readFileSync(join(directory, "package.json"), "utf8")).version;
    }
  } catch { /* reported as not installed */ }
  return versions;
}

const testSource = (id, caseDir) => `import { test } from 'vitest';
import { runCase } from '../runner';
test('${id}', () => runCase('${id}', ${JSON.stringify(caseDir)}, () => import('../cases/${id}/App.tsx')), 60000);
`;

/**
 * Runs the cases under Happy DOM and returns `Map<id, record>`.
 * @param {{ id: string, dir: string }[]} cases each `dir` holds App.tsx and sites.json
 */
export async function runFast(cases, { install = DEFAULT_FAST_INSTALL, workers = 4, keep = false, timeoutMs = 600000, pauseMs, finalMs } = {}) {
  for (const { id } of cases) if (!/^[A-Za-z0-9_-]+$/.test(id)) throw new Error(`unsafe case id ${id}`);
  const root = resolve(install);
  const problems = pinnedProblems(installedVersions(root));
  if (problems.length) throw new Error(`the fast path needs the pinned install (${root}): ${problems.join("; ")}`);
  const vitest = join(root, "node_modules/vitest/vitest.mjs");
  if (!existsSync(vitest)) throw new Error(`no vitest in ${root}/node_modules (npm install in scripts/fuzz/install)`);
  const work = join(root, ".solid-checker-fuzz-fast", `${Date.now()}-${process.pid}`);
  mkdirSync(join(work, "tests"), { recursive: true });
  const fast = join(here, "fast");
  for (const file of ["runner.tsx", "shims.ts", "frames.mjs", "vitest.config.mjs"]) cpSync(join(fast, file), join(work, file));
  for (const { id, dir } of cases) {
    const target = join(work, "cases", id);
    mkdirSync(target, { recursive: true });
    for (const file of ["App.tsx", "sites.json", "oracle.json"]) if (existsSync(join(dir, file))) cpSync(join(dir, file), join(target, file));
    writeFileSync(join(work, "tests", `${id}.test.ts`), testSource(id, target));
  }
  const out = join(work, "out");
  const run = await new Promise((done) => {
    const child = spawn(process.execPath, [vitest, "run", "--config", "vitest.config.mjs"], {
      cwd: work, stdio: ["ignore", "pipe", "pipe"],
      env: {
        ...process.env, NODE_ENV: "development", FUZZ_FAST_OUT: out, FUZZ_FAST_WORKERS: String(workers),
        ...(pauseMs === undefined ? {} : { FUZZ_PAUSE_MS: String(pauseMs) }), ...(finalMs === undefined ? {} : { FUZZ_FINAL_MS: String(finalMs) }),
      },
    });
    let log = "";
    const timer = setTimeout(() => child.kill("SIGKILL"), timeoutMs);
    child.stdout.on("data", (chunk) => { log += chunk; });
    child.stderr.on("data", (chunk) => { log += chunk; });
    child.on("close", (status) => { clearTimeout(timer); done({ status, log }); });
  });
  const records = new Map();
  const written = existsSync(out) ? new Set(readdirSync(out)) : new Set();
  for (const { id } of cases) {
    if (written.has(`${id}.json`)) records.set(id, JSON.parse(readFileSync(join(out, `${id}.json`), "utf8")));
    else records.set(id, { id, failure: { phase: "vitest", message: `no record written (vitest exit ${run.status})`, tail: run.log.slice(-1200) }, diagnostics: [], errors: [], witnesses: [] });
  }
  if (!keep) rmSync(work, { recursive: true, force: true });
  return records;
}
