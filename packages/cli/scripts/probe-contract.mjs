// `solid-checker contract probe <CONTRACT>`: RFC 0002 Stage 1.
//
// It executes the drivable claims of a generated contract against the package
// the project actually installed, records `probed` row evidence for the ones
// that passed, and writes `<contract>.probe.json` -- the audit trail of what
// the machine believed, what it observed, and what it could not reach.
//
// It changes no evidence kind. A contract probed here is still `inferred` and
// still certifies nothing; mechanical promotion to `verified` is Stage 2.
//
// **This command executes package code.** That is why it is a command and not a
// flag on `contract generate`, whose stated design property is that package code
// is never imported or executed. The trust involved is the same trust as running
// one's own dependencies, but it has to be taken rather than inherited.
//
// Where it sits in the pipeline:
//
//     contract generate  ->  contract probe [--write]  ->  contract review
//
// `--write` moves the contract's bytes, and a review plan is bound to the exact
// bytes it was written beside. So probing belongs *before* any review decision:
// the write re-binds the untouched plan to the new bytes, and refuses outright
// once a review has recorded anything. See `rebindReviewPlan`.

import { randomUUID } from "node:crypto";
import { spawn } from "node:child_process";
import {
  copyFileSync,
  existsSync,
  mkdtempSync,
  readFileSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync
} from "node:fs";
import { availableParallelism, tmpdir } from "node:os";
import { dirname, join, resolve, sep } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

import { runNative } from "../bin/launcher.mjs";
import { expandContract, normalizeContract } from "./contract-document.mjs";
import {
  collectReviewItems,
  probeReportPath as probeReportSiblingPath,
  reviewPlanJsonPath,
  reviewStatePath
} from "./contract-review-plan.mjs";
import {
  isArgumentRecipe,
  PROBE_MODES,
  applyProbeEvidence,
  buildProbePlan,
  buildProbeReport,
  interpretSession,
  settleClaims,
  sha256Bytes
} from "./contract-probe-driver.mjs";
import { packageIntegrity } from "../../../scripts/lib/package-integrity.mjs";

const DEFAULT_TIMEOUT_MS = 60_000;

export const contractProbeHelp = `Usage:
  solid-checker contract probe <CONTRACT> [OPTIONS]

contract probe executes a generated contract's drivable claims against the
package the project has installed and writes <contract>.probe.json: which
claims were driven, which passed in which condition modes, and the exact reason
every undrivable claim could not be reached.

*** It imports and runs the package's code, and its dependencies', in a child
process. Run it where you would run that package's own test suite: a sandbox,
no ambient credentials, no network egress. It is never part of contract
generate, which imports nothing. ***

Probing comes between generate and review. It confirms claims that already
exist and never writes a new one: a behavior observed that the contract does
not state is an incompleteness finding, which fails the run.

Options:
  --package-root <DIR>   The installed package to probe (default: resolved from
                         the contract's package name)
  --modes <LIST>         Condition modes to attempt, from client, server,
                         development, production (default: all four, narrowed
                         per entrypoint by its recorded conditions)
  --timeout <MS>         Per-mode child process timeout (default: ${DEFAULT_TIMEOUT_MS})
  --no-discovery         Skip the probes that plant a callback where the
                         contract states none. Those are the only automated
                         check that can contradict a negative claim, so a
                         report produced with this flag is for investigation
                         only: contract verify refuses it outright, and the
                         probe report records the refusal reason under
                         "discovery".
  --no-environment-shim  Do not define the minimal browser globals in the
                         client, development and production sessions. Without
                         the shim an entrypoint that reads window at import
                         time throws and every one of its claims is undriven;
                         with it, the claims that are then observed are
                         observed against a fake DOM. Either way the report's
                         "environment" block says which names were faked.
  --report <FILE>        Report output path (default: <contract>.probe.json)
  --write                Record passing modes as probed row evidence on claims
                         that already exist. Refused when any probe failed or
                         any incompleteness was reported, and refused once a
                         review of this contract has recorded anything.
  -h, --help             Show this help
`;

function usage(message) {
  return new Error(`${message}\n\n${contractProbeHelp}`);
}

export function parseProbeArguments(arguments_) {
  const options = {
    contract: undefined,
    packageRoot: undefined,
    modes: undefined,
    timeout: DEFAULT_TIMEOUT_MS,
    discovery: true,
    environmentShim: true,
    report: undefined,
    write: false
  };
  const value = (flag, next) => {
    if (next === undefined || next.length === 0) throw usage(`${flag} requires a value`);
    return next;
  };
  for (let index = 0; index < arguments_.length; index += 1) {
    const argument = arguments_[index];
    if (argument === "--package-root") options.packageRoot = value(argument, arguments_[++index]);
    else if (argument === "--report") options.report = value(argument, arguments_[++index]);
    else if (argument === "--write") options.write = true;
    else if (argument === "--no-discovery") options.discovery = false;
    else if (argument === "--no-environment-shim") options.environmentShim = false;
    else if (argument === "--modes") {
      const list = value(argument, arguments_[++index])
        .split(",")
        .map(name => name.trim())
        .filter(Boolean);
      const unknown = list.filter(name => !PROBE_MODES.some(mode => mode.name === name));
      if (unknown.length) {
        throw usage(
          `--modes names ${unknown.join(", ")}; use ${PROBE_MODES.map(mode => mode.name).join(", ")}`
        );
      }
      if (!list.length) throw usage("--modes requires at least one mode");
      options.modes = PROBE_MODES.filter(mode => list.includes(mode.name));
    } else if (argument === "--timeout") {
      const raw = Number(value(argument, arguments_[++index]));
      if (!Number.isInteger(raw) || raw <= 0) throw usage("--timeout requires a positive integer");
      options.timeout = raw;
    } else if (argument.startsWith("-")) throw usage(`unknown argument ${argument}`);
    else if (options.contract) throw usage(`unexpected argument ${argument}`);
    else options.contract = argument;
  }
  if (!options.contract) throw usage("contract probe requires a contract path");
  return options;
}

