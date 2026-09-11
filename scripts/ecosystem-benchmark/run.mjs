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
//   whether the run "succeeded" -- that is `--thresholds` mode's job, and
//   only `--thresholds` mode can turn probe-level content into a non-zero
//   exit. Exit 2 is reserved for the harness itself failing to do its job
//   (bad manifest, missing binaries, a crash in the runner, unwritable
//   reports) -- never for what the packages under test happened to do. Even a
//   run where every single probe fails to install is still exit 0: the
//   runner reported faithfully, it did not judge.

import { execFileSync, spawn } from "node:child_process";
import { createHash, randomBytes } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  realpathSync,
  writeFileSync
} from "node:fs";
import { mkdtemp, rm } from "node:fs/promises";
import { availableParallelism, tmpdir, totalmem } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { classifyResult, normalizeSignature } from "./lib/classify.mjs";
import { readCertifiedCoverage } from "./lib/certified-coverage.mjs";
import { readContractContent, readProposalRefusalAudit } from "./lib/contract-content.mjs";
import { planRecursiveDependencies } from "./lib/dependency-plan.mjs";
import {
  collectExternalEdges,
  isDependencyCompositionRefusalText
} from "./lib/external-edges.mjs";
import {
  createProject,
  installPackages as bunInstall,
  readInstalledVersions,
  readLockIntegrity,
  verifyInstall
} from "./lib/install.mjs";
import { sortRows, validateManifest } from "./lib/manifest.mjs";
import { buildReport, evaluateThresholds, renderMarkdown } from "./lib/report.mjs";
import {
  certificationImporterPathFor,
  partialProposalHasDependencyFrontier
} from "../../packages/cli/scripts/certify-contract.mjs";
import { createCliWorkerPool } from "./lib/cli-worker-pool.mjs";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const DEFAULT_MANIFEST = join(ROOT, "scripts/ecosystem-benchmark/manifest.json");
const DEFAULT_SENTINEL = join(ROOT, "scripts/ecosystem-benchmark/sentinel.json");
const DEFAULT_SCHEDULE_REPORT = join(ROOT, "benchmarks/ecosystem/report.json");
// Content-addressed registry bytes shared by every certification child of a
// run (and across runs). Lives under rust/target with the other ignored,
// reclaimable build products; `make clean` wipes it. See
// packages/cli/scripts/certify-contract.mjs for what an entry is and the
// checks it is held to before it is used.
const DEFAULT_REGISTRY_CACHE = join(ROOT, "rust/target/registry-cache");
// Each probe's resolved package.json and bun.lock, keyed by its exact spec set,
// so a later run installs frozen (no registry manifest round trips) and with
// the same transitive resolution. See lib/install.mjs.
const DEFAULT_INSTALL_LOCKFILE_CACHE = join(ROOT, "rust/target/install-locks");
// Materialized compiler-source packages shared by every certification, linked
// into each private Type Facts project instead of written; see
// rust/crates/solid-facts-backend/src/contract_certification/type_facts.rs
// (`materialized_store_root`) for what an entry is and how it is verified.
const DEFAULT_MATERIALIZED_STORE = join(ROOT, "rust/target/materialized-store");
const DEFAULT_TIMEOUT_SECONDS = 300;
const PROGRESS_HEARTBEAT_INTERVAL_MS = 30_000;

// Eight generation jobs avoid the measured Bun/install and wide-proposal
// contention cliff. Certification uses otherwise-idle host slots below, so
// the long receipt drain does not force generation itself past this bound.
export function recommendedConcurrency(parallelism = availableParallelism()) {
  if (!Number.isInteger(parallelism) || parallelism <= 0) return 4;
  return Math.min(8, parallelism);
}

const DEFAULT_CONCURRENCY = recommendedConcurrency();

// Certification is a large share of measured worker time. Let its drain phase
// use the host directly while generation retains its smaller install-safe
// outer pool; the child artifact-analysis width is derived separately below.
//
// The drain may run wider than the core count. Once registry bytes come from
// the shared cache, a certification child spends most of its slot time
// waiting — for a core under a load average above the core count, and while
// it materializes and removes its private Type Facts project — rather than
// computing: on the 14-core authority host the 384 native certifications of
// one corpus run held 1,300 s of slot time for 199 s of CPU. Six extra slots,
// the same number generation reserves, fill that waiting time: 176-178 s
// against 185-190 s at a cores-bounded fourteen, with identical outcomes, and
// 24 slots were no faster than 20.
//
// The width is bounded by memory as well as cores, because a heavy probe's
// process tree is a real working set and enough of them in flight together can
// exhaust the host. One memory share per certification slot is reserved from
// total RAM; SOLID_CHECKER_CERTIFICATION_CONCURRENCY overrides the computed
// width, mirroring SOLID_CHECKER_GATE_CONCURRENCY for the gates.
//
// The share is 2 GiB, from measurement rather than from a guess. The share was
// first set to 8 GiB after a 14-wide drain took down a 48 GB machine, when the
// resolver's module-description cache retained one whole `ts.Program` per
// module needing symbol identity; the heavy tail then peaked far above the
// nominal 2.5 GB -- 30.5 GB for `@solidjs/start@2.0.3`, 25.1 GB for
// `@solid-devtools/transform@0.10.4`, 10.4 GB for `@kobalte/core@2.0.0-alpha.0`
// -- and 38 probes were killed by the ceiling below. With that retention
// released (see `moduleDescription` in packages/cli/scripts/artifact-resolution.mjs)
// the same six probes' worst process-tree peak is 762 MiB, the whole set
// spanning 496-762 MiB. Two gigabytes is therefore ~2.7x the measured worst
// peak, and it lets a 48 GB host run the full cores-bounded width while a
// 4 GB machine still floors at two slots.
const CERTIFICATION_MEMORY_SHARE_BYTES = 2 * 1024 * 1024 * 1024;

export function recommendedCertificationConcurrency(
  parallelism = availableParallelism(),
  totalMemoryBytes = totalmem()
) {
  if (!Number.isInteger(parallelism) || parallelism <= 0) return 2;
  const memorySlots = Number.isFinite(totalMemoryBytes) && totalMemoryBytes > 0
    ? Math.max(2, Math.floor(totalMemoryBytes / CERTIFICATION_MEMORY_SHARE_BYTES))
    : 2;
  return Math.min(CERTIFICATION_SLOT_CEILING, parallelism + CERTIFICATION_OVERSUBSCRIPTION, memorySlots);
}

const CERTIFICATION_OVERSUBSCRIPTION = 6;
const CERTIFICATION_SLOT_CEILING = 20;

export function certificationConcurrencyFromEnvironment(env = process.env) {
  const raw = env.SOLID_CHECKER_CERTIFICATION_CONCURRENCY;
  if (raw === undefined || raw === "") return undefined;
  const parsed = Number(raw);
  if (!Number.isInteger(parsed) || parsed <= 0) return undefined;
  return parsed;
}

const DEFAULT_CERTIFICATION_CONCURRENCY =
  certificationConcurrencyFromEnvironment() ?? recommendedCertificationConcurrency();

export function recommendedCertificationInnerConcurrency(
  certificationConcurrency,
  parallelism = availableParallelism()
) {
  if (!Number.isInteger(certificationConcurrency) || certificationConcurrency <= 0) return 1;
  if (!Number.isInteger(parallelism) || parallelism <= 0) return 1;
  return Math.min(8, Math.max(1, Math.floor(parallelism / certificationConcurrency)));
}

// The real runner intentionally buffers each probe's child output so package
// diagnostics become report data rather than interleaved console noise. Keep
// the CLI visibly alive while that bounded work runs; this is operational
// progress only and never enters a result, report, digest, or threshold.
export function startProgressHeartbeat({
  intervalMs = PROGRESS_HEARTBEAT_INTERVAL_MS,
  writeLine = line => console.error(line),
  schedule = (callback, delay) => setInterval(callback, delay),
  cancel = timer => clearInterval(timer)
} = {}) {
  let beats = 0;
  let stopped = false;
  const timer = schedule(() => {
    beats += 1;
    try {
      writeLine(
        `solid-checker-ecosystem-benchmark: still running (${beats * intervalMs / 1000}s heartbeat; reports follow all probes)`
      );
    } catch {
      // Losing a progress-only sink must not turn completed semantic work into
      // a harness failure. The final report write remains authoritative.
    }
  }, intervalMs);

  return () => {
    if (stopped) return;
    stopped = true;
    cancel(timer);
  };
}

// ---------------------------------------------------------------------------
// Pure helpers (exported for direct unit testing).
// ---------------------------------------------------------------------------

