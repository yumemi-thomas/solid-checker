#!/usr/bin/env bun
// Offline benchmark runner: installs each manifest probe's exact package and
// Solid runtime versions in an isolated temporary project, runs the real
// `contract generate` CLI against it, classifies the outcome, and reports.
//
// Three things matter more here than convenience:
//
// - `SOLID_CHECKER_NATIVE_BIN` and `SOLID_TYPEFACTS_BIN` are both mandatory
//   and must point at real files. Falling back to the checked-in
//   bin/solid-checker-rust would silently measure a possibly-stale engine
//   (see AGENTS.md's "Stale binaries hide source changes"), and treating a
//   missing Type Facts binary as "skip that check" would silently turn a
//   real semantic gap into a clean run. Both are documented traps this
//   runner exists to never fall into, so both are checked before anything
//   else happens and either missing one is an immediate, explicit exit 2.
// - The generated contract is written OUTSIDE the probe's node_modules tree
//   (a wholly separate temporary directory, not a subdirectory of the
//   install project). This is not cosmetic: some packages' own build or
//   package-manager tooling walks node_modules, and a report artifact living
//   inside it could be mistaken for package content or swept up by a
//   subsequent install in the same probe run.
// - A single probe's install failure, integrity mismatch, timeout, or crash
//   is business data for the report, not a reason to stop the benchmark. The
//   runner's job is to report what happened to every probe, not to judge
//   whether the run "succeeded" — that is `--thresholds` mode's job, and
//   only `--thresholds` mode can turn probe-level content into a non-zero
//   exit. Exit 2 is reserved for the harness itself failing to do its job
//   (bad manifest, missing binaries, a crash in the runner, unwritable
//   reports) — never for what the packages under test happened to do. Even a
//   run where every single probe fails to install is still exit 0: the
//   runner reported faithfully, it did not judge.