function contractPath(argument) {
  const target = resolve(argument);
  return existsSync(target) && statSync(target).isDirectory()
    ? join(target, "solid-reactivity.json")
    : target;
}

export function probeConstructionPlanPath(contractFile) {
  return contractFile.toLowerCase().endsWith(".json")
    ? `${contractFile.slice(0, -5)}.probe-plan.json`
    : `${contractFile}.probe-plan.json`;
}

export function readProbeConstructionPlan(contractFile, contractHash, package_) {
  const path = probeConstructionPlanPath(contractFile);
  if (!existsSync(path)) return undefined;
  const plan = JSON.parse(readFileSync(path, "utf8"));
  if (![1, 2].includes(plan.schemaVersion) || plan.source !== "typescript-value-domain") {
    throw new Error(`${path} is not a supported TypeFacts probe construction plan`);
  }
  if (plan.contract !== contractHash) {
    throw new Error(
      `${path} is bound to ${plan.contract ?? "no contract"}, but ${contractFile} hashes to ${contractHash}; regenerate before probing`
    );
  }
  if (plan.package?.name !== package_?.name || plan.package?.version !== package_?.version) {
    throw new Error(`${path} describes a different package artifact than ${contractFile}`);
  }
  for (const entry of Object.values(plan.entrypoints ?? {})) {
    for (const recipes of Object.values(entry ?? {})) {
      for (const rawCandidates of Object.values(recipes ?? {})) {
        const candidates = Array.isArray(rawCandidates) ? rawCandidates : [rawCandidates];
        if (
          candidates.length === 0 ||
          candidates.length > 8 ||
          candidates.some(recipe => !isArgumentRecipe(recipe))
        ) {
          throw new Error(
            `${path} contains unsupported argument recipe ${JSON.stringify(rawCandidates)}`
          );
        }
      }
    }
  }
  return plan;
}

function readManifest(directory) {
  const path = join(directory, "package.json");
  return existsSync(path) ? JSON.parse(readFileSync(path, "utf8")) : undefined;
}

/// The installed copy of the package the contract describes.
///
/// A contract emitted into the package sits beside its own manifest; a
/// project-owned one under `.solid-checker/contracts/<package>/` does not, so
/// the search walks up looking for the `node_modules/<name>` the project would
/// resolve. Resolution is by exact package name, never by directory name.
export function resolvePackageRoot({ explicit, contractDirectory, packageName }) {
  if (explicit) {
    const root = resolve(explicit);
    const manifest = readManifest(root);
    if (!manifest) throw new Error(`no package.json at ${root}`);
    if (manifest.name !== packageName) {
      throw new Error(
        `--package-root ${root} is ${manifest.name}, and the contract describes ${packageName}`
      );
    }
    return root;
  }
  if (readManifest(contractDirectory)?.name === packageName) return contractDirectory;
  let directory = contractDirectory;
  for (;;) {
    const candidate = join(directory, "node_modules", ...packageName.split("/"));
    if (readManifest(candidate)?.name === packageName) return candidate;
    const parent = dirname(directory);
    if (parent === directory) break;
    directory = parent;
  }
  throw new Error(
    `cannot find an installed ${packageName} above ${contractDirectory}; pass --package-root`
  );
}

/// The Solid release the probed code would import, and therefore which runtime
/// the driver has to settle.
///
/// This mirrors the checker's own dialect detection: the nearest
/// `node_modules/solid-js/package.json` above the package under probe. Unlike
/// the analyzer, probing has no safe default -- settling a 1.x runtime with 2.0
/// semantics observes the wrong thing -- so an unreadable or unclassifiable
/// version is a refusal, never a fallback.
export function resolveProbeRuntime(packageRoot) {
  let directory = packageRoot;
  for (;;) {
    const manifest = readManifest(join(directory, "node_modules", "solid-js"));
    if (manifest?.version) {
      const major = Number(String(manifest.version).split(".")[0]);
      const dialect = major === 1 ? "solid-v1" : major === 2 ? "solid-v2" : undefined;
      if (!dialect) {
        throw new Error(
          `the solid-js installed at ${directory} is ${manifest.version}, which names no dialect this checker probes`
        );
      }
      return { dialect, projectRoot: directory, version: manifest.version };
    }
    const parent = dirname(directory);
    if (parent === directory) break;
    directory = parent;
  }
  throw new Error(
    `no installed solid-js above ${packageRoot}; probing needs the project's own Solid release to settle a probe`
  );
}