// Verifies the two binaries execution absolutely cannot proceed without.
// Exported standalone (rather than folded into `main`) so a test can prove
// the exact missing path is named without needing real binaries on disk.
// The registry cache handed to certification children: an explicit
// `--registry-cache` wins, then a non-empty SOLID_CHECKER_REGISTRY_CACHE from
// the environment, then the repository default; `--no-registry-cache` yields
// null, which the hooks pass down as an empty variable so a child never
// inherits a cache the run said not to use.
export function resolveRegistryCache({
  option = null,
  disabled = false,
  env = process.env,
  fallback = DEFAULT_REGISTRY_CACHE
} = {}) {
  if (disabled) return null;
  if (option) return resolve(option);
  const fromEnvironment = env.SOLID_CHECKER_REGISTRY_CACHE;
  if (fromEnvironment !== undefined && fromEnvironment !== "") return resolve(fromEnvironment);
  return fallback;
}

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
//
// `wildcard` travels with the count because it is the only place it can be
// seen. A wildcard key makes `count` a *pattern* count that expands to as many
// real entrypoints as the package ships, so it is not a denominator -- and no
// comparison of the two numbers downstream can recover that, since a wildcard
// expansion may coincidentally certify exactly as many entrypoints as the
// manifest declares. See `hasUsableDenominator` in lib/certified-coverage.mjs.
export function countDeclaredEntrypoints(exportsField) {
  if (exportsField == null) return { count: 0, wildcard: false };
  if (typeof exportsField === "string") return { count: 1, wildcard: false };
  if (typeof exportsField !== "object") return { count: 0, wildcard: false };
  const keys = Object.keys(exportsField);
  if (keys.length === 0) return { count: 0, wildcard: false };
  // No key starting with "." means this object is itself the conditions map
  // for the "." entrypoint (e.g. { "import": "...", "require": "..." }).
  if (!keys.some(key => key.startsWith("."))) return { count: 1, wildcard: false };
  return { count: keys.length, wildcard: keys.some(key => key.includes("*")) };
}