import { spawn } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { mkdtemp, rm } from "node:fs/promises";
import { availableParallelism, tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { classifyResult, normalizeSignature } from "./lib/classify.mjs";
import { readContractContent } from "./lib/contract-content.mjs";
import {
  createProject,
  installPackages as bunInstall,
  readInstalledVersions,
  readLockIntegrity,
  verifyInstall
} from "./lib/install.mjs";
import { sortRows, validateManifest } from "./lib/manifest.mjs";
import { buildReport, evaluateThresholds, renderMarkdown } from "./lib/report.mjs";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const DEFAULT_MANIFEST = join(ROOT, "scripts/ecosystem-benchmark/manifest.json");
const DEFAULT_SENTINEL = join(ROOT, "scripts/ecosystem-benchmark/sentinel.json");
const DEFAULT_CLI = join(ROOT, "packages/cli/bin/solid-checker.mjs");
const DEFAULT_TIMEOUT_SECONDS = 300;

// Each probe launches Bun and the native checker, so unconstrained fan-out
// can exhaust memory on large hosts. Eight kept all cores busy on the measured
// corpus without multiplying the checker process tree beyond a safe bound;
// smaller machines follow their actual CPU availability instead of inheriting
// a workstation-specific constant.
export function recommendedConcurrency(parallelism = availableParallelism()) {
  return Number.isInteger(parallelism) && parallelism > 0
    ? Math.min(8, parallelism)
    : 4;
}

const DEFAULT_CONCURRENCY = recommendedConcurrency();

// ---------------------------------------------------------------------------
// Pure helpers (exported for direct unit testing).
// ---------------------------------------------------------------------------

// Verifies the two binaries execution absolutely cannot proceed without.
// Exported standalone (rather than folded into `main`) so a test can prove
// the exact missing path is named without needing real binaries on disk.
export function checkRequiredBinaries(env = process.env) {
  const problems = [];
  const nativeBin = env.SOLID_CHECKER_NATIVE_BIN ?? null;
  const typeFactsBin = env.SOLID_TYPEFACTS_BIN ?? null;

  if (!nativeBin) {
    problems.push("SOLID_CHECKER_NATIVE_BIN is not set");
  } else if (!existsSync(nativeBin)) {
    problems.push(`SOLID_CHECKER_NATIVE_BIN does not exist: ${nativeBin}`);
  }

  if (!typeFactsBin) {
    problems.push("SOLID_TYPEFACTS_BIN is not set");
  } else if (!existsSync(typeFactsBin)) {
    problems.push(`SOLID_TYPEFACTS_BIN does not exist: ${typeFactsBin}`);
  }

  return { ok: problems.length === 0, problems, nativeBin, typeFactsBin };
}

// The runner's only exit-code decision that depends on package-level content
// rather than infrastructure health. Isolated as a pure function so the
// exit-1-only-under-thresholds rule can be tested directly instead of by
// spawning the process and reading its exit code.
export function decideExitCode({ thresholdsRequested, evaluation }) {
  if (!thresholdsRequested) return 0;
  return evaluation?.ok === false ? 1 : 0;
}

// `exports` is the raw value of an installed package's package.json#exports.
// A single string or a conditions object for "." both declare exactly one
// entrypoint; a map of subpaths (each possibly a wildcard pattern) declares
// one per key. Counting a wildcard as one declared pattern (not attempting
// to expand it) matches "Entrypoint counting" in INTERFACES.md.
export function countDeclaredEntrypoints(exportsField) {
  if (exportsField == null) return 0;
  if (typeof exportsField === "string") return 1;
  if (typeof exportsField !== "object") return 0;
  const keys = Object.keys(exportsField);
  if (keys.length === 0) return 0;
  // No key starting with "." means this object is itself the conditions map
  // for the "." entrypoint (e.g. { "import": "...", "require": "..." }).
  if (!keys.some(key => key.startsWith("."))) return 1;
  return keys.length;
}

export function readDeclaredEntrypointCount(packageJsonPath) {
  try {
    const parsed = JSON.parse(readFileSync(packageJsonPath, "utf8"));
    return countDeclaredEntrypoints(parsed.exports);
  } catch {
    // Missing, unreadable, or unparsable: the probe never got far enough for
    // this to be knowable, which is a fact of the failure, not a zero.
    return null;
  }
}

export function readGeneratedEntrypointCount(contractPath) {
  try {
    const parsed = JSON.parse(readFileSync(contractPath, "utf8"));
    return Object.keys(parsed.entrypoints ?? {}).length;
  } catch {
    return null;
  }
}

function packageInstallPath(projectDir, packageName) {
  return join(projectDir, "node_modules", ...packageName.split("/"));
}

function buildSpecs(row, probe) {
  const specs = [`${row.package}@${row.version}`];
  for (const [name, version] of Object.entries(probe.solid ?? {})) {
    specs.push(`${name}@${version}`);
  }
  return specs;
}

// `expected` feeds `verifyInstall` from lib/install.mjs directly. Only the
// probed package itself carries an integrity pin in the manifest row; the
// Solid runtime packages are checked by version only (verifyInstall already
// skips the integrity comparison whenever `want.integrity` is falsy).
function buildExpectedVersions(row, probe) {
  const expected = { [row.package]: { version: row.version, integrity: row.integrity ?? null } };
  for (const [name, version] of Object.entries(probe.solid ?? {})) {
    expected[name] = { version, integrity: null };
  }
  return expected;
}

// One file per probe, sibling-safe: probe ids contain "/" (scoped package
// names) which would otherwise be read as directory separators.
function sanitizeProbeId(id) {
  return id.replace(/\//g, "__").replace(/\|/g, "--");
}

// Supplemental rows are the unofficial forks and lookalikes discovery found.
// They are recorded in the manifest so a reviewer can SEE them, but they are
// not part of the corpus: running them by default folded fork results into the
// official family's probe counts and success rate, which is exactly the
// conflation the manifest's official/supplemental split exists to prevent.
// `--include-supplemental` opts in deliberately; the report still keeps them
// out of the official totals either way.
function collectProbeTasks(manifest, idFilter, { includeSupplemental = false } = {}) {
  const supplemental = includeSupplemental ? (manifest?.supplemental ?? []) : [];
  const rows = sortRows([...(manifest?.rows ?? []), ...supplemental]);
  const tasks = [];
  for (const row of rows) {
    for (const probe of row.probes ?? []) {
      if (idFilter && !idFilter.has(probe.id)) continue;
      tasks.push({ row, probe });
    }
  }
  return tasks;
}

// Every family/solid/sentinel filter converges here into one probe-id set (or
// `null` meaning "every probe in the manifest"), so `runBenchmark` itself
// only ever has to understand a flat id filter.
// The scope a run actually covered, as data rather than as an assumption the
// reader has to make. A filtered run and the full corpus are not
// interchangeable artifacts: a sentinel report contains 23 results while its
// header still describes the manifest's 417 probes, so without this a partial
// report is indistinguishable from a full one at a glance.
export function runScope({
  sentinel = false,
  families = [],
  solidTargets = [],
  includeSupplemental = false
} = {}) {
  const filters = [];
  if (sentinel) filters.push("sentinel");
  for (const family of [...families].sort()) filters.push(`family-${family}`);
  for (const target of [...solidTargets].sort()) filters.push(`solid${target}`);
  return {
    kind: filters.length ? "filtered" : "full",
    sentinel,
    families: [...families].sort(),
    solidTargets: [...solidTargets].sort(),
    includeSupplemental,
    // A stable, order-independent name for this scope. `full` owns the
    // canonical report path; every filter earns its own so it can never
    // overwrite the corpus-wide artifact by default.
    slug: filters.length ? filters.join("-") : "full"
  };
}

// Default report paths derive from the scope. An explicit --json/--markdown
// always wins, so CI can still pin a path of its own.
export function defaultReportPaths(scope, directory = join(ROOT, "benchmarks/ecosystem")) {
  const suffix = scope.kind === "full" ? "" : `-${scope.slug}`;
  return {
    json: join(directory, `report${suffix}.json`),
    markdown: join(directory, `report${suffix}.md`)
  };
}

function describeScopeShort(scope) {
  if (!scope || scope.kind === "full") return "full corpus";
  const filters = [];
  if (scope.sentinel) filters.push("sentinel");
  for (const family of scope.families ?? []) filters.push(`family=${family}`);
  for (const target of scope.solidTargets ?? []) filters.push(`solid${target}`);
  return filters.length ? filters.join(" ") : "filtered";
}

export function resolveProbeIdFilter({ manifest, families = [], solidTargets = [], sentinelIds = null }) {
  const noFilter = families.length === 0 && solidTargets.length === 0 && sentinelIds === null;
  if (noFilter) return null;

  const normalizedTargets = solidTargets.map(target => (target === "1" ? "solid1" : target === "2" ? "solid2" : target));
  const sentinelSet = sentinelIds ? new Set(sentinelIds) : null;

  const ids = [];
  // Resolving a filter over supplemental rows too is harmless -- it only maps
  // ids -- and it means an explicitly requested fork probe id still resolves
  // when the caller opted in with --include-supplemental.
  for (const row of [...(manifest?.rows ?? []), ...(manifest?.supplemental ?? [])]) {
    if (families.length && !families.includes(row.family)) continue;
    if (normalizedTargets.length && !normalizedTargets.includes(row.solidTarget)) continue;
    for (const probe of row.probes ?? []) {
      if (sentinelSet && !sentinelSet.has(probe.id)) continue;
      ids.push(probe.id);
    }
  }
  return ids;
}

// Concurrency-limited map that always writes into a pre-sized array by
// index, so results come back in `items` order no matter which worker
// finishes first — completion order and report order are deliberately
// decoupled.
async function mapConcurrent(items, concurrency, worker) {
  const results = new Array(items.length);
  let cursor = 0;
  const size = Math.max(1, Math.min(concurrency || 1, items.length || 1));
  const runners = Array.from({ length: size }, async () => {
    while (cursor < items.length) {
      const index = cursor++;
      results[index] = await worker(items[index], index);
    }
  });
  await Promise.all(runners);
  return results;
}

// The three outcomes a probe can be filed under. `success` means a COMPLETE
// contract — every declared entrypoint the generator reached is described by
// it. A contract with refused entrypoints is real output and is recorded as
// such, but it is its own outcome: folding it into `success` would let the
// corpus-wide rate read 100% while a third of the ecosystem's entrypoints went
// undescribed. Nothing here ever moves a probe the other way (a failure into
// a success), which is the rule the benchmark exists under — it may only ever
// make its own rate stricter.
export function probeOutcome(className) {
  if (className === "success") return "success";
  if (className === "partial-success") return "partial-success";
  return "failure";
}

function buildResult({
  row,
  probe,
  installedVersions,
  integrityVerified,
  declaredEntrypoints,
  generatedEntrypoints,
  refusedEntrypoints = null,
  checklistItems,
  // What the emitted contract actually claims, as opposed to whether it was
  // emitted. Null for every probe that never wrote one; see
  // lib/contract-content.mjs for why an unparsable contract is `measured:
  // false` rather than a row of zeroes.
  contractContent = null,
  outcome,
  classification,
  exitStatus,
  timedOut,
  durationMs,
  installDurationMs,
  generationDurationMs,
  stdout,
  stderr
}) {
  return {
    probeId: probe.id,
    family: row.family,
    status: row.status,
    package: row.package,
    version: row.version,
    solidTarget: row.solidTarget,
    probeKind: probe.kind,
    channel: probe.channel,
    solid: probe.solid,
    installedVersions,
    integrityVerified,
    declaredEntrypoints,
    generatedEntrypoints,
    refusedEntrypoints,
    checklistItems,
    contractContent,
    outcome,
    class: classification.class,
    signature: classification.signature,
    detail: classification.detail,
    exitStatus,
    timedOut,
    durationMs,
    installDurationMs,
    generationDurationMs,
    stdout,
    stderr
  };
}

// A hook throwing (rather than resolving with a status/stderr shape) is
// still just this one probe's failure — never rethrown, always folded into
// the same result shape every other failure produces.
function buildInfraFailureResult({ row, probe, error, phase, durationMs }) {
  const message = error?.stack ?? String(error);
  const classification = classifyResult({ status: 1, stdout: "", stderr: message, timedOut: false, phase });
  return buildResult({
    row,
    probe,
    installedVersions: {},
    integrityVerified: false,
    declaredEntrypoints: null,
    generatedEntrypoints: null,
    checklistItems: null,
    outcome: "failure",
    classification,
    exitStatus: null,
    timedOut: false,
    durationMs,
    installDurationMs: null,
    generationDurationMs: null,
    stdout: "",
    stderr: message
  });
}

async function runProbe({ row, probe }, { timeoutMs, keepTemp }, hooks) {
  const now = hooks.now ?? Date.now;
  const overallStart = now();
  const specs = buildSpecs(row, probe);
  const expected = buildExpectedVersions(row, probe);

  let project;
  try {
    project = await hooks.mkProject({ probeId: probe.id, row, probe });
  } catch (error) {
    return buildInfraFailureResult({ row, probe, error, phase: "install", durationMs: now() - overallStart });
  }

  const { projectDir, outputDir } = project;

  try {
    const installStart = now();
    let installResult;
    try {
      installResult = await hooks.installPackages({ projectDir, specs, expected, timeoutMs });
    } catch (error) {
      installResult = { status: 1, stdout: "", stderr: error?.stack ?? String(error), timedOut: false };
    }
    const installDurationMs = now() - installStart;

    const installedVersions = installResult.installedVersions ?? {};
    const packageJsonPath = packageInstallPath(projectDir, row.package) + "/package.json";
    const declaredEntrypoints = readDeclaredEntrypointCount(packageJsonPath);

    const installClass = classifyResult({
      status: installResult.status,
      stdout: installResult.stdout,
      stderr: installResult.stderr,
      timedOut: installResult.timedOut,
      phase: "install"
    });

    // Verifying a failed install would only relabel the same failure, so
    // verification only runs once install itself reported success.
    const verify =
      installClass.class === "success"
        ? verifyInstall({ expected, versions: installedVersions, integrity: installResult.integrity ?? {} })
        : { ok: true, problems: [] };

    if (installClass.class !== "success" || !verify.ok) {
      const classification =
        installClass.class !== "success"
          ? installClass
          : {
              class: verify.problems.some(problem => problem.kind === "integrity-mismatch")
                ? "integrity-failure"
                : "install-failure",
              signature: normalizeSignature(verify.problems.map(problem => `${problem.kind}: ${problem.package}`).join("; ")),
              detail: { problems: verify.problems },
              raw: { stdout: installResult.stdout ?? "", stderr: installResult.stderr ?? "" }
            };
      return buildResult({
        row,
        probe,
        installedVersions,
        integrityVerified: verify.ok,
        declaredEntrypoints,
        generatedEntrypoints: null,
        checklistItems: null,
        outcome: "failure",
        classification,
        exitStatus: installResult.status ?? null,
        timedOut: installResult.timedOut ?? false,
        durationMs: now() - overallStart,
        installDurationMs,
        generationDurationMs: null,
        stdout: installResult.stdout ?? "",
        stderr: installResult.stderr ?? ""
      });
    }

    const outputPath = join(outputDir, `${sanitizeProbeId(probe.id)}.json`);
    const packageRoot = packageInstallPath(projectDir, row.package);

    const generationStart = now();
    let genResult;
    try {
      genResult = await hooks.generateContract({ packageRoot, outputPath, timeoutMs });
    } catch (error) {
      genResult = { status: 1, stdout: "", stderr: error?.stack ?? String(error), timedOut: false };
    }
    const generationDurationMs = now() - generationStart;

    const genClass = classifyResult({
      status: genResult.status,
      stdout: genResult.stdout,
      stderr: genResult.stderr,
      timedOut: genResult.timedOut,
      phase: "generate"
    });

    // A partial contract is still a contract on disk: its generated-entrypoint
    // count and checklist are real measurements and are recorded exactly like
    // a complete one's. What changes is the outcome it is filed under -- see
    // `probeOutcome`.
    const producedContract = genClass.class === "success" || genClass.class === "partial-success";
    const generatedEntrypoints = producedContract ? readGeneratedEntrypointCount(outputPath) : null;
    const checklistItems = genClass.detail?.checklistItems ?? null;
    // Read here, inside the try, because the `finally` below deletes the
    // output directory: after cleanup there is nothing left to measure, and a
    // probe whose content went unread would be indistinguishable from a probe
    // whose contract had nothing in it.
    const contractContent = producedContract
      ? readContractContent(outputPath, genClass.detail?.refusedEntrypoints ?? 0)
      : null;

    return buildResult({
      row,
      probe,
      installedVersions,
      integrityVerified: true,
      declaredEntrypoints,
      generatedEntrypoints,
      refusedEntrypoints: genClass.detail?.refusedEntrypoints ?? null,
      checklistItems,
      contractContent,
      outcome: probeOutcome(genClass.class),
      classification: genClass,
      exitStatus: genResult.status ?? null,
      timedOut: genResult.timedOut ?? false,
      durationMs: now() - overallStart,
      installDurationMs,
      generationDurationMs,
      stdout: genResult.stdout ?? "",
      stderr: genResult.stderr ?? ""
    });
  } finally {
    // "unless --keep-temp": the decision lives here, in the run core, rather
    // than inside the hook, so a test can prove cleanup was skipped entirely
    // without needing a hook that behaves differently per flag.
    if (!keepTemp) {
      try {
        await hooks.cleanup({ projectDir, outputDir });
      } catch {
        // A cleanup failure (e.g. a file the package's install left
        // read-only) must never overwrite this probe's already-computed
        // result, and must never abort the run.
      }
    }
  }
}

// The injectable core. Takes a validated manifest, an optional explicit
// probe-id filter (`null` means every probe in the manifest), run options,
// and the four side-effecting hooks plus a clock. Returns results in
// deterministic manifest order regardless of completion order, and never
// rejects because one probe's hooks threw — see `runProbe`.
export async function runBenchmark({ manifest, probeIds = null, options = {}, hooks }) {
  const timeoutMs = (options.timeoutMs ?? DEFAULT_TIMEOUT_SECONDS * 1000) | 0;
  const concurrency = options.concurrency ?? DEFAULT_CONCURRENCY;
  const keepTemp = options.keepTemp ?? false;

  const idFilter = probeIds ? new Set(probeIds) : null;
  const tasks = collectProbeTasks(manifest, idFilter, {
    includeSupplemental: Boolean(options.includeSupplemental)
  });

  return mapConcurrent(tasks, concurrency, task => runProbe(task, { timeoutMs, keepTemp }, hooks));
}

// ---------------------------------------------------------------------------
// CLI wrapper: argv parsing, resolving inputs, building real hooks, writing
// reports. Everything above this line is pure or hook-driven and exercised
// by run.test.mjs without touching npm, the network, or the real checker.
// ---------------------------------------------------------------------------

function usage() {
  return `Usage: bun scripts/ecosystem-benchmark/run.mjs [options]

  --manifest <FILE>      default scripts/ecosystem-benchmark/manifest.json
  --sentinel             run only the pinned sentinel subset
  --family <ID>          restrict to one family (repeatable)
  --solid <1|2>          restrict to one Solid target (repeatable)
  --json <FILE>          default benchmarks/ecosystem/report<-scope>.json
  --markdown <FILE>      default benchmarks/ecosystem/report<-scope>.md
                         Only an unfiltered run defaults to the canonical
                         report.json/report.md; --sentinel, --family and
                         --solid each derive their own name so a subset can
                         never overwrite the full-corpus artifact.
  --baseline <FILE>      compare against a pinned previous run
  --thresholds <FILE>    threshold mode: exit 1 when a threshold regresses
  --timeout <SECONDS>    per-probe generation timeout, default 300
  --concurrency <N>      default min(available CPUs, 8), currently ${DEFAULT_CONCURRENCY}
  --keep-temp            keep the temporary install directories
  --include-supplemental run the unofficial fork rows too (off by default:
                         forks are listed for review, not part of the corpus)
  -h, --help

Requires SOLID_CHECKER_NATIVE_BIN and SOLID_TYPEFACTS_BIN to be set and to
exist. Exit 0 once the benchmark infrastructure ran, regardless of
per-package outcomes; exit 1 only in --thresholds mode on a regression;
exit 2 for an infrastructure failure (bad manifest, missing binaries,
harness crash, unwritable reports).
`;
}

function parseArgs(argv) {
  const options = {
    manifest: DEFAULT_MANIFEST,
    sentinel: false,
    families: [],
    solidTargets: [],
    // Left null until the scope is known: the default path depends on which
    // subset the run covers. An explicit flag sets it and wins.
    json: null,
    markdown: null,
    baseline: null,
    thresholds: null,
    timeoutSeconds: DEFAULT_TIMEOUT_SECONDS,
    concurrency: DEFAULT_CONCURRENCY,
    keepTemp: false,
    includeSupplemental: false,
    help: false
  };

  const errors = [];
  const takeValue = (args, index, flag) => {
    const value = args[index + 1];
    if (value === undefined) errors.push(`${flag} requires a value`);
    return value;
  };

  for (let index = 0; index < argv.length; index++) {
    const arg = argv[index];
    switch (arg) {
      case "-h":
      case "--help":
        options.help = true;
        break;
      case "--manifest":
        options.manifest = takeValue(argv, index++, arg);
        break;
      case "--sentinel":
        options.sentinel = true;
        break;
      case "--family":
        options.families.push(takeValue(argv, index++, arg));
        break;
      case "--solid":
        options.solidTargets.push(takeValue(argv, index++, arg));
        break;
      case "--json":
        options.json = takeValue(argv, index++, arg);
        break;
      case "--markdown":
        options.markdown = takeValue(argv, index++, arg);
        break;
      case "--baseline":
        options.baseline = takeValue(argv, index++, arg);
        break;
      case "--thresholds":
        options.thresholds = takeValue(argv, index++, arg);
        break;
      case "--timeout":
        options.timeoutSeconds = Number(takeValue(argv, index++, arg));
        break;
      case "--concurrency":
        options.concurrency = Number(takeValue(argv, index++, arg));
        break;
      case "--keep-temp":
        options.keepTemp = true;
        break;
      case "--include-supplemental":
        options.includeSupplemental = true;
        break;
      default:
        errors.push(`unrecognized option: ${arg}`);
    }
  }

  return { options, errors };
}

function readJsonFile(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

function readSentinelIds(path) {
  const parsed = readJsonFile(path);
  if (parsed?.schemaVersion !== 1 || !Array.isArray(parsed.probes)) {
    throw new Error(`${path} is not a valid sentinel document (schemaVersion 1 with a "probes" array)`);
  }
  return parsed.probes;
}

// Real, side-effecting hooks: exactly the four `runBenchmark` needs, backed
// by lib/install.mjs (for npm) and a spawned CLI subprocess (for
// generation) — never a `require`/`import` of anything under an installed
// package's own node_modules tree.
function buildRealHooks({ nativeBin, typeFactsBin, cliPath }) {
  return {
    now: () => Date.now(),

    mkProject: async () => {
      const projectDir = await mkdtemp(join(tmpdir(), "solid-checker-ecosystem-"));
      const outputDir = await mkdtemp(join(tmpdir(), "solid-checker-ecosystem-out-"));
      return { projectDir, outputDir };
    },

    installPackages: async ({ projectDir, specs, expected, timeoutMs }) => {
      await createProject({ root: projectDir, specs });
      const result = await bunInstall({ projectDir, specs, timeoutMs });
      const names = Object.keys(expected);
      const installedVersions = readInstalledVersions(projectDir, names);
      const integrity = readLockIntegrity(projectDir, names);
      return { ...result, installedVersions, integrity };
    },

    generateContract: ({ packageRoot, outputPath, timeoutMs }) =>
      new Promise(resolvePromise => {
        const child = spawn(
          process.execPath,
          [cliPath, "contract", "generate", "--package-root", packageRoot, "--output", outputPath],
          {
            env: { ...process.env, SOLID_CHECKER_NATIVE_BIN: nativeBin, SOLID_TYPEFACTS_BIN: typeFactsBin },
            stdio: ["ignore", "pipe", "pipe"]
          }
        );
        let stdout = "";
        let stderr = "";
        let timedOut = false;
        const timer = timeoutMs
          ? setTimeout(() => {
              timedOut = true;
              child.kill("SIGKILL");
            }, timeoutMs)
          : null;
        child.stdout.on("data", chunk => {
          stdout += chunk;
        });
        child.stderr.on("data", chunk => {
          stderr += chunk;
        });
        child.on("close", status => {
          if (timer) clearTimeout(timer);
          resolvePromise({ status, stdout, stderr, timedOut });
        });
      }),

    cleanup: async ({ projectDir, outputDir }) => {
      await rm(projectDir, { recursive: true, force: true });
      await rm(outputDir, { recursive: true, force: true });
    }
  };
}

function fail(message, code = 2) {
  console.error(`solid-checker-ecosystem-benchmark: ${message}`);
  process.exit(code);
}

async function main(argv = process.argv.slice(2)) {
  const { options, errors } = parseArgs(argv);

  if (options.help) {
    console.log(usage());
    return;
  }

  if (errors.length) {
    console.error(usage());
    fail(errors.join("; "));
    return;
  }

  // Checked first and unconditionally: no manifest read, no bun install, no
  // subprocess spawn is worth attempting against binaries we have not
  // confirmed exist. See the file header for why there is no fallback.
  const binaries = checkRequiredBinaries(process.env);
  if (!binaries.ok) {
    fail(binaries.problems.join("; "));
    return;
  }

  let manifest;
  try {
    manifest = readJsonFile(options.manifest);
  } catch (error) {
    fail(`cannot read manifest ${options.manifest}: ${error.message}`);
    return;
  }

  const manifestProblems = validateManifest(manifest);
  if (manifestProblems.length) {
    console.error(`solid-checker-ecosystem-benchmark: manifest ${options.manifest} failed validation:`);
    for (const problem of manifestProblems) console.error(`  - ${problem}`);
    process.exit(2);
    return;
  }

  let sentinelIds = null;
  if (options.sentinel) {
    try {
      sentinelIds = readSentinelIds(DEFAULT_SENTINEL);
    } catch (error) {
      fail(`cannot read sentinel file ${DEFAULT_SENTINEL}: ${error.message}`);
      return;
    }
  }

  const scope = runScope({
    sentinel: options.sentinel,
    families: options.families,
    solidTargets: options.solidTargets,
    includeSupplemental: options.includeSupplemental
  });
  const defaults = defaultReportPaths(scope);
  options.json ??= defaults.json;
  options.markdown ??= defaults.markdown;

  const probeIds = resolveProbeIdFilter({
    manifest,
    families: options.families,
    solidTargets: options.solidTargets,
    sentinelIds
  });

  let baseline = null;
  if (options.baseline) {
    try {
      baseline = readJsonFile(options.baseline);
    } catch (error) {
      fail(`cannot read baseline ${options.baseline}: ${error.message}`);
      return;
    }
    // Comparing across scopes is not a regression signal, it is an artifact
    // mismatch: a full run measured against a sentinel baseline reports every
    // probe the sentinel never ran as removed. That is harness misuse, so it
    // exits 2 with the two scopes named rather than printing a diff nobody
    // should act on.
    const baselineScope = baseline?.scope;
    if (baselineScope && baselineScope.kind !== undefined) {
      const sameScope =
        baselineScope.kind === scope.kind &&
        Boolean(baselineScope.sentinel) === scope.sentinel &&
        JSON.stringify(baselineScope.families ?? []) === JSON.stringify(scope.families) &&
        JSON.stringify(baselineScope.solidTargets ?? []) === JSON.stringify(scope.solidTargets);
      if (!sameScope) {
        fail(
          `baseline ${options.baseline} covers a different scope than this run ` +
            `(baseline: ${describeScopeShort(baselineScope)}; this run: ${describeScopeShort(scope)}); ` +
            "compare runs of the same scope"
        );
        return;
      }
    } else {
      // A report written before scopes were recorded cannot state what it
      // covered. Say so instead of silently assuming it matches.
      console.error(
        `solid-checker-ecosystem-benchmark: warning: baseline ${options.baseline} predates run-scope ` +
          "recording; it may not cover the same probes as this run"
      );
    }
  }

  let thresholds = null;
  if (options.thresholds) {
    try {
      thresholds = readJsonFile(options.thresholds);
    } catch (error) {
      fail(`cannot read thresholds file ${options.thresholds}: ${error.message}`);
      return;
    }
  }

  const hooks = buildRealHooks({ nativeBin: binaries.nativeBin, typeFactsBin: binaries.typeFactsBin, cliPath: DEFAULT_CLI });

  const startedAt = new Date().toISOString();
  let results;
  try {
    results = await runBenchmark({
      manifest,
      probeIds,
      options: {
        timeoutMs: options.timeoutSeconds * 1000,
        concurrency: options.concurrency,
        keepTemp: options.keepTemp
      },
      hooks
    });
  } catch (error) {
    // runBenchmark itself is designed to never reject over a single probe's
    // behavior (see runProbe) — reaching here means the harness itself
    // broke, which is exactly the infrastructure-failure case exit 2 exists
    // for.
    fail(`benchmark harness crashed: ${error?.stack ?? error}`);
    return;
  }
  const finishedAt = new Date().toISOString();

  let report;
  try {
    report = buildReport({
      manifest,
      results,
      startedAt,
      finishedAt,
      baseline,
      // Not one of the five parameters INTERFACES.md names, but buildReport's
      // documented top-level `checker: { nativeBin, typeFactsBin }` field has
      // no other source of this data — see lib/report.mjs's own comment on
      // this parameter.
      checker: { nativeBin: binaries.nativeBin, typeFactsBin: binaries.typeFactsBin },
      scope
    });
  } catch (error) {
    fail(`failed to build report: ${error?.stack ?? error}`);
    return;
  }

  let markdown;
  try {
    markdown = renderMarkdown(report);
  } catch (error) {
    fail(`failed to render markdown report: ${error?.stack ?? error}`);
    return;
  }

  try {
    mkdirSync(dirname(options.json), { recursive: true });
    mkdirSync(dirname(options.markdown), { recursive: true });
    writeFileSync(options.json, `${JSON.stringify(report, null, 2)}\n`, "utf8");
    writeFileSync(options.markdown, markdown, "utf8");
  } catch (error) {
    fail(`failed to write reports: ${error?.stack ?? error}`);
    return;
  }

  let evaluation = null;
  if (thresholds) {
    try {
      evaluation = evaluateThresholds(report, thresholds);
    } catch (error) {
      fail(`failed to evaluate thresholds: ${error?.stack ?? error}`);
      return;
    }
  }

  const exitCode = decideExitCode({ thresholdsRequested: Boolean(options.thresholds), evaluation });
  const successCount = results.filter(result => result.outcome === "success").length;
  const partialCount = results.filter(result => result.outcome === "partial-success").length;
  console.log(
    `solid-checker-ecosystem-benchmark: ${results.length} probes, ${successCount} complete contracts, ` +
      `${partialCount} partial, reports written to ${options.json} and ${options.markdown}`
  );
  if (evaluation && evaluation.ok === false) {
    console.error("solid-checker-ecosystem-benchmark: threshold regression:");
    for (const failure of evaluation.failures ?? []) console.error(`  - ${JSON.stringify(failure)}`);
  }
  process.exit(exitCode);
}

const isMain = process.argv[1] && pathToFileURL(resolve(process.argv[1])).href === import.meta.url;
if (isMain) {
  main().catch(error => {
    console.error(`solid-checker-ecosystem-benchmark: unhandled error: ${error?.stack ?? error}`);
    process.exit(2);
  });
}