/// The package manager lockfile records the integrity of what is actually on
/// disk. The package may be nested under node_modules or passed directly with
/// --package-root, so first locate the project directory that owns the lock.
function installedIntegrity(packageRoot, packageName) {
  const marker = `${sep}node_modules${sep}`;
  const index = packageRoot.lastIndexOf(marker);
  const projectDirectory = index < 0 ? dirname(packageRoot) : packageRoot.slice(0, index);
  return packageIntegrity(projectDirectory, packageName) ?? undefined;
}

/// Runs one condition mode in a child process.
///
/// The worker is staged inside the project's `node_modules` so its bare imports
/// resolve to the project's installs, and the child's working directory is a
/// scratch temporary directory so a package that writes on import writes there.
function spawnSessionAsync({ probes, session, worker, dialect, timeout }) {
  const processWallStarted = process.hrtime.bigint();
  const request = JSON.stringify({ mode: session.mode, dialect, environment: session.environment, probes });
  const scratch = mkdtempSync(join(tmpdir(), "solid-checker-probe-cwd-"));
  return new Promise(resolvePromise => {
    const resolve = answer =>
      resolvePromise({
        ...answer,
        timing: {
          ...(answer.timing ?? {}),
          processWallNs: Number(process.hrtime.bigint() - processWallStarted)
        }
      });
    const child = spawn(
      process.execPath,
      [...session.conditions.flatMap(condition => ["--conditions", condition]), worker, "-"],
      { cwd: scratch, stdio: ["pipe", "pipe", "pipe"] }
    );
    child.stdin.end(request);
    let stdout = "";
    let stderr = "";
    let spawnError;
    let timedOut = false;
    let forcedKill;
    child.stdout.on("data", chunk => {
      stdout += chunk;
    });
    child.stderr.on("data", chunk => {
      stderr += chunk;
    });
    child.on("error", error => {
      spawnError = error;
    });
    const timer = setTimeout(() => {
      timedOut = true;
      child.kill("SIGTERM");
      forcedKill = setTimeout(() => child.kill("SIGKILL"), 1_000);
    }, timeout);
    child.on("close", (status, signal) => {
      clearTimeout(timer);
      clearTimeout(forcedKill);
      rmSync(scratch, { recursive: true, force: true });
      if (spawnError || status !== 0 || !stdout) {
        const detail =
          spawnError?.message ??
          (signal
            ? `the probe process was killed by ${signal}${timedOut ? ` (timeout ${timeout}ms)` : ""}`
            : `the probe process exited ${status}`);
        resolve({ failed: `${detail}${stderr ? `: ${stderr.trim()}` : ""}`, results: [] });
        return;
      }
      try {
        const answer = JSON.parse(stdout);
        resolve({
          completed: answer.completed !== false,
          environment: answer.environment,
          runtime: answer.runtime ?? null,
          timing: answer.timing,
          aborted: answer.aborted,
          results: answer.results ?? []
        });
      } catch (error) {
        resolve({
          failed: `the probe process wrote no readable report: ${error.message}`,
          results: []
        });
      }
    });
  });
}

/// Runs one mode to completion, restarting the worker after every probe that
/// threw.
///
/// A worker stops at its first throw because Solid 2.0's development build
/// halts the reactive system permanently on an uncaught error, and every later
/// observation in that process would be of a runtime where nothing re-runs --
/// a tracked callback reading as inline, reported as a conformance failure the
/// package does not deserve. Each restart answers at least the probe that
/// stopped the previous one, so the loop is bounded by the probe count.
///
/// A session-level failure -- a crash, a timeout, unreadable output -- is
/// different: nothing says which probe caused it, so the remaining probes are
/// recorded undriven rather than retried into a loop of timeouts.
/// The accounting is returned rather than logged. A mode that answered its
/// probes in one process and a mode that needed forty restarts cost wildly
/// different wall time and mean different things about the package, and until
/// this the report recorded neither -- the restart count was visible only as
/// an unexplained probe duration.
export function runSessionWithRestarts({ session, spawn }) {
  const results = [];
  const accounting = {
    mode: session.mode,
    started: 0,
    restarts: 0,
    failed: 0,
    completed: false,
    // The first process that measured the runtime settles the mode's record. A
    // restart re-imports the same artifacts, so its answer is the same one; a
    // process that died before importing `solid-js` measured nothing and leaves
    // this `null` rather than claiming the runtime was inert.
    runtime: null
  };
  let environment;
  let aborted;
  let pending = session.probes;
  const finish = () => ({ results, accounting, environment });
  for (let attempt = 0; pending.length && attempt <= session.probes.length; attempt += 1) {
    const answer = spawn(pending);
    accounting.started += 1;
    if (attempt > 0) accounting.restarts += 1;
    // The first session that answers at all settles the environment record: a
    // restart runs with the same request-level environment, and a session that
    // never produced readable output has none to report.
    environment ??= answer.environment;
    accounting.runtime ??= answer.runtime ?? null;
    if (answer.aborted) aborted = answer.aborted;
    results.push(...answer.results);
    const answered = new Set(answer.results.map(result => result.id));
    const remaining = pending.filter(probe => !answered.has(probe.id));
    if (answer.failed) {
      accounting.failed += 1;
      results.push(...sessionFailure(remaining, answer.failed));
      return finish();
    }
    // A worker deliberately stops after an import or invocation failure to
    // protect later observations from partially initialized runtime state. If
    // that stopping probe was the last one in this isolated specifier batch,
    // every requested claim still received an answer; `completed` describes
    // coverage, not whether the worker chose to keep its process alive.
    if (remaining.length === 0) {
      accounting.completed = true;
      pending = [];
      break;
    }
    if (answer.completed || remaining.length === pending.length) {
      // A process that answered nothing made no progress, so the loop stops
      // here rather than restarting into the same abort. When an asynchronous
      // throw is why, that reason travels to the probes it could not reach --
      // which is more than "stopped before reaching this claim" could say.
      if (remaining.length === pending.length && answer.aborted) accounting.failed += 1;
      pending = remaining;
      accounting.completed = remaining.length === 0;
      break;
    }
    pending = remaining;
  }
  results.push(
    ...sessionFailure(
      pending,
      aborted
        ? `the probe process was aborted by package code running outside a probe: ${aborted}`
        : "the probe process stopped before reaching this claim"
    )
  );
  return finish();
}