export function readDeclaredEntrypointCensus(packageJsonPath) {
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


// ADR 0036: the withheld records by the reason they carry. `noRecipe` is the
// pre-ADR reason; the census and veto buckets are what the certifier withholds
// for instead of refusing the row. A veto that did not complete is split by
// the parenthesized account the certifier attaches: `vetoUnreproducible` is a
// synthesized veto the pinned interpreter cannot run for the artifact case
// (Node's own `node` condition would load `dist/server.js` where the witness
// read `dist/solid.js`), `vetoThrew` / `vetoTimedOut` / `vetoRunRefused` are
// the worker's own failure, budget, and refusal, and `vetoIncomplete` is a
// record with no account at all.
export const WITHHELD_CLOSURE_REASON_BUCKETS = Object.freeze([
  "noRecipe",
  "censusRefused",
  "vetoUnreproducible",
  "vetoThrew",
  "vetoTimedOut",
  "vetoRunRefused",
  "vetoIncomplete",
  "dependencyWithheld",
  "other"
]);

function withheldClosureReasons(audit) {
  const counts = Object.fromEntries(WITHHELD_CLOSURE_REASON_BUCKETS.map((bucket) => [bucket, 0]));
  for (const record of Array.isArray(audit?.withheldClosures) ? audit.withheldClosures : []) {
    const reason = typeof record?.reason === "string" ? record.reason : "";
    if (reason === "no recipe in corpus") counts.noRecipe += 1;
    else if (reason.startsWith("census refused: ")) counts.censusRefused += 1;
    else if (reason.startsWith("veto did not complete: ")) {
      // The account follows the gate digest in parentheses and is prefixed
      // by the probe mode that did not complete (`certification-probe: the
      // worker threw: …`), so the markers are matched anywhere in it.
      if (reason.includes("synthesized veto cannot run for this artifact case:")) counts.vetoUnreproducible += 1;
      else if (reason.includes("the worker threw")) counts.vetoThrew += 1;
      else if (reason.includes("the worker did not report within")) counts.vetoTimedOut += 1;
      else if (reason.includes("the run was refused:")) counts.vetoRunRefused += 1;
      else counts.vetoIncomplete += 1;
    }
    // A graph node whose closure composes from a dependency claim the
    // dependency withheld (ADR 0036, graph lanes).
    else if (reason.startsWith("composed from a withheld dependency claim: ")) counts.dependencyWithheld += 1;
    else counts.other += 1;
  }
  return counts;
}

function packageInstallPath(projectDir, packageName) {
  return join(projectDir, "node_modules", ...packageName.split("/"));
}

// The runtime packages a Solid 2 probe's environment is completed with beyond
// what the manifest row pinned.
//
// A Solid 2 application installs `solid-js` and `@solidjs/web` together --
// the DOM half moved out of `solid-js/web` into its own package -- and a
// package whose runtime imports `@solidjs/web` routinely declares only
// `solid-js` as a peer (`@tanstack/solid-query@6.0.0-rc.0` and its
// persist-client are the corpus cases). Installing the row's declared peers
// alone then leaves that import unresolved and the checker refuses the row
// with "`@solidjs/web` is not installed", which describes the benchmark's
// environment rather than the package. The completion is the same-version
// release, and only when the pinned manifest's release catalog lists it, so it
// never substitutes a version the discovery did not see. A Solid 1 probe is
// never completed: `solid-js/web` is part of `solid-js` there. The result
// records what was added under `runtimeCompletion`; `solid` stays the
// manifest's own pin.
export function solidRuntimeCompletion(row, probe, solidReleases = null) {
  if (row?.solidTarget !== "solid2") return {};
  const pinned = probe?.solid ?? {};
  const solidJs = pinned["solid-js"];
  if (typeof solidJs !== "string" || "@solidjs/web" in pinned) return {};
  const webReleases = solidReleases?.["@solidjs/web"]?.v2;
  if (!Array.isArray(webReleases) || !webReleases.includes(solidJs)) return {};
  return { "@solidjs/web": solidJs };
}

function buildSpecs(row, probe, completion = {}) {
  const specs = [`${row.package}@${row.version}`];
  for (const [name, version] of Object.entries({ ...(probe.solid ?? {}), ...completion })) {
    specs.push(`${name}@${version}`);
  }
  return specs;
}

// `expected` feeds `verifyInstall` from lib/install.mjs directly. Only the
// probed package itself carries an integrity pin in the manifest row; the
// Solid runtime packages are checked by version only (verifyInstall already
// skips the integrity comparison whenever `want.integrity` is falsy).
function buildExpectedVersions(row, probe, completion = {}) {
  const expected = { [row.package]: { version: row.version, integrity: row.integrity ?? null } };
  for (const [name, version] of Object.entries({ ...(probe.solid ?? {}), ...completion })) {
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
  probeIds = [],
  packages = [],
  includeSupplemental = false
} = {}) {
  const filters = [];
  if (sentinel) filters.push("sentinel");
  for (const family of [...families].sort()) filters.push(`family-${family}`);
  for (const target of [...solidTargets].sort()) filters.push(`solid${target}`);
  if (packages.length) {
    const digest = createHash("sha256").update([...packages].sort().join("\0")).digest("hex").slice(0, 12);
    filters.push(`packages-${digest}`);
  }
  if (probeIds.length) {
    const digest = createHash("sha256")
      .update([...probeIds].sort().join("\0"))
      .digest("hex")
      .slice(0, 12);
    filters.push(`probes-${digest}`);
  }
  return {
    kind: filters.length ? "filtered" : "full",
    sentinel,
    families: [...families].sort(),
    solidTargets: [...solidTargets].sort(),
    probeIds: [...probeIds].sort(),
    ...(packages.length ? { packages: [...packages].sort() } : {}),
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
  for (const name of scope.packages ?? []) filters.push(`package=${name}`);
  for (const family of scope.families ?? []) filters.push(`family=${family}`);
  for (const target of scope.solidTargets ?? []) filters.push(`solid${target}`);
  if (scope.probeIds?.length) filters.push(`${scope.probeIds.length} explicit probe(s)`);
  return filters.length ? filters.join(" ") : "filtered";
}

export function resolveProbeIdFilter({
  manifest,
  packages = [],
  families = [],
  solidTargets = [],
  sentinelIds = null,
  explicitProbeIds = []
}) {
  const noFilter =
    packages.length === 0 &&
    families.length === 0 &&
    solidTargets.length === 0 &&
    sentinelIds === null &&
    explicitProbeIds.length === 0;
  if (noFilter) return null;

  const normalizedTargets = solidTargets.map(target => (target === "1" ? "solid1" : target === "2" ? "solid2" : target));
  const sentinelSet = sentinelIds ? new Set(sentinelIds) : null;
  const explicitSet = explicitProbeIds.length ? new Set(explicitProbeIds) : null;

  const ids = [];
  // Resolving a filter over supplemental rows too is harmless -- it only maps
  // ids -- and it means an explicitly requested fork probe id still resolves
  // when the caller opted in with --include-supplemental.
  for (const row of [...(manifest?.rows ?? []), ...(manifest?.supplemental ?? [])]) {
    if (packages.length && !packages.includes(row.package)) continue;
    if (families.length && !families.includes(row.family)) continue;
    if (normalizedTargets.length && !normalizedTargets.includes(row.solidTarget)) continue;
    for (const probe of row.probes ?? []) {
      if (sentinelSet && !sentinelSet.has(probe.id)) continue;
      if (explicitSet && !explicitSet.has(probe.id)) continue;
      ids.push(probe.id);
    }
  }
  return ids;
}

export function unknownExplicitProbeIds(manifest, explicitProbeIds = []) {
  if (explicitProbeIds.length === 0) return [];
  const known = new Set();
  for (const row of [...(manifest?.rows ?? []), ...(manifest?.supplemental ?? [])]) {
    for (const probe of row.probes ?? []) known.add(probe.id);
  }
  return [...new Set(explicitProbeIds)].filter(id => !known.has(id)).sort();
}

// Concurrency-limited map that always writes into a pre-sized array by
// index, so results come back in `items` order no matter which worker
// finishes first -- completion order and report order are deliberately
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
// contract -- every declared entrypoint the generator reached is described by
// it. A contract with refused entrypoints is real output and is recorded as
// such, but it is its own outcome: folding it into `success` would let the
// corpus-wide rate read 100% while a third of the ecosystem's entrypoints went
// undescribed. Nothing here ever moves a probe the other way (a failure into
// a success), which is the rule the benchmark exists under -- it may only ever
// make its own rate stricter.
export function probeOutcome(className) {
  if (className === "success") return "success";
  if (className === "partial-success") return "partial-success";
  return "failure";
}

function buildResult({
  row,
  probe,
  // Runtime packages `solidRuntimeCompletion` added beyond the manifest pin;
  // `{}` for a row installed as pinned, and for an install that never ran.
  runtimeCompletion = {},
  installedVersions,
  integrityVerified,
  declaredEntrypoints,
  // Whether `declaredEntrypoints` is a pattern count rather than an entrypoint
  // count. Recorded per row because certification can happen in a later queue
  // pass, which then has only this result to read the manifest fact from.
  declaredWildcard = false,
  generatedEntrypoints,
  refusedEntrypoints = null,
  refusedArtifactCases = null,
  artifactCaseRefusals = null,
  inapplicableArtifactCases = null,
  artifactCaseInapplicabilities = null,
  declinedClosures = null,
  externalEdges = [],
  dependencyPlan = null,
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
  certificationAttempt = null,
  stdout,
  stderr
}) {
  const result = {
    probeId: probe.id,
    family: row.family,
    status: row.status,
    package: row.package,
    version: row.version,
    solidTarget: row.solidTarget,
    probeKind: probe.kind,
    channel: probe.channel,
    solid: probe.solid,
    // Runtime packages added beyond the manifest pin (see
    // `solidRuntimeCompletion`); `{}` when the pin was installed as is.
    runtimeCompletion,
    installedVersions,
    integrityVerified,
    declaredEntrypoints,
    declaredWildcard,
    generatedEntrypoints,
    refusedEntrypoints,
    refusedArtifactCases,
    artifactCaseRefusals,
    inapplicableArtifactCases,
    artifactCaseInapplicabilities,
    // The generator's declined `creates` closure proposals, as a row-level
    // count beside the artifact-case censuses. Additive and null-safe: `null`
    // is "no sidecar was read", which is not the measured zero a row that
    // declined nowhere reports. The blockers themselves stay on
    // `contractContent.dialectSilentBlockers`.
    declinedClosures,
    externalEdges,
    dependencyPlan,
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
  if (certificationAttempt !== null) result.certificationAttempt = certificationAttempt;
  return result;
}

// Which proposal lane this probe asks certification to use, from the
// generation result alone.
//
// A `success` generation describes every applicable artifact case, so reusing
// the emitted proposal is both cheaper and exactly equivalent -- there is no
// second lane to want.
//
// A `partial-success` splits by *why* the missing cases are missing. When at
// least one refused on dependency composition, those cases refused for want of
// an accepted contract for a dependency, which is precisely what the
// published-dependency-graph lane supplies: it acquires, generates and
// certifies the dependency, then certifies the refused roots against it. So the
// probe stops handing over its proposal (a reused proposal short-circuits the
// lane decision inside `contract certify`) and asks for the graph lane instead.
//
// Every other partial row keeps the reuse. Its refusals are facts about the
// publisher's own bytes -- a `.cjs` entrypoint with no declaration target, a
// `.d.ts` named as its own fact source, an entry file whose runtime and
// declaration export sets do not intersect -- and no dependency catalog moves
// them, so re-deciding the lane could only drop the cases that did generate.
//
// The two lanes are not ordered by coverage: the graph lane certifies exactly
// the cases the plain lane refused, and the reused proposal exactly the ones it
// generated. `certificationAttempt.coverage` is what makes that trade visible
// per row rather than hidden inside one word.
//
// Which of the two composing lanes it asks for is not a preference. The
// frontier-only graph lane publishes exactly the refused cases *instead of*
// the generated ones; entrypoint recovery prepares the union and publishes
// what proves, so it dominates whenever there is anything to retain. The
// frontier-only lane is therefore reached only when the row generated nothing
// to keep, or when `--dependency-graph-lane` asks for it by name.
//
// This is a policy over the row's own refusal census, not a reviewed list of
// probe ids: a row that needs an accepted contract for a dependency is exactly
// a row the composing lane is the answer for, wherever it appears in the
// corpus.
/// Whether a row's closure declines name a dependency the certifier would need
/// an accepted contract for. Read from the generation result's own decline
/// census, which is where a frontier shows up once the case itself resolves
/// and is therefore never refused.
function declinedDependencyFrontier(result) {
  const kinds = result?.contractContent?.declinedClosuresByKind;
  return Boolean(kinds) && Number(kinds["unaccepted-external-dependency"] ?? 0) > 0;
}

export function certificationLaneRequest(result, { frontierOnly = false } = {}) {
  if (result?.class !== "partial-success") {
    // A row can want an accepted dependency contract without refusing an
    // artifact case. `@solid-primitives/memo` is the worked example: one case,
    // none refused, class `success`, and all seven exports still unknown in
    // every domain behind 21 `unaccepted-external-dependency` declines. Its
    // frontier is a decline census, which `partialProposalHasDependencyFrontier`
    // cannot see because that reads artifact-case refusals.
    //
    // Routing it does not make the lane able to compose — certify publishes
    // refused cases and there are none — but it does make certify *say so*
    // instead of exiting silently, which is the whole difference between
    // § 26's unexplained no-op and an audit line naming the specifier.
    //
    // Only an explicit `--dependency-graph-lane` reaches this; the default
    // policy is untouched, because the frontier-only lane publishes refused
    // cases instead of generated ones and a measured corpus lost receipts to
    // that trade.
    if (frontierOnly && declinedDependencyFrontier(result)) {
      return { lane: "published-graph" };
    }
    return { lane: "reused-proposal" };
  }
  if (!partialProposalHasDependencyFrontier(result.artifactCaseRefusals)) {
    return { lane: "reused-proposal" };
  }
  if (frontierOnly) return { lane: "published-graph" };
  // The policy never selects the frontier-only lane. It publishes the refused
  // cases *instead of* the generated ones, and a measured corpus lost receipts
  // to exactly that trade; only an explicit `--dependency-graph-lane` accepts
  // it. A row whose generated entrypoint count could not be read has nothing
  // recovery can retain, so it keeps its reuse rather than risking the trade
  // on a number that is missing.
  return (result.generatedEntrypoints ?? 0) > 0
    ? { lane: "entrypoint-recovery" }
    : { lane: "reused-proposal" };
}

/// The lane that actually produced the proposal, read from the audit the
/// certification itself wrote -- never from what the runner asked for. The
/// request is a preference; `contract certify` decides, and a graph preparation
/// it could not complete falls back to the ordinary proposal.
export function certificationLaneOf(audit) {
  const preparation = audit?.graphPreparation;
  if (preparation?.reusedProposal === true) return "reused-proposal";
  if (preparation?.retainedProposalFallback === true) return "generated-proposal";
  if (preparation && typeof preparation === "object" && "rootCases" in preparation) {
    return "published-graph";
  }
  // No graph preparation and no reuse marker: certification generated the
  // proposal itself, in its own scratch. That is a third real lane, and
  // reporting it as either of the other two would be a fabrication.
  return audit ? "generated-proposal" : null;
}

export function readCertificationAttempt(
  result,
  auditPath,
  durationMs,
  {
    catalogPath = "",
    packageName = "",
    packageVersion = "",
    declaredEntrypoints = null,
    declaredWildcard = false,
    laneRequested = null
  } = {}
) {
  let audit = null;
  if (existsSync(auditPath)) {
    try {
      audit = JSON.parse(readFileSync(auditPath, "utf8"));
    } catch {
      audit = null;
    }
  }
  const lane = certificationLaneOf(audit);
  const coverage = readCertifiedCoverage({
    catalogPath,
    packageName,
    packageVersion,
    declaredEntrypoints,
    declaredWildcard
  });
  if (result.status === 0) {
    return {
      attempted: true,
      status: "certified",
      stage: "catalog-publication",
      owner: "configured-issuer",
      demandId: null,
      family: null,
      reason: null,
      lane,
      laneRequested,
      coverage,
      // Whether the memory watchdog killed this probe's tree is a resource
      // fact, not a package fact, and it previously reached the report only as
      // a marker appended to `reason`. Carry the flag itself so "this row is a
      // resource failure" is machine-queryable without matching that prose.
      memoryExceeded: result.memoryExceeded === true,
      durationMs,
      stageDurationsMs: audit?.stageDurationsMs ?? {},
      graphPreparation: audit?.graphPreparation ?? null,
      demandCountsByFamily: {},
      artifactSatisfiedDemandsByFamily: {},
      refusalCountsByFamily: {},
      refusalCountsByOwner: {},
      // How many closure candidates recipe-gated planning withheld by name --
      // a `creates` candidate with no recipe in the corpus is planned into
      // neither a demand nor a gate, and the row certifies with that domain
      // open. Read off the audit's `withheldClosures`; an older audit has none.
      withheldClosures: Array.isArray(audit?.withheldClosures) ? audit.withheldClosures.length : 0,
      withheldClosureReasons: withheldClosureReasons(audit),
      withheldClosureDetails: Array.isArray(audit?.withheldClosures) ? audit.withheldClosures : [],
      ordinaryAnalysis: audit?.ordinaryAnalysis ?? null
    };
  }
  const countBy = (items, key) => {
    const counts = {};
    for (const item of items) {
      const value = item?.[key];
      if (typeof value === "string" && value) counts[value] = (counts[value] ?? 0) + 1;
    }
    return Object.fromEntries(Object.entries(counts).sort(([left], [right]) => left.localeCompare(right)));
  };
  const demands = (audit?.demandPlans ?? []).flatMap(plan => plan.demands ?? []);
  const artifactSatisfied = demands.filter(demand => demand.satisfiedByArtifactSnapshot);
  const refusals = audit?.refusals ?? [];
  return {
    attempted: true,
    status: audit?.status === "refused" ? "refused" : "infrastructure-failure",
    lane,
    laneRequested,
    // A refused attempt publishes no catalog, so this is normally `null`. It is
    // still read rather than assumed: a refusal after publication would be a
    // fact worth seeing, and asserting `null` would hide it.
    coverage,
    stage: audit?.stage ?? (result.timedOut ? "timeout" : "orchestration"),
    owner: audit?.refusal?.owner ?? "orchestration",
    demandId: audit?.refusal?.demandId ?? null,
    family: audit?.refusal?.family ?? null,
    reason:
      audit?.refusal?.reason ??
      (result.timedOut
        ? "policy-2 certification attempt timed out"
        : result.stderr?.trim() || result.stdout?.trim() || `certification exited ${result.status}`),
    refusalCount: refusals.length,
    memoryExceeded: result.memoryExceeded === true,
    durationMs,
    stageDurationsMs: audit?.stageDurationsMs ?? {},
    graphPreparation: audit?.graphPreparation ?? null,
    demandCountsByFamily: countBy(demands, "family"),
    artifactSatisfiedDemandsByFamily: countBy(artifactSatisfied, "family"),
    refusalCountsByFamily: countBy(refusals, "family"),
    refusalCountsByOwner: countBy(refusals, "owner"),
    withheldClosures: Array.isArray(audit?.withheldClosures) ? audit.withheldClosures.length : 0,
    withheldClosureReasons: withheldClosureReasons(audit),
    withheldClosureDetails: Array.isArray(audit?.withheldClosures) ? audit.withheldClosures : []
  };
}

// A hook throwing (rather than resolving with a status/stderr shape) is
// still just this one probe's failure -- never rethrown, always folded into
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

async function runProbe(
  { row, probe },
  { timeoutMs, keepTemp, attemptCertification, projectLease = null, solidReleases = null },
  hooks
) {
  const now = hooks.now ?? Date.now;
  const overallStart = now();
  const runtimeCompletion = solidRuntimeCompletion(row, probe, solidReleases);
  const specs = buildSpecs(row, probe, runtimeCompletion);
  const expected = buildExpectedVersions(row, probe, runtimeCompletion);

  let project;
  try {
    project = await hooks.mkProject({ probeId: probe.id, row, probe });
  } catch (error) {
    return buildInfraFailureResult({ row, probe, error, phase: "install", durationMs: now() - overallStart });
  }

  const { projectDir, outputDir } = project;
  if (projectLease) projectLease.project = project;

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
    const declaredCensus = readDeclaredEntrypointCensus(packageJsonPath);
    const declaredEntrypoints = declaredCensus?.count ?? null;
    const declaredWildcard = declaredCensus?.wildcard ?? false;

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
        declaredWildcard,
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
      genResult = await hooks.generateContract({
        packageRoot,
        outputPath,
        timeoutMs,
        integrity: row.integrity,
        entrypoints: probe.entrypoints ?? []
      });
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
    const refusalAudit = readProposalRefusalAudit(outputPath);
    const externalEdges = collectExternalEdges({
      texts: [
        genResult.stderr ?? "",
        genResult.stdout ?? "",
        ...(refusalAudit?.refusals ?? []).map(refusal => refusal.reason)
      ],
      projectDir,
      packageRoot
    });
    const dependencyCases = (refusalAudit?.refusals ?? []).filter(refusal =>
      isDependencyCompositionRefusalText(refusal.reason)
    );
    let dependencyPlan = null;
    if (externalEdges.length > 0 && dependencyCases.length > 0) {
      const planner = hooks.planDependencies ?? planRecursiveDependencies;
      try {
        dependencyPlan = planner({
          projectDir,
          rootPackageRoot: packageRoot,
          rootPackage: row.package,
          rootVersion: row.version,
          rootIntegrity: row.integrity,
          artifactCases: dependencyCases
        });
      } catch (error) {
        dependencyPlan = {
          schemaVersion: 1,
          rootIdentity: { package: row.package, version: row.version, integrity: row.integrity },
          status: "planner-failure",
          complete: false,
          roots: [],
          nodes: [],
          edges: [],
          cycles: [],
          leaves: [{ kind: "planner-failure", reason: error?.message ?? String(error) }],
          graphDigest: null
        };
      }
    }
    const contractContent = producedContract
      ? readContractContent(outputPath, genClass.detail?.refusedEntrypoints ?? 0)
      : null;

    let certificationAttempt = null;
    if (attemptCertification && genClass.class === "success") {
      const certificationStart = now();
      const auditPath = `${outputPath}.certification-audit.json`;
      const catalogPath = `${outputPath}.accepted-catalog`;
      // A `success` generation is never routed to the graph lane, so this arm
      // always reuses. `certifyCompleteProbe` is where the split happens.
      const { lane } = certificationLaneRequest({ class: genClass.class });
      let certificationResult;
      try {
        const proposalRefusalAudit = `${outputPath}.refusals.json`;
        certificationResult = await hooks.attemptCertification({
          packageRoot,
          catalogPath,
          auditPath,
          timeoutMs,
          integrity: row.integrity,
          entrypoints: probe.entrypoints ?? [],
          proposalRefusalAudit: existsSync(proposalRefusalAudit)
            ? proposalRefusalAudit
            : "",
          // The generation phase emitted this probe's proposal under the
          // certification importer; hand it over with its sidecars so
          // certification verifies it instead of regenerating it. Certify
          // itself decides whether the hand-over is admissible.
          proposal: existsSync(`${outputPath}.certification-inputs.json`) ? outputPath : ""
        });
      } catch (error) {
        certificationResult = {
          status: 1,
          stdout: "",
          stderr: error?.stack ?? String(error),
          timedOut: false
        };
      }
      certificationAttempt = readCertificationAttempt(
        certificationResult,
        auditPath,
        now() - certificationStart,
        {
          catalogPath,
          packageName: row.package,
          // The row's pinned version, which `verifyInstall` has already bound
          // to the archive this probe generated from -- this arm only runs
          // after integrity verification. It is the identity half of the
          // catalog filter, not a label: the graph lane can publish *another
          // version of this same package* into the same catalog.
          packageVersion: row.version,
          declaredEntrypoints,
          declaredWildcard,
          laneRequested: lane
        }
      );
    }

    return buildResult({
      row,
      probe,
      runtimeCompletion,
      installedVersions,
      integrityVerified: true,
      declaredEntrypoints,
      declaredWildcard,
      generatedEntrypoints,
      refusedEntrypoints:
        genClass.detail?.refusalUnit === "entrypoint"
          ? genClass.detail.refusedCases
          : null,
      refusedArtifactCases:
        genClass.detail?.refusalUnit === "artifact-case"
          ? genClass.detail.refusedCases
          : refusalAudit?.refusals.length ?? null,
      artifactCaseRefusals: refusalAudit?.refusals ?? null,
      inapplicableArtifactCases: refusalAudit?.inapplicable.length ?? null,
      artifactCaseInapplicabilities: refusalAudit?.inapplicable ?? null,
      declinedClosures: refusalAudit?.declinedClosures.length ?? null,
      externalEdges,
      dependencyPlan,
      checklistItems,
      contractContent,
      outcome: probeOutcome(genClass.class),
      classification: genClass,
      exitStatus: genResult.status ?? null,
      timedOut: genResult.timedOut ?? false,
      durationMs: now() - overallStart,
      installDurationMs,
      generationDurationMs,
      certificationAttempt,
      stdout: genResult.stdout ?? "",
      stderr: genResult.stderr ?? ""
    });
  } finally {
    // "unless --keep-temp": the decision lives here, in the run core, rather
    // than inside the hook, so a test can prove cleanup was skipped entirely
    // without needing a hook that behaves differently per flag.
    if (!keepTemp && !projectLease) {
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

async function certifyCompleteProbe(
  item,
  { timeoutMs, keepTemp, dependencyGraphLane = false, recoverEntrypoints = false, recoverProbeIds = [] },
  hooks
) {
  const certificationStart = hooks.now?.() ?? Date.now();
  let project = item.project ?? null;
  let auditPath = "";
  let catalogPath = "";
  let laneRequested = null;
  let certificationResult;
  try {
    let installationVerified = project !== null;
    if (!project) {
      project = await hooks.mkProject({
        probeId: item.result.probeId,
        row: item.task.row,
        probe: item.task.probe,
        phase: "certification"
      });
      const specs = buildSpecs(item.task.row, item.task.probe);
      const expected = buildExpectedVersions(item.task.row, item.task.probe);
      let installResult;
      try {
        installResult = await hooks.installPackages({
          projectDir: project.projectDir,
          specs,
          expected,
          timeoutMs
        });
      } catch (error) {
        installResult = {
          status: 1,
          stdout: "",
          stderr: error?.stack ?? String(error),
          timedOut: false,
          installedVersions: {},
          integrity: {}
        };
      }
      const verification =
        installResult.status === 0
          ? verifyInstall({
              expected,
              versions: installResult.installedVersions ?? {},
              integrity: installResult.integrity ?? {}
            })
          : { ok: false, problems: [] };
      installationVerified = installResult.status === 0 && verification.ok;
      if (!installationVerified) {
        certificationResult = {
          status: 1,
          stdout: installResult.stdout ?? "",
          stderr:
            installResult.stderr?.trim() ||
            verification.problems
              .map(problem => `${problem.kind}: ${problem.package}`)
              .join("; ") ||
            "certification reinstall verification failed",
          timedOut: installResult.timedOut ?? false
        };
      }
    }
    if (installationVerified) {
      const outputPath = join(
        project.outputDir,
        `${sanitizeProbeId(item.result.probeId)}.json`
      );
      auditPath = `${outputPath}.certification-audit.json`;
      catalogPath = `${outputPath}.accepted-catalog`;
      const packageRoot = packageInstallPath(
        project.projectDir,
        item.task.row.package
      );
      const proposalRefusalAudit = `${outputPath}.refusals.json`;
      laneRequested = (recoverEntrypoints || recoverProbeIds.includes(item.task.probe.id))
        ? "entrypoint-recovery"
        : certificationLaneRequest(item.result, { frontierOnly: dependencyGraphLane }).lane;
      try {
        certificationResult = await hooks.attemptCertification({
          packageRoot,
          catalogPath,
          auditPath,
          timeoutMs,
          integrity: item.task.row.integrity,
          entrypoints: item.task.probe.entrypoints ?? [],
          proposalRefusalAudit: existsSync(proposalRefusalAudit)
            ? proposalRefusalAudit
            : "",
          // The generation phase emitted this probe's proposal under the
          // certification importer; hand it over with its sidecars so
          // certification verifies it instead of regenerating it. Certify
          // itself decides whether the hand-over is admissible.
          //
          // Withheld for a `published-graph` request: a reused proposal
          // short-circuits the lane decision inside `contract certify`, so
          // handing it over would answer the request by ignoring it.
          proposal:
            laneRequested !== "published-graph" &&
            existsSync(`${outputPath}.certification-inputs.json`)
              ? outputPath
              : "",
          dependencyGraphLane: laneRequested === "published-graph",
          recoverEntrypoints: laneRequested === "entrypoint-recovery"
        });
      } catch (error) {
        certificationResult = {
          status: 1,
          stdout: "",
          stderr: error?.stack ?? String(error),
          timedOut: false
        };
      }
    }
  } catch (error) {
    certificationResult = {
      status: 1,
      stdout: "",
      stderr: error?.stack ?? String(error),
      timedOut: false
    };
  }
  // A certified row keeps no stderr, so the checker's `SOLID_CHECKER_TIMINGS`
  // lines (forwarded by the CLI) would vanish with it; echo them to the
  // runner's own stderr when timings were asked for.
  if (process.env.SOLID_CHECKER_TIMINGS && certificationResult?.stderr) {
    process.stderr.write(certificationResult.stderr);
  }
  try {
    const durationMs = (hooks.now?.() ?? Date.now()) - certificationStart;
    item.result.certificationAttempt = readCertificationAttempt(
      certificationResult,
      auditPath,
      durationMs,
      {
        catalogPath,
        packageName: item.task.row.package,
        packageVersion: item.task.row.version,
        declaredEntrypoints: item.result.declaredEntrypoints ?? null,
        declaredWildcard: item.result.declaredWildcard === true,
        laneRequested
      }
    );
    item.result.durationMs += durationMs;
  } finally {
    if (project && !keepTemp) {
      try {
        await hooks.cleanup(project);
      } catch {
        // Cleanup cannot rewrite already measured semantic results or replace
        // a malformed certification result with a cleanup failure.
      }
    }
  }
}

// The injectable core. Takes a validated manifest, an optional explicit
// probe-id filter (`null` means every probe in the manifest), run options,
// and the four side-effecting hooks plus a clock. Returns results in
// deterministic manifest order regardless of completion order, and never
// rejects because one probe's hooks threw -- see `runProbe`.
export async function runBenchmark({ manifest, probeIds = null, options = {}, hooks }) {
  const timeoutMs = (options.timeoutMs ?? DEFAULT_TIMEOUT_SECONDS * 1000) | 0;
  const concurrency = options.concurrency ?? DEFAULT_CONCURRENCY;
  const keepTemp = options.keepTemp ?? false;
  const attemptCertification = options.attemptCertification ?? false;
  // Off by default, and deliberately so: measured on the 21 partial rows of
  // the 2026-09-03 corpus the routing loses more receipts than it gains (six
  // rows go certified -> refused, two gain a certified root), so the canonical
  // report must not take it silently. See docs/ecosystem-benchmark.md.
  const dependencyGraphLane = options.dependencyGraphLane ?? false;
  const recoverEntrypoints = options.recoverEntrypoints ?? false;
  const recoverProbeIds = options.recoverProbeIds ?? [];
  const certificationConcurrency =
    options.certificationConcurrency ?? DEFAULT_CERTIFICATION_CONCURRENCY;

  if (attemptCertification && typeof hooks.attemptCertification !== "function") {
    throw new TypeError("attemptCertification requires an attemptCertification hook");
  }

  const idFilter = probeIds ? new Set(probeIds) : null;
  const tasks = collectProbeTasks(manifest, idFilter, {
    includeSupplemental: Boolean(options.includeSupplemental)
  });
  const scheduleCosts = options.scheduleCosts ?? {};
  const scheduled = tasks
    .map((task, manifestIndex) => ({
      task,
      manifestIndex,
      cost: Number(scheduleCosts[task.probe.id]) || 0
    }))
    .sort((left, right) => right.cost - left.cost || left.manifestIndex - right.manifestIndex);

  if (scheduled.length === 0) return [];

  // Generation and certification share one host-bounded worker pool. When the
  // certification bound is wider, its extra slots never displace the measured
  // install-safe generation width; after proposal work drains, every slot may
  // certify. Smaller explicit bounds retain the old shared-pool interleave.
  const executed = [];
  const certificationQueue = [];
  const waiting = [];
  let nextGeneration = 0;
  let remainingGenerations = scheduled.length;
  let activeGenerations = 0;
  let activeCertifications = 0;
  const generationWorkers = Math.max(1, Math.min(concurrency || 1, scheduled.length));
  const certificationLimit = Math.max(1, certificationConcurrency || 1);
  const dedicatedCertificationSlots = Math.max(
    0,
    certificationLimit - generationWorkers
  );
  const workerCount = Math.max(
    generationWorkers,
    certificationLimit,
    generationWorkers + dedicatedCertificationSlots
  );
  const interleavedCertificationLimit = dedicatedCertificationSlots > 0
    ? dedicatedCertificationSlots
    : Math.min(
        certificationLimit,
        Math.max(1, Math.floor(generationWorkers / 4))
      );
  const interleavedGenerationLimit = Math.max(
    1,
    generationWorkers - interleavedCertificationLimit
  );
  const wakeWorkers = () => {
    for (const wake of waiting.splice(0)) wake();
  };
  const waitForWork = () => new Promise(resolve => waiting.push(resolve));
  const takeWork = () => {
    if (
      dedicatedCertificationSlots === 0 &&
      attemptCertification &&
      certificationQueue.length > 0 &&
      activeCertifications < interleavedCertificationLimit &&
      activeGenerations >= interleavedGenerationLimit
    ) {
      activeCertifications += 1;
      return { kind: "certification", item: certificationQueue.shift() };
    }
    if (nextGeneration < scheduled.length && activeGenerations < generationWorkers) {
      activeGenerations += 1;
      return { kind: "generation", scheduledTask: scheduled[nextGeneration++] };
    }
    if (
      dedicatedCertificationSlots > 0 &&
      attemptCertification &&
      nextGeneration < scheduled.length &&
      certificationQueue.length > 0 &&
      activeCertifications < interleavedCertificationLimit
    ) {
      activeCertifications += 1;
      return { kind: "certification", item: certificationQueue.shift() };
    }
    if (
      attemptCertification &&
      nextGeneration >= scheduled.length &&
      certificationQueue.length > 0 &&
      activeCertifications < Math.min(
        certificationLimit,
        Math.max(1, workerCount - activeGenerations)
      )
    ) {
      activeCertifications += 1;
      return { kind: "certification", item: certificationQueue.shift() };
    }
    if (remainingGenerations === 0 && certificationQueue.length === 0) return null;
    return undefined;
  };

  await Promise.all(
    Array.from({ length: workerCount }, async () => {
      while (true) {
        const work = takeWork();
        if (work === null) return;
        if (work === undefined) {
          await waitForWork();
          continue;
        }
        if (work.kind === "certification") {
          try {
            await certifyCompleteProbe(
              work.item,
              { timeoutMs, keepTemp, dependencyGraphLane, recoverEntrypoints, recoverProbeIds },
              hooks
            );
          } finally {
            activeCertifications -= 1;
            wakeWorkers();
          }
          continue;
        }

        const projectLease = {};
        let result;
        let generationCompleted = false;
        try {
          result = await runProbe(
            work.scheduledTask.task,
            {
              timeoutMs,
              keepTemp,
              attemptCertification: false,
              projectLease,
              solidReleases: manifest.solidReleases ?? null
            },
            hooks
          );
          generationCompleted = true;
        } finally {
          activeGenerations -= 1;
          if (!generationCompleted) {
            remainingGenerations -= 1;
            if (projectLease.project && !keepTemp) {
              try {
                await hooks.cleanup(projectLease.project);
              } catch {
                // Cleanup must not replace the harness failure that interrupted
                // generation after ownership of the project was transferred.
              }
            }
            wakeWorkers();
          }
        }
        if (keepTemp && projectLease.project) {
          result.retainedArtifacts = {
            projectDir: projectLease.project.projectDir,
            outputDir: projectLease.project.outputDir
          };
        }
        const item = {
          manifestIndex: work.scheduledTask.manifestIndex,
          task: work.scheduledTask.task,
          result,
          project: projectLease.project ?? null
        };
        executed.push(item);
        remainingGenerations -= 1;
        if (
          attemptCertification &&
          (result.class === "success" ||
            ((recoverEntrypoints || recoverProbeIds.includes(item.task.probe.id)) &&
              result.class === "partial-success" && result.generatedEntrypoints > 0) ||
            // A row whose refusal census names a dependency frontier is a row a
            // composing lane can answer. Leaving it out of the queue would
            // decide that on scheduling grounds before any evidence is asked
            // for.
            certificationLaneRequest(result, { frontierOnly: dependencyGraphLane }).lane !==
              "reused-proposal" ||
            (result.dependencyPlan?.complete === true &&
              (result.dependencyPlan?.roots?.length ?? 0) > 0))
        ) {
          certificationQueue.push(item);
        } else if (item.project && !keepTemp) {
          try {
            await hooks.cleanup(item.project);
          } catch {
            // Cleanup cannot rewrite the already measured probe result.
          }
        }
        wakeWorkers();
      }
    })
  );

  return executed
    .sort((left, right) => left.manifestIndex - right.manifestIndex)
    .map(item => item.result);
}

// ---------------------------------------------------------------------------
// CLI wrapper: argv parsing, resolving inputs, building real hooks, writing
// reports. Everything above this line is pure or hook-driven and exercised
// by run.test.mjs without touching npm, the network, or the real checker.
// ---------------------------------------------------------------------------

export function reportForPersistence(report, includeGraph = false) {
  if (includeGraph) return report;
  return {
    ...report,
    results: report.results.map(result => {
      if (!result.dependencyPlan || typeof result.dependencyPlan !== "object") return result;
      const { nodes, edges, ...plan } = result.dependencyPlan;
      return { ...result, dependencyPlan: plan };
    })
  };
}

function usage() {
  return `Usage: bun scripts/ecosystem-benchmark/run.mjs [options]

  --manifest <FILE>      default scripts/ecosystem-benchmark/manifest.json
  --sentinel             run only the pinned sentinel subset
  --family <ID>          restrict to one family (repeatable)
  --solid <1|2>          restrict to one Solid target (repeatable)
  --probe <ID>           run one exact probe id (repeatable)
  --package <NAME>       run all probes of an exact package name (repeatable)
  --include-graph        retain dependency graph nodes and edges in JSON
  --json <FILE>          default benchmarks/ecosystem/report<-scope>.json
  --markdown <FILE>      default benchmarks/ecosystem/report<-scope>.md
                         Only an unfiltered run defaults to the canonical
                         report.json/report.md; --sentinel, --family and
                         --solid each derive their own name so a subset can
                         never overwrite the full-corpus artifact.
  --baseline <FILE>      compare against a pinned previous run
  --thresholds <FILE>    threshold mode: exit 1 when a threshold regresses
  --timeout <SECONDS>    per-probe generation timeout, default 300
  --concurrency <N>      default caps install/generation probes at 8,
                         currently ${DEFAULT_CONCURRENCY}
  --certification-concurrency <N>
                         separate certification pool, currently
                         ${DEFAULT_CERTIFICATION_CONCURRENCY}
  --registry-cache <DIR> content-addressed store for registry bytes shared by
                         every certification child; default
                         SOLID_CHECKER_REGISTRY_CACHE, else
                         rust/target/registry-cache
  --no-registry-cache    fetch every registry byte fresh, as a certification
                         run outside this harness does
  --durable-writes       flush every published catalog to stable storage as a
                         certification outside this harness does; default off,
                         since the run's catalogs are scratch
  --no-install-lockfile-cache
                         resolve every install against the registry instead of
                         installing a previous run's exact resolution frozen
                         (default rust/target/install-locks)
  --no-materialized-store
                         write every compiler-source package into each private
                         Type Facts project instead of linking it from the
                         shared store (default rust/target/materialized-store)
  --attempt-certification
                         attempt policy-2 certification for every structurally
                         complete proposal and retain its exact first refusal
  --dependency-graph-lane
                         route a partial proposal whose refusal census names a
                         dependency-composition case through the
                         published-dependency-graph lane instead of reusing the
                         emitted proposal. The two lanes cover different
                         artifact-case sets, so this is a choice, not an
                         improvement: measured on the 2026-09-03 corpus it
                         turns six certified rows into refusals and gives two
                         rows a certified root. Off by default
  --recover-entrypoints  re-certify generated and refused dependency cases
                         together; isolate proof-refused value-only cases for
                         new publications. Preserve existing catalogs. Opt-in
  --recover-probe <ID>   enable recovery only for this exact probe (repeatable),
                         without filtering other probes from a full measurement
  --probe-recipe-corpus <DIR>
                         hand-authored, claim-addressed runtime-probe recipes
                         to certify closed claim domains against. A proposal
                         that closes a domain schedules a mandatory
                         contradiction veto, and without a corpus that gate
                         refuses rather than certifying an unvetoed closure --
                         which is what every row in the current corpus does,
                         since none of them has a recipe yet
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
    probeIds: [],
    packages: [],
    includeGraph: false,
    // Left null until the scope is known: the default path depends on which
    // subset the run covers. An explicit flag sets it and wins.
    json: null,
    markdown: null,
    baseline: null,
    thresholds: null,
    timeoutSeconds: DEFAULT_TIMEOUT_SECONDS,
    concurrency: DEFAULT_CONCURRENCY,
    certificationConcurrency: DEFAULT_CERTIFICATION_CONCURRENCY,
    registryCache: null,
    noRegistryCache: false,
    durableWrites: false,
    installLockfileCache: true,
    materializedStore: true,
    attemptCertification: false,
    dependencyGraphLane: false,
    recoverEntrypoints: false,
    recoverProbeIds: [],
    probeRecipeCorpus: null,
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
      case "--probe":
        options.probeIds.push(takeValue(argv, index++, arg));
        break;
      case "--package":
        options.packages.push(takeValue(argv, index++, arg));
        break;
      case "--include-graph":
        options.includeGraph = true;
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
      case "--certification-concurrency":
        options.certificationConcurrency = Number(takeValue(argv, index++, arg));
        break;
      case "--registry-cache":
        options.registryCache = takeValue(argv, index++, arg);
        break;
      case "--no-registry-cache":
        options.noRegistryCache = true;
        break;
      case "--durable-writes":
        options.durableWrites = true;
        break;
      case "--no-install-lockfile-cache":
        options.installLockfileCache = false;
        break;
      case "--no-materialized-store":
        options.materializedStore = false;
        break;
      case "--attempt-certification":
        options.attemptCertification = true;
        break;
      case "--dependency-graph-lane":
        options.dependencyGraphLane = true;
        break;
      case "--recover-entrypoints":
        options.recoverEntrypoints = true;
        break;
      case "--recover-probe":
        options.recoverProbeIds.push(takeValue(argv, index++, arg));
        break;
      case "--probe-recipe-corpus":
        options.probeRecipeCorpus = takeValue(argv, index++, arg);
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

function historicalScheduleCosts(path = DEFAULT_SCHEDULE_REPORT) {
  if (!existsSync(path)) return {};
  try {
    const report = readJsonFile(path);
    if (!Array.isArray(report?.results)) return {};
    return Object.fromEntries(
      report.results
        .filter(result =>
          typeof result?.probeId === "string" &&
          (Number.isFinite(result.generationDurationMs) || Number.isFinite(result.durationMs))
        )
        .map(result => [
          result.probeId,
          Number.isFinite(result.durationMs) && result.durationMs > 0
            ? result.durationMs
            : Math.max(0, result.generationDurationMs ?? 0)
        ])
    );
  } catch {
    // Historical costs affect execution order only. A missing or pre-schema
    // report falls back to manifest order; it never changes benchmark data.
    return {};
  }
}

function readSentinelIds(path) {
  const parsed = readJsonFile(path);
  if (parsed?.schemaVersion !== 1 || !Array.isArray(parsed.probes)) {
    throw new Error(`${path} is not a valid sentinel document (schemaVersion 1 with a "probes" array)`);
  }
  return parsed.probes;
}

// A probe child (bun/JSC) has no conservative heap ceiling of its own, and
// certification's process tree now includes the checker plus a Type Facts
// producer type-checking a materialized dependency closure -- so a pathological
// probe must be turned into one failed row here, never a host memory
// exhaustion. Polls the child's whole process tree RSS and SIGKILLs it at the
// cap. Resource fail-closed: exceeding the ceiling reads as that probe's
// failure with an explicit stderr marker and a `memoryExceeded` flag on the
// row, exactly like a timeout.
//
// 4 GiB stays the default after the retention repair: the measured worst
// process-tree peak across the previously-killed heavy tail is 762 MiB, so the
// ceiling is already ~5x it -- a guard, not a budget. Lowering it towards the
// measured peak would start deciding rows rather than catching pathologies,
// and the whole ecosystem manifest is far wider than the probes measured here.
const PROBE_MEMORY_CAP_MB = (() => {
  const raw = Number(process.env.SOLID_CHECKER_PROBE_MEMORY_MB);
  return Number.isInteger(raw) && raw > 0 ? raw : 4096;
})();
const PROBE_MEMORY_POLL_MS = 2000;

function sampleProcessTreeRssKb(rootPid) {
  try {
    const table = execFileSync("ps", ["-ax", "-o", "pid=,ppid=,rss="], {
      encoding: "utf8",
      maxBuffer: 8 * 1024 * 1024
    });
    const children = new Map();
    const rss = new Map();
    for (const line of table.split("\n")) {
      const parts = line.trim().split(/\s+/);
      if (parts.length !== 3) continue;
      const pid = Number(parts[0]);
      const ppid = Number(parts[1]);
      const kb = Number(parts[2]);
      if (!Number.isInteger(pid)) continue;
      rss.set(pid, Number.isFinite(kb) ? kb : 0);
      if (Number.isInteger(ppid)) {
        if (!children.has(ppid)) children.set(ppid, []);
        children.get(ppid).push(pid);
      }
    }
    let total = 0;
    const queue = [rootPid];
    const seen = new Set();
    while (queue.length) {
      const pid = queue.pop();
      if (seen.has(pid)) continue;
      seen.add(pid);
      total += rss.get(pid) ?? 0;
      for (const child of children.get(pid) ?? []) queue.push(child);
    }
    return total;
  } catch {
    return 0;
  }
}

function superviseChildMemory(child, capMb = PROBE_MEMORY_CAP_MB) {
  const state = { exceeded: false };
  const interval = setInterval(() => {
    if (child.exitCode !== null || child.signalCode !== null) return;
    const kb = sampleProcessTreeRssKb(child.pid);
    if (kb > capMb * 1024) {
      state.exceeded = true;
      clearInterval(interval);
      child.kill("SIGKILL");
    }
  }, PROBE_MEMORY_POLL_MS);
  interval.unref?.();
  return {
    stop: () => clearInterval(interval),
    exceeded: () => state.exceeded,
    marker: () =>
      state.exceeded
        ? `\n[solid-checker-ecosystem-benchmark: probe process tree exceeded the ${capMb} MiB memory ceiling and was killed]`
        : ""
  };
}

// Real, side-effecting hooks: exactly the four `runBenchmark` needs, backed
// by lib/install.mjs (for npm) and a spawned CLI subprocess (for
// generation) -- never a `require`/`import` of anything under an installed
// package's own node_modules tree.
function buildRealHooks({
  nativeBin,
  typeFactsBin,
  certificationInnerConcurrency,
  registryCache = null,
  maxWorkers = 1,
  durableWrites = false,
  installLockfileCache = null,
  materializedStore = null,
  probeRecipeCorpus = null
}) {
  // Generation and certification run inside a pool of long-lived CLI workers
  // (lib/cli-worker.mjs) instead of one CLI process per probe and phase. The
  // worker reproduces the CLI's stdout, stderr and exit status exactly; the
  // pool keeps the per-request timeout and memory supervision, killing the
  // worker that serves a request exactly where the CLI child was killed
  // before. The variables below are the ones the CLI reads at call time; the
  // native binaries are fixed for the run and inherited by every worker.
  const pool = createCliWorkerPool({
    maxWorkers,
    environment: {
      ...process.env,
      SOLID_CHECKER_NATIVE_BIN: nativeBin,
      SOLID_TYPEFACTS_BIN: typeFactsBin,
      // Every catalog a certification publishes here lives in a temporary
      // directory the probe removes seconds later, so the full-disk flushes
      // that make a real publication crash-durable buy nothing and cost the
      // run about ten F_FULLFSYNCs per certification. Atomic visibility is
      // unchanged; only crash durability is relaxed, and only when the run
      // asked for it. The product default flushes.
      SOLID_CHECKER_DURABLE_WRITES: durableWrites ? "full" : "none",
      // Empty, not absent, when disabled, for the same reason as the
      // registry cache.
      SOLID_CHECKER_MATERIALIZED_STORE: materializedStore ?? ""
    },
    supervise: superviseChildMemory
  });
  const generationEnvironment = {
    SOLID_CHECKER_ARTIFACT_ANALYSIS_BATCH_CONCURRENCY: null,
    SOLID_CHECKER_REGISTRY_CACHE: null
  };
  const certificationEnvironment = {
    SOLID_CHECKER_ARTIFACT_ANALYSIS_BATCH_CONCURRENCY: String(certificationInnerConcurrency),
    // Empty, not absent, when disabled: the child must not pick a cache up
    // from the inherited environment that this run decided against.
    SOLID_CHECKER_REGISTRY_CACHE: registryCache ?? ""
  };
  return {
    now: () => Date.now(),

    mkProject: async () => {
      const projectDir = await mkdtemp(join(tmpdir(), "solid-checker-ecosystem-"));
      const outputDir = await mkdtemp(join(tmpdir(), "solid-checker-ecosystem-out-"));
      return { projectDir, outputDir };
    },

    installPackages: async ({ projectDir, specs, expected, timeoutMs }) => {
      await createProject({ root: projectDir, specs });
      const result = await bunInstall({ projectDir, specs, timeoutMs, lockfileCache: installLockfileCache });
      const names = Object.keys(expected);
      const installedVersions = readInstalledVersions(projectDir, names);
      const integrity = readLockIntegrity(projectDir, names);
      return { ...result, installedVersions, integrity };
    },

    generateContract: ({ packageRoot, outputPath, timeoutMs, integrity, entrypoints = [] }) => {
      // Generate under the importer certification will use for this probe
      // (named by the package root and the catalog certification publishes
      // to), so the emitted proposal can be handed to `contract certify
      // --proposal` instead of being regenerated. The importer does not
      // change the emitted document, plan or refusal audit. Certification
      // resolves its package root through realpath and Rust binds the receipt
      // to that exact string, so generate under the same spelling.
      const generationRoot = realpathSync(packageRoot);
      const certificationImporter = certificationImporterPathFor({
        packageRoot: generationRoot,
        catalog: `${outputPath}.accepted-catalog`
      });
      return pool.run({
        kind: "generate",
        args: [
          "--package-root",
          generationRoot,
          "--integrity",
          integrity,
          "--output",
          outputPath,
          "--certification-importer",
          certificationImporter,
          ...entrypoints.flatMap(entrypoint => ["--entrypoint", entrypoint])
        ],
        env: generationEnvironment,
        timeoutMs
      });
    },

    attemptCertification: ({
      packageRoot,
      catalogPath,
      auditPath,
      timeoutMs,
      integrity,
      entrypoints = [],
      proposalRefusalAudit = "",
      proposal = "",
      dependencyGraphLane = false,
      recoverEntrypoints = false
    }) => {
      const authorityDir = `${catalogPath}.authority`;
      mkdirSync(authorityDir, { recursive: true });
      const issuerConfiguration = join(authorityDir, "issuer.json");
      const trustConfiguration = join(authorityDir, "trust.json");
      writeFileSync(issuerConfiguration, `${JSON.stringify({
        format: "solid-checker-policy2-issuer-configuration",
        issuerConfigurationVersion: 1,
        kind: "persistent-local",
        scope: `ecosystem-benchmark:${createHash("sha256").update(catalogPath).digest("hex")}`,
        seed: randomBytes(32).toString("base64"),
        revocationEpoch: 1
      })}\n`, { mode: 0o600 });
      return pool.run({
        kind: "certify",
        args: [
          "--package-root",
          packageRoot,
          "--integrity",
          integrity,
          "--catalog",
          catalogPath,
          "--issuer-configuration",
          issuerConfiguration,
          "--trust-configuration-output",
          trustConfiguration,
          "--audit-output",
          auditPath,
          ...(proposalRefusalAudit
            ? ["--proposal-refusal-audit", proposalRefusalAudit]
            : []),
          ...(proposal ? ["--proposal", proposal] : []),
          ...(dependencyGraphLane ? ["--dependency-graph-lane"] : []),
          ...(recoverEntrypoints ? ["--recover-entrypoints"] : []),
          // Only supplied when the run configured one. Absent, a plan that
          // proposes a closed claim domain refuses its mandatory veto instead
          // of certifying it unvetoed.
          ...(probeRecipeCorpus ? ["--probe-recipe-corpus", probeRecipeCorpus] : []),
          ...entrypoints.flatMap(entrypoint => ["--entrypoint", entrypoint])
        ],
        env: certificationEnvironment,
        timeoutMs
      });
    },

    cleanup: async ({ projectDir, outputDir }) => {
      await rm(projectDir, { recursive: true, force: true });
      await rm(outputDir, { recursive: true, force: true });
    },

    close: () => pool.close()
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

  const unknownProbeIds = unknownExplicitProbeIds(manifest, [...options.probeIds, ...options.recoverProbeIds]);
  if (unknownProbeIds.length) {
    fail(`unknown --probe id(s): ${unknownProbeIds.join(", ")}`);
    return;
  }

  let sentinelIds = null;
  const eligibleRows = [...manifest.rows, ...(options.includeSupplemental ? manifest.supplemental ?? [] : [])];
  const unknownPackages = options.packages.filter(name => !eligibleRows.some(row => row.package === name));
  if (unknownPackages.length) {
    fail(`unknown --package name(s): ${unknownPackages.join(", ")}`);
    return;
  }
  if (options.sentinel) {
    try {
      sentinelIds = readSentinelIds(DEFAULT_SENTINEL);
    } catch (error) {
      fail(`cannot read sentinel file ${DEFAULT_SENTINEL}: ${error.message}`);
      return;
    }
  }

  const scope = runScope({
    packages: options.packages,
    sentinel: options.sentinel,
    families: options.families,
    solidTargets: options.solidTargets,
    probeIds: options.probeIds,
    includeSupplemental: options.includeSupplemental
  });
  const defaults = defaultReportPaths(scope);
  options.json ??= defaults.json;
  options.markdown ??= defaults.markdown;

  const probeIds = resolveProbeIdFilter({
    manifest,
    packages: options.packages,
    families: options.families,
    solidTargets: options.solidTargets,
    sentinelIds,
    explicitProbeIds: options.probeIds
  });
  if (probeIds && !collectProbeTasks(manifest, new Set(probeIds), options).length) {
    fail("the requested filters selected no runnable probes");
    return;
  }

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
        JSON.stringify(baselineScope.solidTargets ?? []) === JSON.stringify(scope.solidTargets) &&
        JSON.stringify(baselineScope.probeIds ?? []) === JSON.stringify(scope.probeIds) &&
        JSON.stringify(baselineScope.packages ?? []) === JSON.stringify(scope.packages ?? []);
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

  const registryCache = options.attemptCertification
    ? resolveRegistryCache({
        option: options.registryCache,
        disabled: options.noRegistryCache
      })
    : null;
  const hooks = buildRealHooks({
    nativeBin: binaries.nativeBin,
    typeFactsBin: binaries.typeFactsBin,
    certificationInnerConcurrency: recommendedCertificationInnerConcurrency(
      options.certificationConcurrency
    ),
    registryCache,
    // The scheduler never runs more concurrent generation and certification
    // work than this, so the pool never queues.
    maxWorkers: Math.max(1, options.concurrency || 1, options.certificationConcurrency || 1),
    durableWrites: options.durableWrites,
    installLockfileCache: options.installLockfileCache ? DEFAULT_INSTALL_LOCKFILE_CACHE : null,
    materializedStore: options.materializedStore ? DEFAULT_MATERIALIZED_STORE : null,
    probeRecipeCorpus: options.probeRecipeCorpus
      ? resolve(options.probeRecipeCorpus)
      : null
  });
  const scheduleCosts = historicalScheduleCosts();

  const startedAt = new Date().toISOString();
  let results;
  const stopProgressHeartbeat = startProgressHeartbeat();
  try {
    results = await runBenchmark({
      manifest,
      probeIds,
      options: {
        timeoutMs: options.timeoutSeconds * 1000,
        concurrency: options.concurrency,
        certificationConcurrency: options.certificationConcurrency,
        attemptCertification: options.attemptCertification,
        dependencyGraphLane: options.dependencyGraphLane,
        recoverEntrypoints: options.recoverEntrypoints,
        recoverProbeIds: options.recoverProbeIds,
        keepTemp: options.keepTemp,
        includeSupplemental: options.includeSupplemental,
        scheduleCosts
      },
      hooks
    });
  } catch (error) {
    // runBenchmark itself is designed to never reject over a single probe's
    // behavior (see runProbe) -- reaching here means the harness itself broke.
    fail(`benchmark harness crashed: ${error?.stack ?? error}`);
    return;
  } finally {
    stopProgressHeartbeat();
    await hooks.close?.();
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
      // no other source of this data -- see lib/report.mjs's own comment on
      // this parameter.
      checker: {
        nativeBin: binaries.nativeBin,
        typeFactsBin: binaries.typeFactsBin,
        probeRecipeCorpus: options.probeRecipeCorpus ? resolve(options.probeRecipeCorpus) : null,
        entrypointRecovery: {
          allSelectedProbes: options.recoverEntrypoints,
          probeIds: [...new Set(options.recoverProbeIds)].sort()
        },
        registryCache,
        durableWrites: options.durableWrites,
        installLockfileCache: options.installLockfileCache ? DEFAULT_INSTALL_LOCKFILE_CACHE : null,
        materializedStore: options.materializedStore ? DEFAULT_MATERIALIZED_STORE : null
      },
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

  // The raw dependency-graph `nodes`/`edges` arrays dominate the persisted
  // report (tens of KB per probe) and no consumer reads them from disk: the
  // ledgers and gates read only `complete`/`status`/`roots`/`rootIdentity`/
  // `leaves`/`cycles`, and `graphDigest` already commits to the full graph.
  // Drop them by default (investigations opt in with --include-graph) so a re-measure diffs on semantics
  // rather than rewriting the whole graph. (The planner's own unit tests build
  // plans in-memory and are unaffected.)
  const persistedReport = reportForPersistence(report, options.includeGraph);

  try {
    mkdirSync(dirname(options.json), { recursive: true });
    mkdirSync(dirname(options.markdown), { recursive: true });
    writeFileSync(options.json, `${JSON.stringify(persistedReport, null, 2)}\n`, "utf8");
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