/// Runs one mode with a separate worker lifetime for each exact entrypoint
/// specifier.
///
/// Importing an ESM entrypoint necessarily evaluates that entrypoint's whole
/// module, so there is no sound way to recover one named export when that
/// evaluation throws. It is equally unsound to let that throw suppress a
/// *different* entrypoint, though: the package export map already proves the
/// two specifiers are separate runtime loads. Isolating them in separate child
/// processes preserves the real published runtime and the no-shim server
/// environment while containing partial initialization to its exact specifier.
export function runSessionBySpecifier({ session, spawn }) {
  const groups = new Map();
  for (const probe of session.probes) {
    const probes = groups.get(probe.specifier) ?? [];
    probes.push(probe);
    groups.set(probe.specifier, probes);
  }
  const runs = [...groups.values()].map(probes =>
    runSessionWithRestarts({ session: { ...session, probes }, spawn })
  );
  return {
    results: runs.flatMap(run => run.results),
    accounting: {
      mode: session.mode,
      started: runs.reduce((total, run) => total + run.accounting.started, 0),
      restarts: runs.reduce((total, run) => total + run.accounting.restarts, 0),
      failed: runs.reduce((total, run) => total + run.accounting.failed, 0),
      completed: runs.every(run => run.accounting.completed),
      runtime: runs.find(run => run.accounting.runtime)?.accounting.runtime ?? null
    },
    environment: runs.find(run => run.environment)?.environment
  };
}

/// Separates observations that only inspect an imported namespace from those
/// that invoke package code.
///
/// A `kind` probe is safe to share one import lifetime with every other kind
/// probe for the same exact specifier: it performs only `typeof`. Every other
/// probe is call-capable and enters a restart chain that is discarded before
/// any remaining work continues after a throw or asynchronous abort.
export function splitSessionProbes(session) {
  const kindsBySpecifier = new Map();
  const invoking = [];
  for (const probe of session.probes) {
    if (probe.type === "kind") {
      const probes = kindsBySpecifier.get(probe.specifier) ?? [];
      probes.push(probe);
      kindsBySpecifier.set(probe.specifier, probes);
    } else {
      invoking.push({ ...session, probes: [probe] });
    }
  }
  return {
    nonInvoking: [...kindsBySpecifier.values()].map(probes => ({ ...session, probes })),
    invoking
  };
}

export async function mapConcurrentOrdered(values, concurrency, operation) {
  const limit = Math.max(1, Math.min(values.length || 1, Number(concurrency) || 1));
  const results = new Array(values.length);
  let next = 0;
  await Promise.all(
    Array.from({ length: limit }, async () => {
      for (;;) {
        const index = next++;
        if (index >= values.length) return;
        results[index] = await operation(values[index], index);
      }
    })
  );
  return results;
}

function invocationTasks(split, concurrency) {
  const groups = split.flatMap(({ session, invoking }) => {
    const bySpecifier = new Map();
    for (const task of invoking) {
      const probe = task.probes[0];
      const probes = bySpecifier.get(probe.specifier) ?? [];
      probes.push(probe);
      bySpecifier.set(probe.specifier, probes);
    }
    return [...bySpecifier.values()].map(probes => ({ session, probes, lanes: 1 }));
  });
  const target = Math.min(
    Math.max(1, Number(concurrency) || 1),
    groups.reduce((total, group) => total + group.probes.length, 0)
  );
  let lanes = groups.length;
  while (lanes < target) {
    let selected;
    for (const group of groups) {
      if (group.lanes >= group.probes.length) continue;
      if (
        !selected ||
        group.probes.length / group.lanes > selected.probes.length / selected.lanes
      ) {
        selected = group;
      }
    }
    if (!selected) break;
    selected.lanes += 1;
    lanes += 1;
  }
  return groups.flatMap(group => {
    const chains = Array.from({ length: group.lanes }, () => []);
    group.probes.forEach((probe, index) => chains[index % chains.length].push(probe));
    return chains.map(probes => ({ session: group.session, probes }));
  });
}

async function runFreshProbeChain({ probes, spawn }) {
  const results = [];
  let pending = probes;
  let started = 0;
  let failed = 0;
  const restartCauses = {};
  const timing = {};
  const addTiming = source => {
    for (const [phase, duration] of Object.entries(source ?? {})) {
      if (Number.isFinite(duration)) timing[phase] = (timing[phase] ?? 0) + duration;
    }
  };
  const addRestartCause = cause => {
    restartCauses[cause] = (restartCauses[cause] ?? 0) + 1;
  };
  let environment;
  let runtime;
  while (pending.length) {
    const answer = await spawn(pending);
    started += 1;
    addTiming(answer.timing);
    environment ??= answer.environment;
    runtime ??= answer.runtime ?? null;
    const answeredResults = answer.results ?? [];
    results.push(...answeredResults);
    const answered = new Set(answeredResults.map(result => result.id));
    const remaining = pending.filter(probe => !answered.has(probe.id));
    if (answer.failed) {
      failed += 1;
      const kinds = remaining.filter(probe => probe.type === "kind");
      if (kinds.length > 0 && kinds.length < remaining.length) {
        addRestartCause("unreadableResult");
        const recovered = await runFreshProbeChain({ probes: kinds, spawn });
        results.push(...recovered.results);
        started += recovered.started;
        failed += recovered.failed;
        addTiming(recovered.timing);
        for (const [cause, count] of Object.entries(recovered.restartCauses)) {
          restartCauses[cause] = (restartCauses[cause] ?? 0) + count;
        }
        environment ??= recovered.environment;
        runtime ??= recovered.runtime ?? null;
        const risky = remaining.filter(probe => probe.type !== "kind");
        results.push(...sessionFailure(risky, answer.failed));
        return {
          results,
          started,
          failed,
          completed: false,
          environment,
          runtime,
          restartCauses,
          timing
        };
      }
      results.push(...sessionFailure(remaining, answer.failed));
      return {
        results,
        started,
        failed,
        completed: false,
        environment,
        runtime,
        restartCauses,
        timing
      };
    }
    if (remaining.length === 0) {
      return {
        results,
        started,
        failed,
        completed: true,
        environment,
        runtime,
        restartCauses,
        timing
      };
    }
    if (answered.size === 0 || answer.completed) {
      if (answer.aborted) failed += 1;
      results.push(
        ...sessionFailure(
          remaining,
          answer.aborted
            ? `the probe process was aborted by package code running outside a probe: ${answer.aborted}`
            : "the probe process stopped before reaching this claim"
        )
      );
      return {
        results,
        started,
        failed,
        completed: false,
        environment,
        runtime,
        restartCauses,
        timing
      };
    }
    // The worker stopped after a package throw or asynchronous abort. Continue
    // only in a newly spawned process; the stopped runtime is never reused.
    addRestartCause(
      answer.aborted
        ? "asynchronousAbort"
        : answeredResults.some(result => result.outcome === "threw")
          ? "synchronousThrow"
          : answeredResults.some(result => result.outcome === "import-failed")
            ? "importFailure"
            : "stopped"
    );
    pending = remaining;
  }
  return {
    results,
    started,
    failed,
    completed: true,
    environment,
    runtime,
    restartCauses,
    timing
  };
}

/// Runs kind-only imports first, then call-capable probes in fresh restart chains.
///
/// The pool bounds process fan-out. A chain may retain a process only while its
/// package calls complete; a stopped process is replaced before any remaining
/// probe runs. Completion order is discarded and results are restored to the
/// plan's stable probe order.
export async function runIsolatedProbePool({ session, spawn, concurrency = 4 }) {
  const [result] = await runIsolatedProbePools({
    sessions: [session],
    concurrency,
    spawn: ({ probes }) => spawn(probes)
  });
  return {
    results: result.results,
    accounting: result.accounting,
    environment: result.environment
  };
}

function summarizeSessionRuns(session, runs) {
  const order = new Map(session.probes.map((probe, index) => [probe.id, index]));
  const results = runs
    .flatMap(result => result.results)
    .sort((left, right) => (order.get(left.id) ?? Infinity) - (order.get(right.id) ?? Infinity));
  const started = runs.reduce((total, result) => total + result.started, 0);
  const restartCauses = {};
  const timing = {};
  for (const run of runs) {
    for (const [cause, count] of Object.entries(run.restartCauses)) {
      restartCauses[cause] = (restartCauses[cause] ?? 0) + count;
    }
    for (const [phase, duration] of Object.entries(run.timing)) {
      timing[phase] = (timing[phase] ?? 0) + duration;
    }
  }
  return {
    mode: session.mode,
    results,
    accounting: {
      mode: session.mode,
      started,
      chains: runs.length,
      restarts: Math.max(0, started - runs.length),
      ...(Object.keys(restartCauses).length ? { restartCauses } : {}),
      ...(Object.keys(timing).length ? { timing } : {}),
      failed: runs.reduce((total, result) => total + result.failed, 0),
      completed: runs.every(result => result.completed),
      runtime: runs.find(result => result.runtime)?.runtime ?? null
    },
    environment: runs.find(result => result.environment)?.environment
  };
}

/// Runs every condition mode through one row-global bounded pool.
///
/// Each task still carries one exact mode and every stopped chain still gets a
/// new process. Sharing the scheduler only prevents a short mode from leaving
/// capacity idle while another mode has restart work remaining.
export async function runIsolatedProbePools({ sessions, spawn, concurrency = 4 }) {
  const runsBySession = new Map(sessions.map(session => [session, []]));
  const split = sessions.map(session => ({ session, ...splitSessionProbes(session) }));
  const invokingTasks = invocationTasks(split, concurrency);
  const firstInvocationBySession = new Map();
  for (const task of invokingTasks) {
    const bySpecifier = firstInvocationBySession.get(task.session) ?? new Map();
    bySpecifier.set(task.probes[0].specifier, bySpecifier.get(task.probes[0].specifier) ?? task);
    firstInvocationBySession.set(task.session, bySpecifier);
  }
  const nonInvokingTasks = [];
  for (const { session, nonInvoking } of split) {
    for (const task of nonInvoking) {
      const invocation = firstInvocationBySession.get(session)?.get(task.probes[0].specifier);
      if (invocation) invocation.probes = [...task.probes, ...invocation.probes];
      else nonInvokingTasks.push({ session, probes: task.probes });
    }
  }
  const runTask = async task => ({
    session: task.session,
    run: await runFreshProbeChain({
      probes: task.probes,
      spawn: probes => spawn({ session: task.session, probes })
    })
  });
  const nonInvoking = await mapConcurrentOrdered(nonInvokingTasks, concurrency, runTask);
  const invoking = await mapConcurrentOrdered(invokingTasks, concurrency, runTask);
  for (const completed of [...nonInvoking, ...invoking]) {
    runsBySession.get(completed.session).push(completed.run);
  }
  return sessions.map(session => summarizeSessionRuns(session, runsBySession.get(session)));
}

function sessionFailure(probes, error) {
  return probes.map(probe => ({
    id: probe.id,
    specifier: probe.specifier,
    export: probe.export,
    outcome: "session-failed",
    error
  }));
}

/// Re-binds an untouched review plan to the bytes an evidence write produced.
///
/// A plan is a set of questions about one exact document, and `contract review`
/// refuses a plan whose hash does not match the contract beside it. An evidence
/// write moves those bytes, so without this the pipeline would tell a user to
/// regenerate a contract that is fresher than the plan, not staler.
///
/// Re-binding is only sound while the review has answered nothing, which is why
/// the check is on the review *state* and not on the plan: a recorded decision
/// is bound to the bytes it was recorded against, and silently moving the plan
/// under it would re-bless answers given to a different document. The item set
/// is compared as well -- probed evidence provably raises no new question, and
/// if it ever did, that question must be reviewed rather than re-bound past.
export function rebindReviewPlan({ contractFile, previousHash, nextHash, nextContract, apply }) {
  const planFile = reviewPlanJsonPath(contractFile);
  if (!existsSync(planFile)) return undefined;
  const plan = JSON.parse(readFileSync(planFile, "utf8"));
  if (plan.contract !== previousHash) {
    throw new Error(
      `the review plan at ${planFile} was written for contract bytes ${plan.contract} and ` +
        `${contractFile} hashes to ${previousHash}; regenerate the contract before probing it`
    );
  }
  const statePath = reviewStatePath(contractFile);
  if (existsSync(statePath)) {
    const state = JSON.parse(readFileSync(statePath, "utf8"));
    const answered = Object.keys(state.resolutions ?? {}).length;
    if (answered > 0 || state.promoted) {
      throw new Error(
        `${statePath} already records ${state.promoted ? "a promotion" : `${answered} review decision(s)`} for ` +
          `${contractFile}; probe evidence would move the bytes those decisions were recorded against. ` +
          "Probe before reviewing, or regenerate the contract and probe the fresh document"
      );
    }
  }
  const before = new Set(plan.items.map(item => item.id));
  const after = collectReviewItems(nextContract.entrypoints).map(item => item.id);
  const introduced = after.filter(id => !before.has(id));
  if (introduced.length) {
    throw new Error(
      `writing probe evidence into ${contractFile} would raise review question(s) ${introduced.join(", ")} ` +
        "that the plan does not list; regenerate the contract so they are reviewed rather than re-bound past"
    );
  }
  if (apply) {
    writeFileSync(planFile, `${JSON.stringify({ ...plan, contract: nextHash }, null, 2)}\n`);
  }
  return planFile;
}

/// Writes the contract only after the document that will be written validates,
/// exactly as `--promote reviewed` does, so a rejected write never leaves a
/// contract on disk that the loader refuses.
function writeContract(contractFile, document) {
  const candidate = `${contractFile}.tmp-${randomUUID()}`;
  writeFileSync(candidate, document);
  try {
    const validation = runNative("solid-checker", ["--validate-contract", candidate], {
      encoding: "utf8",
      stdio: "pipe"
    });
    if (validation.error) throw validation.error;
    if (validation.status !== 0) {
      throw new Error(
        `the probed document for ${contractFile} does not validate, so the contract is unchanged: ${
          [validation.stderr, validation.stdout].filter(Boolean).join("\n").trim() ||
          `native solid-checker exited ${validation.status}`
        }`
      );
    }
    renameSync(candidate, contractFile);
  } finally {
    rmSync(candidate, { force: true });
  }
}

function probeDriverIdentity() {
  const manifest = JSON.parse(
    readFileSync(fileURLToPath(new URL("../package.json", import.meta.url)), "utf8")
  );
  return `${manifest.name}@${manifest.version}`;
}

/// The default session runner. Tests inject their own to exercise every
/// judgement of the driver without an install.
function probeConcurrency(value = process.env.SOLID_CHECKER_PROBE_CONCURRENCY) {
  const fallback = Math.min(8, availableParallelism());
  const parsed = Number(value ?? fallback);
  return Number.isInteger(parsed) && parsed > 0 ? Math.min(parsed, 8) : fallback;
}

async function defaultRunSessions({ sessions, dialect, projectRoot, timeout }) {
  const stagingDirectory = mkdtempSync(
    join(projectRoot, "node_modules", ".solid-checker-probe-")
  );
  try {
    const worker = join(stagingDirectory, "contract-probe-worker.mjs");
    copyFileSync(fileURLToPath(new URL("./contract-probe-worker.mjs", import.meta.url)), worker);
    const runs = await runIsolatedProbePools({
      sessions,
      concurrency: probeConcurrency(),
      spawn: ({ session, probes }) =>
        spawnSessionAsync({ probes, session, worker, dialect, timeout })
    });
    return runs.map((run, index) => {
      const session = sessions[index];
      return {
        mode: run.mode,
        results: run.results,
        accounting: run.accounting,
        // What the session *asked for* is the fallback: a mode whose every
        // process died reports no environment of its own, and saying "no shim"
        // there would be a claim about a process that never answered.
        environment: run.environment ?? session.environment
      };
    });
  } finally {
    rmSync(stagingDirectory, { recursive: true, force: true });
  }
}

export async function probeContract(arguments_, { runSessions = defaultRunSessions } = {}) {
  if (arguments_.includes("--help") || arguments_.includes("-h")) {
    process.stdout.write(contractProbeHelp);
    return;
  }
  const options = parseProbeArguments(arguments_);
  const contractFile = contractPath(options.contract);
  if (!existsSync(contractFile)) throw new Error(`no contract at ${contractFile}`);
  const contractBytes = readFileSync(contractFile);
  const contractHash = sha256Bytes(contractBytes);
  const document = JSON.parse(contractBytes.toString("utf8"));
  const contract = expandContract(document);
  const packageName = contract.package?.name;
  if (!packageName) throw new Error(`${contractFile} names no package`);
  const constructionPlan = readProbeConstructionPlan(
    contractFile,
    contractHash,
    contract.package
  );

  const packageRoot = resolvePackageRoot({
    explicit: options.packageRoot,
    contractDirectory: dirname(contractFile),
    packageName
  });
  const installed = readManifest(packageRoot);
  if (installed.version !== contract.package.version) {
    // Probing a different artifact than the contract describes would record an
    // observation of bytes the contract never claimed to be about.
    throw new Error(
      `${contractFile} describes ${packageName}@${contract.package.version} and ${packageRoot} is ` +
        `${installed.version}; regenerate the contract for the installed release before probing it`
    );
  }
  const runtime = resolveProbeRuntime(packageRoot);

  const plan = buildProbePlan(contract, {
    modes: options.modes,
    discovery: options.discovery,
    environmentShim: options.environmentShim,
    constructionPlan
  });
  const sessions = await runSessions({
    sessions: plan.sessions,
    dialect: runtime.dialect,
    projectRoot: runtime.projectRoot,
    packageRoot,
    timeout: options.timeout
  });

  const incompleteness = [];
  const evidence = [];
  const environment = {};
  const accounting = [];
  for (const session of sessions) {
    const interpreted = interpretSession({
      claims: plan.claims,
      index: plan.index,
      mode: session.mode,
      results: session.results ?? []
    });
    incompleteness.push(...interpreted.incompleteness);
    evidence.push(...interpreted.evidence);
    const planned = plan.sessions.find(candidate => candidate.mode === session.mode);
    environment[session.mode] = session.environment ??
      planned?.environment ?? { kind: "none", shimmed: [], present: [] };
    if (session.accounting) accounting.push(session.accounting);
  }
  const claims = settleClaims(plan.claims);

  const report = buildProbeReport({
    contract,
    contractHash,
    contractPath: contractFile,
    installed: {
      version: installed.version,
      integrity: installedIntegrity(packageRoot, packageName)
    },
    generator: readGeneratorIdentity(contractFile),
    probeDriver: probeDriverIdentity(),
    dialect: runtime.dialect,
    runtime: { package: "solid-js", version: runtime.version },
    modes: options.modes ?? PROBE_MODES,
    discovery: plan.discovery,
    environment,
    sessions: accounting,
    claims,
    incompleteness
  });

  const failed = claims.filter(claim => claim.status === "failed");
  const passed = claims.filter(claim => claim.status === "passed");
  const undriven = claims.filter(claim => claim.status === "undriven");

  for (const claim of passed) {
    process.stdout.write(
      `ok   ${claim.entrypoint}:${claim.export} ${claim.claim} ` +
        `(${[...new Set(claim.modesPassed)].sort().join(", ")}` +
        // A `kind` observation reads `typeof` and invokes nothing, so it
        // reports no call count rather than a fabricated one.
        `${claim.calls ? `, ${claim.calls} calls` : ""})\n`
    );
  }
  for (const claim of failed) {
    process.stderr.write(
      `FAIL ${claim.entrypoint}:${claim.export} ${claim.claim}: ${claim.reason}\n`
    );
  }
  for (const finding of incompleteness) {
    process.stderr.write(`INCOMPLETENESS ${finding.text}\n`);
  }
  const reasons = new Map();
  for (const claim of undriven) {
    reasons.set(claim.reason, (reasons.get(claim.reason) ?? 0) + 1);
  }
  for (const [reason, count] of [...reasons].sort((left, right) => right[1] - left[1])) {
    process.stdout.write(`undriven ${count}: ${reason}\n`);
  }

  let wrote;
  if (options.write) {
    if (failed.length || incompleteness.length) {
      process.stderr.write(
        "solid-checker: not written: a probe failed or reported incompleteness, so the contract is unchanged\n"
      );
    } else {
      const applied = applyProbeEvidence(contract, evidence, claims);
      const next = `${JSON.stringify(normalizeContract(applied.contract), null, 2)}\n`;
      const nextHash = sha256Bytes(Buffer.from(next, "utf8"));
      // Every refusal is raised before anything is written.
      const planFile = rebindReviewPlan({
        contractFile,
        previousHash: contractHash,
        nextHash,
        nextContract: expandContract(JSON.parse(next)),
        apply: false
      });
      writeContract(contractFile, next);
      if (planFile) {
        rebindReviewPlan({
          contractFile,
          previousHash: contractHash,
          nextHash,
          nextContract: expandContract(JSON.parse(next)),
          apply: true
        });
      }
      wrote = { markers: applied.written, superseded: applied.superseded, planFile };
      // The report's `contract.hash` is the document that was *probed*. Once
      // evidence lands the bytes move, so the report names both rather than
      // letting a reader assume the hash still matches the file beside it.
      report.contract.afterWrite = nextHash;
      report.contract.markersWritten = applied.written;
      report.contract.markersSuperseded = applied.superseded.length;
      // A supersession is a deletion, and a deletion nobody can see is how a
      // stale observation survived in the first place. Each entry names the
      // claim whose re-drive did not pass and the marker that claim used to
      // carry, so a reader can tell "never observed" from "observed once, and
      // this run says otherwise".
      if (applied.superseded.length) report.superseded = applied.superseded;
    }
  }

  const reportPath = options.report ? resolve(options.report) : probeReportPath(contractFile);
  writeFileSync(reportPath, `${JSON.stringify(report, null, 2)}\n`);

  process.stdout.write(
    `${claims.length} claim(s) for ${packageName}@${installed.version} on ${runtime.dialect} ` +
      `(solid-js ${runtime.version}): ${passed.length} passed, ${failed.length} failed, ` +
      `${undriven.length} undriven, ${incompleteness.length} incompleteness; report ${reportPath}\n`
  );
  if (wrote) {
    for (const marker of wrote.superseded) {
      process.stdout.write(
        `superseded ${marker.entrypoint}:${marker.export} ${marker.field} probed evidence ` +
          `(${(marker.previous.modes ?? []).join(", ")}): this run drove ${marker.claim} and it did ` +
          "not pass\n"
      );
    }
    process.stdout.write(
      `wrote ${wrote.markers} probed row marker(s) into ${contractFile}` +
        `${wrote.superseded.length ? `; superseded ${wrote.superseded.length} stale marker(s)` : ""}` +
        `${wrote.planFile ? `; re-bound the review plan at ${wrote.planFile}` : ""}\n`
    );
  }
  if (failed.length || incompleteness.length) process.exitCode = 1;
  return report;
}

export function probeReportPath(contractFile) {
  return probeReportSiblingPath(contractFile);
}

/// The generator identity the review plan already records, so the report names
/// every identity its result is a function of. A hand-authored contract has no
/// plan and therefore no generator; that is recorded as null rather than
/// guessed.
function readGeneratorIdentity(contractFile) {
  const planFile = reviewPlanJsonPath(contractFile);
  if (!existsSync(planFile)) return null;
  try {
    return JSON.parse(readFileSync(planFile, "utf8")).generation?.generator ?? null;
  } catch {
    return null;
  }
}
