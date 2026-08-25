#!/usr/bin/env node
// Machine-verification harness for the pinned ecosystem corpus: how many real
// packages verify end to end.
//
// For every manifest probe row it runs the full RFC 0002 pipeline against a
// throwaway install:
//
//   npm install --ignore-scripts  ->  contract generate
//     ->  contract probe --write  ->  contract verify
//
// and writes benchmarks/ecosystem/verification-report.{json,md}.
//
// Three things matter more here than convenience:
//
// - **`contract probe` executes package code.** That is the whole reason this
//   is a separate command from `contract generate`, whose stated design
//   property is that it imports nothing. Every install and every execution
//   here happens inside a temporary directory under the state directory, npm
//   runs with `--ignore-scripts` so no package lifecycle script ever executes,
//   and every probe runs under both a per-mode child timeout and a whole-phase
//   wall budget. Run it where you would run those packages' own test suites.
// - **It is deliberately not `run.mjs`.** That harness's checked-in reports
//   measure contract *generation*, and folding a verification measurement into
//   them would change what its numbers mean. This one only reads that
//   harness's manifest, install helpers, and failure classifier.
// - **A timeout is its own outcome, never a verification result.** A row whose
//   probe exceeded the wall budget is recorded as `probe-timeout` and is
//   counted as neither verified nor refused. Silently folding it either way
//   would be the one wrong answer this measurement could give.
//
// The run is resumable: every completed row is appended to a journal in the
// state directory, and a re-run skips what the journal already records. Pass
// `--aggregate-only` to rebuild the reports from an existing journal without
// re-running anything.

import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { appendFileSync, existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { PROBE_MODES } from "../../packages/cli/scripts/contract-probe-driver.mjs";
import { classifyResult } from "./lib/classify.mjs";
import { FAMILIES } from "./lib/families.mjs";
import {
  createProject,
  installPackages,
  readInstalledVersions,
  readLockIntegrity,
  verifyInstall
} from "./lib/install.mjs";
import { sortRows } from "./lib/manifest.mjs";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const MANIFEST = join(ROOT, "scripts/ecosystem-benchmark/manifest.json");
const CLI = join(ROOT, "packages/cli/bin/solid-checker.mjs");
const REPORT_DIR = join(ROOT, "benchmarks/ecosystem");

const DEFAULTS = {
  installTimeoutMs: 240_000,
  peerInstallTimeoutMs: 240_000,
  generateTimeoutMs: 120_000,
  // Per condition-mode child process. The driver restarts a worker after every
  // probe that threw, so this bounds one attempt, not the mode.
  probeModeTimeoutMs: 20_000,
  // The whole `contract probe` invocation, restarts included -- scaled per row
  // by `probeBudgetFor`, because a fixed budget is a budget for the median
  // package and a guaranteed timeout for the wide-surface ones.
  // Calibrated against the corpus rather than guessed. The cost of a row is
  // dominated by *worker restarts*, not by claim count alone -- a mode restarts
  // after every probe that throws, and a wide-surface package can spend
  // hundreds of processes -- so the per-claim increment has to cover a fresh
  // node process and its imports, not just one call. A first pass at
  // 60s + 150ms/claim still timed out four rows, one of which had not timed out
  // under the old flat budget at all.
  probeBudgetBaseMs: 90_000,
  probeBudgetPerClaimMs: 500,
  probeBudgetCapMs: 900_000,
  verifyTimeoutMs: 90_000,
  concurrency: 6
};

/// The packages that *are* the Solid runtime, and are therefore pinned by the
/// manifest rather than resolved from a peer range. A peer declaration naming
/// one of these is never installed from its range: the range would be free to
/// move the runtime the row is a measurement of.
const RUNTIME_PACKAGES = new Set(["solid-js", "@solidjs/web", "@solidjs/signals"]);

// ---------------------------------------------------------------------------
// Pure helpers (exported for direct unit testing).
// ---------------------------------------------------------------------------

/// The CLI's own sibling-path rule (contract-review-plan.mjs): the probe and
/// verify sidecars *replace* a trailing `.json`, they do not append to it.
export function siblingPath(output, suffix) {
  return output.toLowerCase().endsWith(".json") ? `${output.slice(0, -5)}${suffix}` : `${output}${suffix}`;
}

/// The Solid runtime a row is a measurement of, completed.
///
/// The manifest pins the runtime versions each probe row is meant to run
/// against, and for Solid 2 that runtime is *two* packages: `solid-js` and
/// `@solidjs/web`. Some rows pin both because the package declares both as
/// peers; a row whose package declares only `solid-js` got only `solid-js`,
/// and then every entrypoint reaching the DOM half of the runtime failed to
/// import -- 248 claims of the previous measurement, attributed to the
/// package.
///
/// Completion is deliberately narrow. The companion version is the *same*
/// version string as the pinned `solid-js`, and only when the manifest's own
/// release list for `@solidjs/web` contains it: a version this corpus never
/// audited is not substituted in to make a row work, and a 1.x row is never
/// given a 2.x companion. What was added is recorded per row.
export function runtimeSpecsFor({ probe, manifest }) {
  const pinned = { ...(probe.solid ?? {}) };
  const added = [];
  const solid = pinned["solid-js"];
  const major = solid ? Number(String(solid).split(".")[0]) : null;
  if (solid && major === 2 && !pinned["@solidjs/web"]) {
    const audited = manifest?.solidReleases?.["@solidjs/web"]?.v2 ?? [];
    if (audited.includes(solid)) {
      pinned["@solidjs/web"] = solid;
      added.push("@solidjs/web");
    }
  }
  return { pinned, added };
}

/// The peers the installed artifact itself declares, minus the ones the row
/// already pins.
///
/// Read from the *installed* `package.json` rather than the manifest row:
/// that file is the artifact under measurement, and a peer set derived from
/// anywhere else could describe a different release. Optional peers are left
/// out -- `peerDependenciesMeta.optional` is the package saying the peer is
/// not required to function, and installing it would change what the probe
/// observes on the strength of nothing.
///
/// A peer naming a runtime package is skipped with a reason rather than
/// silently: those are pinned, and letting a range like `>=1.9.7` resolve
/// would swap the runtime the row is about.
export function peerSpecsFor({ installedManifest, pinned }) {
  const specs = [];
  const skipped = [];
  const meta = installedManifest?.peerDependenciesMeta ?? {};
  for (const [name, range] of Object.entries(installedManifest?.peerDependencies ?? {})) {
    if (meta[name]?.optional) {
      skipped.push({ package: name, reason: "declared optional by the package" });
      continue;
    }
    if (Object.hasOwn(pinned ?? {}, name)) {
      skipped.push({ package: name, reason: "already pinned by the manifest row" });
      continue;
    }
    if (RUNTIME_PACKAGES.has(name)) {
      skipped.push({ package: name, reason: "a Solid runtime package the row does not pin" });
      continue;
    }
    if (typeof range !== "string" || !range.trim()) {
      skipped.push({ package: name, reason: "no usable version range" });
      continue;
    }
    specs.push({ package: name, range: range.trim() });
  }
  specs.sort((left, right) => left.package.localeCompare(right.package));
  skipped.sort((left, right) => left.package.localeCompare(right.package));
  return { specs, skipped };
}

/// The wall budget one row's `contract probe` gets.
///
/// A single fixed budget is a budget for the median package. `@kobalte/core`
/// plans two orders of magnitude more claims than a one-export primitive, and
/// under a flat 120s it timed out -- which is its own outcome class and
/// therefore a row the measurement can say nothing about at all. Scaling with
/// the planned claim count buys those rows proportional time without giving
/// every row the maximum, and the cap keeps one pathological package from
/// holding a worker for the length of the run.
///
/// A timeout is still a timeout. This changes how many rows hit one, never
/// what hitting one means.
export function probeBudgetFor({ claims, base, perClaim, cap }) {
  if (!Number.isFinite(claims) || claims <= 0) return base;
  return Math.min(cap, base + Math.round(claims * perClaim));
}

/// A probe failure, reduced to the shape a maintainer acts on.
///
/// A failure is the strongest thing this measurement produces: the package
/// answered a claim the contract makes differently, which is a generator bug or
/// a package change and never an environment gap. Grouping by
/// `claim -> observed` is what turns 353 individual lines into "the generator
/// says `tracked` and the package does `deferred`, 41 times".
export function probeFailureShape({ claim, observed, reason }) {
  const claimText = String(claim ?? "");
  const field = claimText.replace(/\[\d+\]/, "[n]").split("=")[0] || "claim";
  const claimed = claimText.includes("=") ? claimText.slice(claimText.indexOf("=") + 1) : "?";
  const saw =
    observed ??
    /runtime kind is (\S+)/.exec(String(reason ?? ""))?.[1] ??
    /^the call returned a (\S+)/.exec(String(reason ?? ""))?.[1] ??
    "not observed";
  return `${field}: claimed ${claimed}, observed ${saw}`;
}

/// The blocker taxonomy of RFC 0002 §3 (`BLOCKERS` in
/// packages/cli/scripts/contract-verification.mjs).
///
/// `contract verify` now writes a refusal sidecar carrying `blockers.raised`
/// verbatim, and the harness reads that in preference to stderr. This
/// classifier still exists because the lines are the same either way, and
/// because a journal from an older run has only the stderr heads. The refusal
/// text embeds an absolute contract path, so a distinguishing clause can sit
/// far into the line; each rule below matches on the earliest marker that is
/// unambiguous.
export function blockerClass(line) {
  if (line.startsWith("no probe report at")) return "probe-report-present";
  if (/records \d+ passed claim/.test(line)) return "probe-report-includes-evidence-write";
  if (line.includes("but no evidence write")) return "probe-report-includes-evidence-write";
  if (line.includes("was written for contract bytes")) return "probe-report-binds-contract";
  if (line.includes("re-probe these exact bytes")) return "probe-report-binds-contract";
  if (line.includes("and the contract describes")) return "probe-report-binds-contract";
  if (line.includes("regenerate the contract, re-probe it")) return "probe-report-binds-contract";
  if (line.includes("discovery")) return "probe-report-includes-discovery";
  if (line.startsWith("a probe failed")) return "probe-failed";
  if (line.startsWith("an incompleteness finding")) return "incompleteness";
  // Before the `closure-note` rule and matching a phrase of its own: the two
  // sentences are one word apart ("carries an *attested* closure note"), they
  // block for different reasons, and merging them would make the effect of
  // attestation on this corpus unmeasurable -- which is the whole reason the
  // generator emits them on separate fields.
  if (line.includes("carries an attested closure note")) return "attested-closure-note";
  if (line.includes("carries a closure note")) return "closure-note";
  if (line.includes("no passing kind observation")) return "kind-observed";
  // The floor under amendment A9's per-entrypoint refusal: a document that would
  // certify nothing, with no `kind` refusal behind it (zero entrypoints, or every
  // entrypoint carrying an empty export map). Unreachable from a generated draft
  // today, and named rather than left to `unclassified-refusal` so that if it
  // ever does appear the measurement says what it is.
  if (line.startsWith("no entrypoint certifies anything")) return "certifies-nothing";
  if (line.includes("review decision") || line.includes("a promotion to")) return "review-under-way";
  if (line.includes("does not validate")) return "document-validates";
  return "unclassified-refusal";
}

/// One root cause per refusal, in the order a reader has to resolve them.
///
/// `probe-report-includes-evidence-write` is deliberately last: it is raised
/// *because* `contract probe --write` declined to write after a failure or an
/// incompleteness finding, so on almost every row it is a consequence of
/// another blocker rather than an independent cause. A row where it stands
/// alone is a different and worth-naming shape, which is why it stays in the
/// list rather than being dropped.
export const ROOT_CAUSE_ORDER = [
  "probe-failed",
  "incompleteness",
  "kind-observed",
  "certifies-nothing",
  "closure-note",
  // After `closure-note`: a row carrying both is a row whose record is not
  // established at all, and that is the cause a reader has to resolve first.
  "attested-closure-note",
  "probe-report-binds-contract",
  "probe-report-includes-discovery",
  "document-validates",
  "review-under-way",
  "probe-report-present",
  "probe-report-includes-evidence-write",
  "unclassified-refusal"
];

export function rootCause(classes) {
  for (const name of ROOT_CAUSE_ORDER) if (classes.has(name)) return name;
  return "unclassified-refusal";
}

/// Why a claim went undriven. The driver's reasons are free text carrying
/// package-specific detail -- a thrown message, an absolute path -- so grouping
/// strips the detail to make a distribution readable. The raw reason survives
/// per row in the journal and in the JSON report.
///
/// **Every reason the pipeline can emit has to land in a named bucket.** The
/// rules below are total over three tables -- `UNDRIVABLE` and `OUTCOME_REASON`
/// and `EXECUTION_UNATTRIBUTABLE` in
/// packages/cli/scripts/contract-probe-driver.mjs -- plus the session-death
/// shapes packages/cli/scripts/probe-contract.mjs writes and the two fallbacks
/// `settleClaims` uses, and verify-corpus.test.mjs asserts that totality
/// against the driver's own tables rather than a copied list. `other` is a
/// catch-all, and a catch-all that grows is a measurement that stops saying
/// anything: an unclassified bucket of 834 claims is exactly what made RFC 0002
/// amendment A9's stage 2 undecidable, because the split between "the probe
/// observed the export is absent" and "the session died" was inside it.
///
/// Two rules are deliberately shaped as families rather than exact strings --
/// `the probe process …` and `spawnSync …` -- so that a reworded session
/// failure lands in a *named* bucket instead of `other`. Failing into a name is
/// the safe direction here; the exact rules above them keep the distinctions
/// the design actually reads.
export function undrivenBucket(reason) {
  const text = String(reason);
  if (text === "(no reason recorded)") return "no reason recorded";
  if (text.startsWith("reactive reads are proven from compiler facts")) return "no probe form: reactiveReads";
  if (text.startsWith("owner requirements are proven")) return "no probe form: ownerRequirements";
  if (text.startsWith("an identity claim about a parameter")) return "no probe form: parameter identity";
  if (text.startsWith("callback argument descriptors have no probe form"))
    return "no probe form: callback arguments";
  if (text.startsWith("callback owner rows have no probe form")) return "no probe form: callback owner";
  if (text.startsWith("writeProbeEvidence does not descend into return leaves"))
    return "no probe form: nested return leaf";
  if (text.startsWith("asyncBehavior has no")) return "no probe form: asyncBehavior";
  if (text.startsWith("no generic store-path observation")) return "no probe form: store path";
  if (text.startsWith("no plantable reactive source")) return "no plantable reactive source";
  if (text.startsWith("the synthesized call completed without invoking the callback"))
    return "synthesized call did not invoke the callback";
  // The ways a driven callback observation names no execution mode, plus the one
  // way a malformed one does. Every reason here is a string
  // `EXECUTION_UNATTRIBUTABLE` owns; they are matched by their opening clause
  // because that clause is the observation and the rest is the explanation.
  if (text.startsWith("the callback re-ran across a settle interval in which nothing was written"))
    return "callback re-ran with nothing written";
  if (text.startsWith("the callback had not run by the time of the write"))
    return "callback first ran after the write";
  if (text.startsWith("the callback ran more times than the call site re-invoked the export"))
    return "callback ran more often than the call site";
  if (text.startsWith("the reactive runtime this observation was made in re-runs nothing"))
    return "runtime re-runs nothing in this mode";
  if (text.startsWith("the observation reports no count for the settle interval"))
    return "observation reports no control interval";
  if (text.startsWith("the synthesized call threw")) return "synthesized call threw";
  if (text.startsWith("import of ")) return "entrypoint import threw";
  if (text.startsWith("the probe process exited") || text.startsWith("the probe process was killed"))
    return "probe session failed (process died)";
  if (/^spawnSync .*ETIMEDOUT/.test(text)) return "probe session hit the per-mode timeout";
  if (text.includes("no re-read followed the planted write")) return "planted write was never re-read";
  if (text.startsWith("the probe runtime reported no caching measurement"))
    return "no caching measurement for the returned value";
  if (text.startsWith("reading the returned value "))
    return "returned value indistinguishable from a forwarding closure";
  if (text.startsWith("the callback ran only once the returned accessor was read"))
    return "callback ownership ambiguous in the driver's read scope";
  if (text.startsWith("the probe process stopped before reaching this claim"))
    return "probe session stopped before this claim";
  if (text.startsWith("the probe process wrote no readable report")) return "probe session wrote no report";
  if (text.startsWith("the probe process was aborted by package code"))
    return "probe session aborted by package code";
  if (text.startsWith("no unambiguous summary")) return "no unambiguous summary for the mode";
  if (text === "no probe form") return "no probe form (unnamed)";
  if (text === "no mode was attempted") return "no mode was attempted";
  // The two family rules, last so every exact shape above keeps its own name.
  if (text.startsWith("the probe process")) return "probe session failed (other)";
  if (text.startsWith("spawnSync ")) return "probe session could not be spawned";
  // The one non-observation that is an *observation*: the namespace loaded and
  // the binding was not in it, so the export does not exist in the artifact that
  // mode resolves. RFC 0002 amendment A9 stage 2 turns on how large this bucket
  // is, which is why it gets a name of its own rather than sharing one with the
  // session failures.
  //
  // **Placed below every session rule, and anchored to the end of the string,
  // for one reason.** A session death forwards the child's stderr verbatim
  // (`${detail}: ${child.stderr}` in packages/cli/scripts/probe-contract.mjs),
  // and `'x' is not exported by y` is the canonical Rollup/bundler message a
  // dying package prints. A substring test above the session rules read that as
  // an observation of absence -- laundering the one class that must keep
  // blocking into the one class stage 2 may narrow away. The driver's own reason
  // always *ends* `" in this mode"`
  // (`OUTCOME_REASON["export-missing"]`), so the anchored form is exact and the
  // ordering is the belt to its braces.
  if (/ is not exported by .+ in this mode$/.test(text)) return "export-missing in this mode";
  if (/ is not callable, so no call could be synthesized$/.test(text))
    return "export is not callable";
  return "other";
}

export function emptyKindGaps() {
  return {
    claims: 0,
    modes: {},
    reasons: {},
    // Structurally separate, not merely separately *labelled*. See below.
    contradictions: { claims: 0, modes: {}, reasons: {} }
  };
}

/// Where a `kind` claim's missing observations went, per mode.
///
/// The undriven distribution above is over *every* claim, so it cannot answer
/// the one question RFC 0002 amendment A9 stage 2 is gated on: for the modes a
/// `kind` claim was not observed in, how many were observations of *absence*
/// (`export-missing`, sound to exclude from the stated modes) and how many were
/// gaps (an import that threw, a session that died, a mode never attempted,
/// which must keep blocking).
///
/// **A contradiction is counted in its own object, never in `claims`/`modes`.**
/// A9: *"a mode whose observation exists and disagreed is counted as a
/// contradiction, never as a gap: the two must never share a number."* Sharing
/// `claims` and `modes` and separating only `reasons` was that failure with a
/// label on it -- the markdown headings say "unobserved", and 53 contradicted
/// `kind` claims across 20 corpus rows would have been counted under them. So
/// `contradictions` is a sibling object and renders as its own section: one
/// claim can contribute a gap in one mode and a contradiction in another, and
/// both numbers stay true.
///
/// It reads the probe report's own per-mode observations, so a mode counted in
/// `claims`/`modes` is a mode the plan attempted and the report answered for.
/// Two non-observations A9's stage-0 table enumerates are *not* per-mode
/// observations at all, and each gets a labelled category rather than being
/// silently absent:
///
/// - **a mode this run never attempted.** `runModes` is the probe report's own
///   `modes` list, so the un-attempted set is `PROBE_MODES - runModes`: for a
///   corpus run that drives all four this is empty, and a `--modes` narrowing
///   makes it exactly the modes no claim in the row could have been observed
///   in. It cannot distinguish "not stated" from "not attempted" for a claim
///   whose entrypoint states fewer modes than the run drove, which is why it is
///   derived from the *run's* set and not from each claim's `attempted`; on a
///   narrowed run it therefore over-counts a browser-only entrypoint's `server`
///   mode, in the conservative direction.
/// - **a mode where no unambiguous summary resolves.** `buildProbePlan` creates
///   no `kind=` claim there at all -- it records a family-(C) `summary` claim
///   whose reason names the mode -- so those are read from that claim rather
///   than from a `kind` one, and counted as gaps, because the verifier refuses
///   the entrypoint for exactly those modes.
export function kindGapsFor(claims, { modes: runModes } = {}) {
  const gaps = emptyKindGaps();
  const attemptedByRun = new Set(runModes ?? PROBE_MODES.map(mode => mode.name));
  const neverAttempted = PROBE_MODES.map(mode => mode.name).filter(
    name => !attemptedByRun.has(name)
  );
  const count = (into, mode, key) => {
    into.modes[mode] = (into.modes[mode] ?? 0) + 1;
    into.reasons[key] = (into.reasons[key] ?? 0) + 1;
  };
  for (const claim of claims ?? []) {
    const text = String(claim.claim ?? "");
    // A mode in which no unambiguous summary resolves states no `kind` claim to
    // observe, and the plan records it against this synthetic claim instead.
    if (text === "summary") {
      const mode = /^no unambiguous summary in (\S+)/.exec(String(claim.reason ?? ""))?.[1];
      if (!mode) continue;
      gaps.claims += 1;
      count(gaps, mode, "no unambiguous summary resolves in the mode (no kind claim exists)");
      continue;
    }
    if (!text.startsWith("kind=")) continue;
    const passed = new Set(claim.modes?.passed ?? []);
    const unobserved = (claim.modes?.attempted ?? []).filter(mode => !passed.has(mode));
    if (!unobserved.length && !neverAttempted.length) continue;
    const byMode = new Map((claim.observations ?? []).map(entry => [entry.mode, entry]));
    let gapped = false;
    let contradicted = false;
    for (const mode of unobserved) {
      const observation = byMode.get(mode);
      if (observation && observation.status !== "undriven") {
        contradicted = true;
        count(
          gaps.contradictions,
          mode,
          `observed and did not pass (${observation.status ?? "no status"})`
        );
        continue;
      }
      gapped = true;
      count(
        gaps,
        mode,
        observation
          ? undrivenBucket(observation.reason ?? "(no reason recorded)")
          : "no observation recorded for the mode"
      );
    }
    for (const mode of neverAttempted) {
      gapped = true;
      count(gaps, mode, "the run never attempted this mode");
    }
    if (gapped) gaps.claims += 1;
    if (contradicted) gaps.contradictions.claims += 1;
  }
  return gaps;
}

export function probeErrorBucket(detail) {
  const text = String(detail ?? "");
  if (text.includes("no installed solid-js above")) return "no installed solid-js beside the package";
  if (text.includes("names no dialect this checker probes")) return "solid-js version names no dialect";
  if (text.includes("cannot find an installed")) return "package root not resolvable";
  if (text.includes("regenerate the contract for the installed release"))
    return "installed release differs from the contract";
  return "other";
}

/// Every export in a contract document, split into the two states a verified
/// document can leave one in: a claim the machine stands behind, and the honest
/// `{"status":"unknown"}` sentinel.
///
/// `expandContract` is required rather than optional: a document dedups
/// summaries into a `summaries` table and maps summary-id -> export *names*, so
/// counting off the raw document would count summary ids. A document that
/// cannot be expanded records the error rather than a row of zeroes.
export function classifyExports(rawDocument, expandContract) {
  const result = { exports: 0, unknownBearing: 0, entrypoints: 0, expandError: null };
  let document = null;
  try {
    document = expandContract(rawDocument);
  } catch (error) {
    result.expandError = String(error?.message ?? error).slice(0, 200);
    return result;
  }
  if (!document?.entrypoints) return result;
  const bearsUnknown = value => {
    if (Array.isArray(value)) return value.some(bearsUnknown);
    if (value && typeof value === "object") {
      if (value.status === "unknown" && Object.keys(value).length === 1) return true;
      return Object.values(value).some(bearsUnknown);
    }
    return false;
  };
  for (const entry of Object.values(document.entrypoints)) {
    result.entrypoints += 1;
    for (const summary of Object.values(entry.exports ?? {})) {
      result.exports += 1;
      if (typeof summary === "object" && bearsUnknown(summary)) result.unknownBearing += 1;
    }
  }
  return result;
}

export function notVerifiedLines(stderr) {
  return String(stderr)
    .split("\n")
    .filter(line => line.includes("solid-checker: not verified:"))
    .map(line => line.slice(line.indexOf("not verified:") + "not verified:".length).trim());
}

export function percentile(values, fraction) {
  if (!values.length) return null;
  const sorted = [...values].sort((left, right) => left - right);
  return sorted[Math.min(sorted.length - 1, Math.floor(fraction * (sorted.length - 1)))];
}

export function stats(values) {
  if (!values.length) return { count: 0, medianMs: null, p90Ms: null, maxMs: null, meanMs: null };
  const total = values.reduce((sum, value) => sum + value, 0);
  return {
    count: values.length,
    medianMs: percentile(values, 0.5),
    p90Ms: percentile(values, 0.9),
    maxMs: Math.max(...values),
    meanMs: Math.round(total / values.length)
  };
}

// ---------------------------------------------------------------------------
// Running one row
// ---------------------------------------------------------------------------

function run(command, args, { cwd, timeoutMs, env } = {}) {
  return new Promise(resolvePromise => {
    const child = spawn(command, args, {
      cwd,
      env: { ...process.env, ...env },
      stdio: ["ignore", "pipe", "pipe"]
    });
    let stdout = "";
    let stderr = "";
    let timedOut = false;
    let spawnError = "";
    child.on("error", error => {
      spawnError = `${error.code ?? "spawn error"}: ${error.message}`;
    });
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
      resolvePromise({
        status,
        stdout,
        stderr: spawnError ? `${stderr}\n${spawnError}` : stderr,
        timedOut
      });
    });
  });
}

function readJson(path) {
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch {
    return null;
  }
}

async function runRow({ row, probe }, context) {
  const {
    workDir,
    budgets,
    cliEnv,
    expandContract,
    buildProbePlan,
    manifest,
    environmentShim,
    peerInstall = true
  } = context;
  const started = Date.now();
  const record = {
    probeId: probe.id,
    package: row.package,
    version: row.version,
    family: row.family,
    solidTarget: row.solidTarget,
    probeKind: probe.kind,
    channel: probe.channel,
    solid: probe.solid,
    startedAt: new Date(started).toISOString()
  };

  const projectDir = await mkdtemp(join(workDir, "proj-"));
  const outputDir = await mkdtemp(join(workDir, "out-"));
  try {
    const runtime = peerInstall
      ? runtimeSpecsFor({ probe, manifest })
      : { pinned: { ...(probe.solid ?? {}) }, added: [] };
    const specs = [
      `${row.package}@${row.version}`,
      ...Object.entries(runtime.pinned).map(([name, version]) => `${name}@${version}`)
    ];
    const expected = {
      [row.package]: { version: row.version, integrity: row.integrity ?? null },
      ...Object.fromEntries(
        Object.entries(runtime.pinned).map(([name, version]) => [name, { version, integrity: null }])
      )
    };

    const installStart = Date.now();
    await createProject({ root: projectDir, specs });
    const installResult = await installPackages({
      projectDir,
      specs,
      timeoutMs: budgets.installTimeoutMs
    });
    const installedVersions = readInstalledVersions(projectDir, Object.keys(expected));
    const integrity = readLockIntegrity(projectDir, Object.keys(expected));
    record.installMs = Date.now() - installStart;
    record.install = {
      pinned: [...specs].sort(),
      runtimeCompleted: runtime.added,
      peers: [],
      peersSkipped: [],
      peerInstall: "none"
    };

    const installClass = classifyResult({
      status: installResult.status,
      stdout: installResult.stdout,
      stderr: installResult.stderr,
      timedOut: installResult.timedOut,
      phase: "install"
    });
    const verified =
      installClass.class === "success"
        ? verifyInstall({ expected, versions: installedVersions, integrity })
        : { ok: true, problems: [] };
    if (installClass.class !== "success" || !verified.ok) {
      record.stage = "install";
      record.outcome = "install-failure";
      record.installClass = installClass.class;
      record.detail = verified.ok
        ? installClass.signature
        : verified.problems.map(problem => `${problem.kind}:${problem.package}`).join("; ");
      record.totalMs = Date.now() - started;
      return record;
    }

    const packageRoot = join(projectDir, "node_modules", ...row.package.split("/"));

    // Phase two of the install: the peers the artifact on disk declares.
    //
    // Separate from the pinned install rather than folded into it, so a peer
    // range can never take part in resolving the versions the row is pinned to.
    // If the peer install moves a pin anyway, the row keeps the pinned-only
    // tree: the measurement is about those exact bytes.
    const peers = peerInstall
      ? peerSpecsFor({
          installedManifest: readJson(join(packageRoot, "package.json")),
          pinned: runtime.pinned
        })
      : { specs: [], skipped: [] };
    record.install.peersSkipped = peers.skipped;
    if (peers.specs.length) {
      const peerSpecs = peers.specs.map(peer => `${peer.package}@${peer.range}`);
      const peerResult = await installPackages({
        projectDir,
        specs: peerSpecs,
        timeoutMs: budgets.peerInstallTimeoutMs
      });
      const afterPeers = verifyInstall({
        expected,
        versions: readInstalledVersions(projectDir, Object.keys(expected)),
        integrity: readLockIntegrity(projectDir, Object.keys(expected))
      });
      if (peerResult.status !== 0 || peerResult.timedOut || !afterPeers.ok) {
        record.install.peerInstall = afterPeers.ok ? "failed" : "reverted-pin-moved";
        record.install.peerDetail = (peerResult.stderr || peerResult.stdout || "")
          .slice(0, 300)
          .trim();
        // Reinstalling the pinned specs restores the tree the pins describe.
        // A failure here is recorded and the row continues: what it then
        // probes is the same tree every previous measurement probed.
        await installPackages({ projectDir, specs, timeoutMs: budgets.installTimeoutMs });
      } else {
        record.install.peerInstall = "complete";
        record.install.peers = peerSpecs;
      }
    }

    // No Solid runtime anywhere above the package is its own outcome, not an
    // error and not a refusal. The manifest pins what each row runs against,
    // and for a handful of rows -- `@solidjs/signals`, which is the reactive
    // core itself -- it pins no `solid-js`. Choosing one would be this
    // harness inventing a runtime pairing the corpus deliberately did not
    // audit, so the row is recorded as unprobeable and counted separately.
    const runtimePresent = existsSync(join(projectDir, "node_modules", "solid-js", "package.json"));
    // The contract is written OUTSIDE the install tree, for the same reason
    // run.mjs does it: a report artifact living inside node_modules could be
    // mistaken for package content by the package's own tooling.
    const contractFile = join(outputDir, "solid-reactivity.json");

    const generateStart = Date.now();
    const generateResult = await run(
      process.execPath,
      [CLI, "contract", "generate", "--package-root", packageRoot, "--output", contractFile],
      { timeoutMs: budgets.generateTimeoutMs, env: cliEnv }
    );
    record.generateMs = Date.now() - generateStart;
    const generateClass = classifyResult({
      status: generateResult.status,
      stdout: generateResult.stdout,
      stderr: generateResult.stderr,
      timedOut: generateResult.timedOut,
      phase: "generate"
    });
    record.generateClass = generateClass.class;
    record.refusedEntrypoints = generateClass.detail?.refusedEntrypoints ?? 0;
    if (generateClass.class !== "success" && generateClass.class !== "partial-success") {
      record.stage = "generate";
      record.outcome = "generate-failure";
      record.detail = generateClass.signature;
      record.totalMs = Date.now() - started;
      return record;
    }
    const contractDocument = readJson(contractFile);
    record.generated = classifyExports(contractDocument, expandContract);

    if (!runtimePresent && row.package !== "solid-js") {
      record.stage = "probe";
      record.outcome = "no-runtime";
      record.detail =
        `the manifest pins ${JSON.stringify(probe.solid ?? {})} for this row and the package ` +
        "declares no peer that installs a solid-js, so no Solid release sits above it to settle a probe";
      record.totalMs = Date.now() - started;
      return record;
    }

    // The claim count the plan will produce, computed here rather than guessed
    // from the export count: it is the exact number the probe is about to
    // drive, and it is what the wall budget scales with.
    let plannedClaims = null;
    try {
      plannedClaims = buildProbePlan(expandContract(contractDocument)).claims.length;
    } catch {
      plannedClaims = null;
    }
    const probeWallBudgetMs =
      budgets.probeWallBudgetMs ??
      probeBudgetFor({
        claims: plannedClaims,
        base: budgets.probeBudgetBaseMs,
        perClaim: budgets.probeBudgetPerClaimMs,
        cap: budgets.probeBudgetCapMs
      });
    record.plannedClaims = plannedClaims;
    record.probeWallBudgetMs = probeWallBudgetMs;

    const probeStart = Date.now();
    const probeResult = await run(
      process.execPath,
      [
        CLI,
        "contract",
        "probe",
        contractFile,
        "--package-root",
        packageRoot,
        // Discovery -- planting a callback where the contract states none -- is
        // never disabled: it is the only automated check that can contradict a
        // negative claim, and `contract verify` refuses a report produced
        // without it outright.
        "--write",
        ...(environmentShim ? [] : ["--no-environment-shim"]),
        "--timeout",
        String(budgets.probeModeTimeoutMs)
      ],
      { timeoutMs: probeWallBudgetMs, env: cliEnv }
    );
    record.probeMs = Date.now() - probeStart;
    record.probeExit = probeResult.status;
    record.probeTimedOut = probeResult.timedOut;
    if (probeResult.timedOut) {
      record.stage = "probe";
      record.outcome = "probe-timeout";
      record.detail = `probe exceeded the ${probeWallBudgetMs}ms wall budget`;
      record.totalMs = Date.now() - started;
      return record;
    }

    const probeReport = readJson(siblingPath(contractFile, ".probe.json"));
    if (!probeReport) {
      record.stage = "probe";
      record.outcome = "probe-error";
      record.detail = (probeResult.stderr || probeResult.stdout).slice(0, 600).trim();
      record.totalMs = Date.now() - started;
      return record;
    }
    record.probe = {
      summary: probeReport.summary,
      modes: probeReport.modes,
      discovery: {
        enabled: probeReport.discovery?.enabled ?? false,
        parameters: (probeReport.discovery?.parameters ?? []).length
      },
      dialect: probeReport.identities?.dialect ?? null,
      runtime: probeReport.identities?.runtime ?? null,
      // What the worker faked before it imported anything, and how many
      // processes each mode cost. Both are new in the probe report and both
      // are things a reader of these numbers has to be able to see.
      environment: probeReport.environment ?? null,
      sessions: probeReport.sessions ?? null,
      markersWritten: probeReport.contract?.markersWritten ?? 0,
      markersSuperseded: probeReport.contract?.markersSuperseded ?? 0,
      wrote: probeReport.contract?.afterWrite !== undefined,
      incompleteness: (probeReport.incompleteness ?? []).map(finding => finding.text).slice(0, 20),
      stderrTail: probeResult.stderr.slice(-1500)
    };
    const undrivenReasons = {};
    const failedClaims = [];
    const failures = [];
    const claimFamilies = {};
    for (const claim of probeReport.claims ?? []) {
      const key = `${claim.family}:${claim.status}`;
      claimFamilies[key] = (claimFamilies[key] ?? 0) + 1;
      if (claim.status === "undriven") {
        const reason = claim.reason ?? "(no reason recorded)";
        undrivenReasons[reason] = (undrivenReasons[reason] ?? 0) + 1;
      } else if (claim.status === "failed") {
        failedClaims.push(`${claim.entrypoint}:${claim.export} ${claim.claim}: ${claim.reason}`);
        // Structured, not just a rendered sentence. A failure is a claim the
        // package answered differently, and the report has to be able to say
        // which claim, what was observed instead, and in which modes -- that
        // is the row a maintainer opens the package to.
        const observed = (claim.observations ?? []).find(entry => entry.status === "failed");
        failures.push({
          entrypoint: claim.entrypoint,
          export: claim.export,
          claim: claim.claim,
          observed: observed?.observed ?? null,
          modes: (claim.observations ?? [])
            .filter(entry => entry.status === "failed")
            .map(entry => entry.mode)
            .sort(),
          reason: claim.reason ?? null
        });
      }
    }
    record.probe.undrivenReasons = undrivenReasons;
    record.probe.claimFamilies = claimFamilies;
    // Why each unobserved `kind` mode was unobserved. Recorded per row because
    // the decision RFC 0002 amendment A9 defers to stage 2 -- exclude a mode the
    // probe observed the export absent in, keep blocking on every gap -- is a
    // per-(entrypoint, export, mode) decision, and the corpus-wide undriven
    // distribution cannot answer it.
    record.probe.kindGaps = kindGapsFor(probeReport.claims ?? [], {
      // The modes this run actually drove, so a narrowed run's never-attempted
      // modes are counted rather than invisible.
      modes: probeReport.modes
    });
    record.probe.failedClaims = failedClaims.slice(0, 10);
    // Every failure is kept: the whole point of the section it feeds is that
    // this class is about to be the dominant visible defect, and a capped list
    // could hide a shape entirely.
    record.probe.failures = failures;

    const verifyStart = Date.now();
    const verifyResult = await run(process.execPath, [CLI, "contract", "verify", contractFile], {
      timeoutMs: budgets.verifyTimeoutMs,
      env: cliEnv
    });
    record.verifyMs = Date.now() - verifyStart;
    record.verifyExit = verifyResult.status;
    record.verifyTimedOut = verifyResult.timedOut;

    const verifyReport = readJson(siblingPath(contractFile, ".verify.json"));
    record.final = classifyExports(readJson(contractFile), expandContract);

    if (verifyResult.timedOut) {
      record.stage = "verify";
      record.outcome = "verify-timeout";
    } else if (verifyReport && readJson(contractFile)?.evidence?.kind === "verified") {
      record.stage = "verify";
      record.outcome = "verified";
      record.verify = {
        summary: verifyReport.summary,
        // The entrypoints verification refused inside a document it still
        // promoted (RFC 0002 amendment A9 stage 1). A rising number here is the
        // *cost* of the promotion being made visible: those entrypoints are
        // absent from the contract, so a consumer importing one gets an explicit
        // uncertifiable result instead of a claim nothing observed.
        refusedEntrypoints: (verifyReport.refusedEntrypoints ?? []).map(refusal => ({
          entrypoint: refusal.entrypoint,
          blocker: refusal.blocker ?? null,
          exports: (refusal.exports ?? []).length
        })),
        conversions: (verifyReport.conversions ?? []).map(conversion => ({
          entrypoint: conversion.entrypoint,
          export: conversion.export,
          field: conversion.field,
          reason: conversion.claims?.[0]?.reason ?? null
        }))
      };
    } else {
      record.stage = "verify";
      record.outcome = "refused";
      // The sidecar first: `contract verify` now records its refusal as data,
      // so the blockers arrive verbatim instead of being recovered from
      // sentences on stderr. The stderr path stays as the fallback for a
      // refusal that never reached the write -- and it is what every journal
      // from before this change contains.
      const sidecarBlockers =
        verifyReport?.outcome === "refused" ? verifyReport.blockers?.raised ?? [] : null;
      record.refusalSource = sidecarBlockers ? "sidecar" : "stderr";
      const lines = sidecarBlockers?.length
        ? sidecarBlockers
        : notVerifiedLines(verifyResult.stderr);
      // Every refusal line is kept -- truncated to a head long enough to
      // classify -- because a capped list could hide a blocker class entirely.
      record.blockerCount = lines.length;
      record.blockerHeads = lines.slice(0, 400).map(line => line.slice(0, 260));
      record.blockers = lines.slice(0, 5);
      if (!lines.length) {
        record.detail = (verifyResult.stderr || verifyResult.stdout).slice(0, 600).trim();
        record.outcome = "verify-error";
      }
    }
    record.totalMs = Date.now() - started;
    return record;
  } catch (error) {
    record.stage = record.stage ?? "harness";
    record.outcome = "harness-error";
    record.detail = String(error?.stack ?? error).slice(0, 600);
    record.totalMs = Date.now() - started;
    return record;
  } finally {
    await rm(projectDir, { recursive: true, force: true }).catch(() => {});
    await rm(outputDir, { recursive: true, force: true }).catch(() => {});
  }
}

// ---------------------------------------------------------------------------
// Aggregation
// ---------------------------------------------------------------------------

function emptyAggregate() {
  return {
    rows: 0,
    outcomes: {},
    contractsGenerated: 0,
    verified: 0,
    refused: 0,
    claims: { total: 0, driven: 0, passed: 0, failed: 0, undriven: 0, incompleteness: 0 },
    undriven: {},
    kindGaps: { rows: 0, ...emptyKindGaps(), contradictions: { rows: 0, ...emptyKindGaps().contradictions } },
    failureShapes: {},
    install: { runtimeCompleted: 0, peerComplete: 0, peerFailed: 0, peersInstalled: 0 },
    environment: { rowsShimmed: 0, shimmedGlobals: {}, modesShimmed: {} },
    sessions: { started: 0, restarts: 0, failed: 0 },
    blockerRows: {},
    blockerLines: {},
    rootCauses: {},
    conversions: 0,
    conversionFields: {},
    // Stage 1 of amendment A9: entrypoints refused by *verification* inside a
    // document that was still promoted. Separate from `record.refusedEntrypoints`,
    // which counts what `contract generate` refused.
    verificationRefusedEntrypoints: 0,
    rowsWithAVerificationRefusedEntrypoint: 0,
    probedRowsKept: 0,
    rowsWithProbedEvidence: 0,
    droppedInferredMarkers: 0,
    staleProbedMarkers: 0,
    exports: {
      certifiedInVerified: 0,
      unknownInVerified: 0,
      refusedInVerified: 0,
      inUnverifiedContract: 0,
      draftUnknownInVerified: 0,
      draftExportsInVerified: 0
    },
    totals: []
  };
}

function accumulate(bucket, record) {
  if (!bucket) return;
  bucket.rows += 1;
  bucket.outcomes[record.outcome] = (bucket.outcomes[record.outcome] ?? 0) + 1;
  bucket.totals.push(record.totalMs);
  if (record.generated) bucket.contractsGenerated += 1;
  if ((record.install?.runtimeCompleted ?? []).length) bucket.install.runtimeCompleted += 1;
  if (record.install?.peerInstall === "complete") {
    bucket.install.peerComplete += 1;
    bucket.install.peersInstalled += (record.install.peers ?? []).length;
  }
  if (record.install?.peerInstall === "failed" || record.install?.peerInstall === "reverted-pin-moved") {
    bucket.install.peerFailed += 1;
  }
  const environment = record.probe?.environment;
  if (environment) {
    // Counted once per row, not once per mode session: a row that shimmed
    // `window` in three of its four modes faked one `window`, and reporting
    // three under a column headed "Rows" would be a wrong number.
    const namesOnThisRow = new Set();
    for (const [mode, entry] of Object.entries(environment.modes ?? {})) {
      const names = entry?.shimmed ?? [];
      if (!names.length) continue;
      bucket.environment.modesShimmed[mode] = (bucket.environment.modesShimmed[mode] ?? 0) + 1;
      for (const name of names) namesOnThisRow.add(name);
    }
    for (const name of namesOnThisRow) {
      bucket.environment.shimmedGlobals[name] = (bucket.environment.shimmedGlobals[name] ?? 0) + 1;
    }
    if (namesOnThisRow.size) bucket.environment.rowsShimmed += 1;
  }
  if (record.probe?.sessions) {
    bucket.sessions.started += record.probe.sessions.started ?? 0;
    bucket.sessions.restarts += record.probe.sessions.restarts ?? 0;
    bucket.sessions.failed += record.probe.sessions.failed ?? 0;
  }
  for (const failure of record.probe?.failures ?? []) {
    const shape = probeFailureShape(failure);
    bucket.failureShapes[shape] = (bucket.failureShapes[shape] ?? 0) + 1;
  }
  if (record.probe?.summary) {
    const summary = record.probe.summary;
    bucket.claims.total += summary.claims ?? 0;
    bucket.claims.driven += summary.driven ?? 0;
    bucket.claims.passed += summary.passed ?? 0;
    bucket.claims.failed += summary.failed ?? 0;
    bucket.claims.undriven += summary.undriven ?? 0;
    bucket.claims.incompleteness += summary.incompleteness ?? 0;
    for (const [reason, count] of Object.entries(record.probe.undrivenReasons ?? {})) {
      const key = undrivenBucket(reason);
      bucket.undriven[key] = (bucket.undriven[key] ?? 0) + count;
    }
  }
  const kindGaps = record.probe?.kindGaps;
  // Gaps and contradictions are accumulated into separate objects, and a row can
  // land in both. Never into one number: see `kindGapsFor`.
  const fold = (into, from) => {
    into.rows += 1;
    into.claims += from.claims ?? 0;
    for (const [mode, count] of Object.entries(from.modes ?? {})) {
      into.modes[mode] = (into.modes[mode] ?? 0) + count;
    }
    for (const [reason, count] of Object.entries(from.reasons ?? {})) {
      into.reasons[reason] = (into.reasons[reason] ?? 0) + count;
    }
  };
  if (kindGaps?.claims) fold(bucket.kindGaps, kindGaps);
  if (kindGaps?.contradictions?.claims) {
    fold(bucket.kindGaps.contradictions, kindGaps.contradictions);
  }
  if (record.outcome === "verified") {
    bucket.verified += 1;
    bucket.conversions += record.verify?.summary?.conversions ?? 0;
    bucket.probedRowsKept += record.verify?.summary?.probedRows ?? 0;
    if ((record.verify?.summary?.probedRows ?? 0) > 0) bucket.rowsWithProbedEvidence += 1;
    bucket.droppedInferredMarkers += record.verify?.summary?.droppedInferredMarkers ?? 0;
    bucket.staleProbedMarkers += record.verify?.summary?.staleProbedMarkers ?? 0;
    const refusedHere = (record.verify?.refusedEntrypoints ?? []).length;
    bucket.verificationRefusedEntrypoints += refusedHere;
    if (refusedHere) bucket.rowsWithAVerificationRefusedEntrypoint += 1;
    for (const conversion of record.verify?.conversions ?? []) {
      const field = conversion.field.split(".").pop();
      bucket.conversionFields[field] = (bucket.conversionFields[field] ?? 0) + 1;
    }
    bucket.exports.certifiedInVerified += (record.final?.exports ?? 0) - (record.final?.unknownBearing ?? 0);
    bucket.exports.unknownInVerified += record.final?.unknownBearing ?? 0;
    bucket.exports.draftUnknownInVerified += record.generated?.unknownBearing ?? 0;
    bucket.exports.draftExportsInVerified += record.generated?.exports ?? 0;
    // The exports a *verified* row's promotion left behind, because their
    // entrypoint was refused (RFC 0002 amendment A9 stage 1). Derived from the
    // two documents rather than from the refusal list, so it is exactly the
    // difference between the draft and the promoted bytes: `certifiedInVerified`
    // and `unknownInVerified` both count `record.final`, so without this state
    // these exports would be in none of the composite's states and stage 1 would
    // raise the certified *share* by removing exports from its denominator.
    bucket.exports.refusedInVerified += Math.max(
      0,
      (record.generated?.exports ?? 0) - (record.final?.exports ?? 0)
    );
  } else if (record.generated) {
    bucket.exports.inUnverifiedContract += record.final?.exports ?? record.generated?.exports ?? 0;
  }
  if (record.outcome === "refused" || record.outcome === "verify-error") {
    bucket.refused += 1;
    const classes = new Set((record.blockerHeads ?? []).map(blockerClass));
    for (const name of classes) bucket.blockerRows[name] = (bucket.blockerRows[name] ?? 0) + 1;
    for (const line of record.blockerHeads ?? []) {
      const name = blockerClass(line);
      bucket.blockerLines[name] = (bucket.blockerLines[name] ?? 0) + 1;
    }
    const cause = rootCause(classes);
    bucket.rootCauses[cause] = (bucket.rootCauses[cause] ?? 0) + 1;
  }
}

function finish(bucket) {
  const { totals, ...rest } = bucket;
  return { ...rest, wallMs: stats(totals) };
}

export function buildVerificationReport({ records, manifest, budgets, checker }) {
  const overall = emptyAggregate();
  const byFamily = new Map(FAMILIES.map(family => [family.id, emptyAggregate()]));
  const bySolidTarget = new Map([
    ["solid1", emptyAggregate()],
    ["solid2", emptyAggregate()]
  ]);
  const probeErrors = {};
  const installFailures = [];
  const generateFailures = [];
  const timeouts = [];
  const noRuntime = [];

  for (const record of records) {
    accumulate(overall, record);
    accumulate(byFamily.get(record.family), record);
    accumulate(bySolidTarget.get(record.solidTarget), record);
    if (record.outcome === "probe-error") {
      const bucket = probeErrorBucket(record.detail);
      probeErrors[bucket] = (probeErrors[bucket] ?? 0) + 1;
    }
    if (record.outcome === "install-failure") installFailures.push(record.probeId);
    if (record.outcome === "generate-failure")
      generateFailures.push({ probeId: record.probeId, class: record.generateClass });
    if (record.outcome === "probe-timeout" || record.outcome === "verify-timeout")
      timeouts.push({
        probeId: record.probeId,
        outcome: record.outcome,
        totalMs: record.totalMs,
        plannedClaims: record.plannedClaims ?? null,
        budgetMs: record.probeWallBudgetMs ?? null
      });
    if (record.outcome === "no-runtime")
      noRuntime.push({ probeId: record.probeId, detail: record.detail ?? null });
  }

  // The failures a maintainer acts on, grouped by shape and then named
  // individually. This is deliberately the most legible section of the report:
  // a probe failure is the only outcome here that asserts something is wrong
  // with a package or with the generator, as opposed to something the machine
  // could not reach.
  const failureShapes = {};
  const failureRows = [];
  for (const record of records) {
    for (const failure of record.probe?.failures ?? []) {
      const shape = probeFailureShape(failure);
      failureShapes[shape] = (failureShapes[shape] ?? 0) + 1;
      failureRows.push({
        probeId: record.probeId,
        family: record.family,
        entrypoint: failure.entrypoint,
        export: failure.export,
        claim: failure.claim,
        observed: failure.observed ?? null,
        modes: failure.modes ?? [],
        shape,
        reason: failure.reason ?? null
      });
    }
  }
  failureRows.sort(
    (left, right) => left.shape.localeCompare(right.shape) || left.probeId.localeCompare(right.probeId)
  );

  // Why an entrypoint could not be imported at all -- the single largest
  // undriven cause, and a fact about the environment the probe worker runs in
  // as much as about the package.
  const importThrows = {};
  const importThrowRows = new Set();
  for (const record of records) {
    for (const [reason, count] of Object.entries(record.probe?.undrivenReasons ?? {})) {
      if (!reason.startsWith("import of ")) continue;
      importThrowRows.add(record.probeId);
      const match = /threw: ([A-Za-z]+(?: \[[A-Z_]+\])?): ?(.*)/.exec(reason);
      const key = (match ? `${match[1]}: ${match[2].slice(0, 70)}` : reason.slice(0, 80)).replace(
        /\/(?:private\/)?tmp\/\S+/g,
        "<path>"
      );
      importThrows[key] = (importThrows[key] ?? 0) + count;
    }
  }

  const phaseWallMs = {
    install: stats(records.map(record => record.installMs).filter(Number.isFinite)),
    generate: stats(records.map(record => record.generateMs).filter(Number.isFinite)),
    probe: stats(records.map(record => record.probeMs).filter(Number.isFinite)),
    verify: stats(records.map(record => record.verifyMs).filter(Number.isFinite)),
    pipelineWithoutInstall: stats(
      records
        .map(record =>
          Number.isFinite(record.generateMs)
            ? (record.generateMs ?? 0) + (record.probeMs ?? 0) + (record.verifyMs ?? 0)
            : null
        )
        .filter(Number.isFinite)
    ),
    total: stats(records.map(record => record.totalMs).filter(Number.isFinite))
  };

  const startedAt = records.map(record => record.startedAt).filter(Boolean).sort()[0] ?? null;
  const finishedAt = records.map(record => record.finishedAt).filter(Boolean).sort().at(-1) ?? null;

  const refusals = records
    .filter(record => record.outcome === "refused" || record.outcome === "verify-error")
    .map(record => {
      const classes = new Set((record.blockerHeads ?? []).map(blockerClass));
      return {
        probeId: record.probeId,
        package: record.package,
        version: record.version,
        family: record.family,
        solidTarget: record.solidTarget,
        blockerCount: record.blockerCount ?? 0,
        blockerClasses: [...classes].sort(),
        rootCause: rootCause(classes),
        firstBlocker: record.blockers?.[0] ?? record.detail ?? null,
        claims: record.probe?.summary ?? null,
        exports: record.final?.exports ?? null,
        // Present on every refusal, not only the `kind-observed` ones: a row
        // root-caused elsewhere can still carry a kind gap, and amendment A9's
        // 29 co-blocked rows are exactly the ones that must stay refused.
        kindGaps: record.probe?.kindGaps ?? null
      };
    })
    .sort((left, right) => left.probeId.localeCompare(right.probeId));

  const verified = records
    .filter(record => record.outcome === "verified")
    .map(record => ({
      probeId: record.probeId,
      package: record.package,
      version: record.version,
      family: record.family,
      solidTarget: record.solidTarget,
      exports: record.final?.exports ?? 0,
      exportsUnknown: record.final?.unknownBearing ?? 0,
      exportsUnknownAtGeneration: record.generated?.unknownBearing ?? 0,
      conversions: record.verify?.summary?.conversions ?? 0,
      probedRowsKept: record.verify?.summary?.probedRows ?? 0,
      // Named individually rather than counted: "this package verified, minus
      // ./server" is the finding, and a bare count would hide which subpath a
      // consumer now gets an uncertifiable result for.
      refusedEntrypoints: (record.verify?.refusedEntrypoints ?? []).map(
        refusal => refusal.entrypoint
      ),
      claims: record.probe?.summary ?? null,
      totalMs: record.totalMs
    }))
    .sort((left, right) => left.probeId.localeCompare(right.probeId));

  return {
    schemaVersion: 1,
    kind: "ecosystem-machine-verification",
    aggregatedAt: new Date().toISOString(),
    startedAt,
    finishedAt,
    safety: {
      executesPackageCode: true,
      note:
        "`contract probe` imports and runs each installed package's code in child processes. Every " +
        "install and every execution in this run happened inside temporary directories under the " +
        "harness state directory, npm ran with --ignore-scripts so no package lifecycle script " +
        "executed, and each probe process had both a per-mode timeout and a whole-phase wall budget."
    },
    pipeline: [
      "npm install --ignore-scripts",
      "contract generate",
      "contract probe --write",
      "contract verify"
    ],
    budgets,
    checker,
    corpus: {
      manifest: "scripts/ecosystem-benchmark/manifest.json",
      manifestGeneratedAt: manifest.generatedAt,
      rows: manifest.rows.length,
      probesInManifest: manifest.rows.reduce((total, row) => total + (row.probes?.length ?? 0), 0),
      probesRun: records.length,
      scope: "full corpus (every official row's every probe; supplemental fork rows excluded)"
    },
    overall: finish(overall),
    byFamily: Object.fromEntries(
      FAMILIES.map(family => [family.id, { label: family.label, ...finish(byFamily.get(family.id)) }])
    ),
    bySolidTarget: Object.fromEntries([...bySolidTarget].map(([name, bucket]) => [name, finish(bucket)])),
    phaseWallMs,
    probeEnvironment: {
      rowsWithAnEntrypointImportThrow: importThrowRows.size,
      importThrows,
      // What the probe worker faked, and where. Reported next to the import
      // throws because the two are the same subject: the throws are what the
      // environment could not supply, and the shim is what it did.
      shim: {
        rowsShimmed: overall.environment.rowsShimmed,
        shimmedGlobals: overall.environment.shimmedGlobals,
        modesShimmed: overall.environment.modesShimmed,
        note:
          "A claim observed in a mode whose worker faked these globals is an observation against a " +
          "fake DOM, which is a weaker fact than an observation in a browser. Every probe report and " +
          "every verify sidecar records the per-mode list; server-mode sessions are never shimmed."
      },
      // Session accounting: how many worker processes the corpus cost and how
      // many of those were restarts after a probe threw.
      sessions: overall.sessions
    },
    installEnvironment: {
      ...overall.install,
      note:
        "Rows install the pinned package, the Solid runtime the manifest row pins (completed with " +
        "@solidjs/web where a Solid 2 row pinned only solid-js), and the non-optional peers the " +
        "installed artifact itself declares. A package that imports something it declares nowhere is " +
        "not covered by that and is reported as an import throw."
    },
    probeFailures: { shapes: failureShapes, rows: failureRows },
    preContractFailures: { installFailures, generateFailures, probeErrors, timeouts, noRuntime },
    refusals,
    verified
  };
}

export function renderVerificationMarkdown(report) {
  const rate = (numerator, denominator) =>
    denominator ? `${numerator}/${denominator} (${((numerator / denominator) * 100).toFixed(2)}%)` : "n/a";
  const sortedEntries = object => Object.entries(object).sort((left, right) => right[1] - left[1]);
  const overall = report.overall;
  const lines = [];

  lines.push("# Ecosystem machine-verification report");
  lines.push("");
  lines.push(
    "How many real ecosystem packages machine-verify end to end: `contract generate` -> " +
      "`contract probe --write` -> `contract verify`, run against a throwaway install of every probe " +
      "row in the pinned corpus."
  );
  lines.push("");
  lines.push("> **This measurement executes package code.** `contract probe` imports and runs each");
  lines.push("> installed package, and its dependencies, in child processes. Every install and every");
  lines.push("> execution happened inside temporary directories under the harness state directory, npm");
  lines.push("> ran with `--ignore-scripts` so no package lifecycle script executed, and each probe ran");
  lines.push("> under both a per-mode timeout and a whole-phase wall budget.");
  lines.push("");
  lines.push(`- Started: ${report.startedAt}`);
  lines.push(`- Finished: ${report.finishedAt}`);
  lines.push(
    `- Manifest generated at: ${report.corpus.manifestGeneratedAt} (rows: ${report.corpus.rows}, probes: ${report.corpus.probesInManifest})`
  );
  lines.push(`- Probe rows run: ${report.corpus.probesRun}`);
  lines.push(
    `- Checker native binary: \`${report.checker.nativeBin.sha256}\` (${report.checker.nativeBin.size} bytes, mtime ${report.checker.nativeBin.mtime})`
  );
  lines.push(
    `- Type Facts binary: \`${report.checker.typeFactsBin.sha256}\` (${report.checker.typeFactsBin.size} bytes, mtime ${report.checker.typeFactsBin.mtime})`
  );
  lines.push(
    `- Budgets: install ${report.budgets.installTimeoutMs} ms, generate ${report.budgets.generateTimeoutMs} ms, ` +
      `probe ${report.budgets.probeModeTimeoutMs} ms per condition mode / ` +
      (report.budgets.probeWallBudgetMs
        ? `${report.budgets.probeWallBudgetMs} ms whole phase (fixed)`
        : `${report.budgets.probeBudgetBaseMs} ms + ${report.budgets.probeBudgetPerClaimMs} ms per planned claim, ` +
          `capped at ${report.budgets.probeBudgetCapMs} ms, whole phase`) +
      `, verify ${report.budgets.verifyTimeoutMs} ms; concurrency ${report.budgets.concurrency}`
  );
  lines.push(
    `- Import-environment shim: ${report.budgets.environmentShim === false ? "**disabled**" : "enabled"} ` +
      "(client, development and production sessions only; server sessions never)"
  );
  lines.push("");

  lines.push("## Headline");
  lines.push("");
  lines.push("| Figure | Count |");
  lines.push("| --- | --- |");
  lines.push(`| Probe rows run | ${report.corpus.probesRun} |`);
  lines.push(`| Reached a generated contract | ${rate(overall.contractsGenerated, overall.rows)} |`);
  lines.push(`| **Reached \`verified\`** | **${rate(overall.verified, overall.rows)}** of all rows |`);
  lines.push(
    `| Reached \`verified\`, of rows that produced a contract | ${rate(overall.verified, overall.contractsGenerated)} |`
  );
  lines.push(`| Refused by \`contract verify\` | ${rate(overall.refused, overall.rows)} |`);
  lines.push("");
  lines.push("Outcome classes, raw:");
  lines.push("");
  lines.push("| Outcome | Rows |");
  lines.push("| --- | --- |");
  for (const [name, count] of sortedEntries(overall.outcomes)) lines.push(`| \`${name}\` | ${count} |`);
  lines.push("");

  lines.push("## Per family");
  lines.push("");
  lines.push(
    "| Family | Rows | Contracts | Verified | Refused | Claims driven | Claims passed | Conversions | Exports certified | Exports unknown |"
  );
  lines.push("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |");
  for (const [, bucket] of Object.entries(report.byFamily)) {
    if (!bucket.rows) continue;
    lines.push(
      `| ${bucket.label} | ${bucket.rows} | ${bucket.contractsGenerated} | ${rate(bucket.verified, bucket.rows)} | ` +
        `${bucket.refused} | ${rate(bucket.claims.driven, bucket.claims.total)} | ` +
        `${rate(bucket.claims.passed, bucket.claims.driven)} | ${bucket.conversions} | ` +
        `${bucket.exports.certifiedInVerified} | ${bucket.exports.unknownInVerified} |`
    );
  }
  lines.push("");
  lines.push("| Solid target | Rows | Contracts | Verified | Refused |");
  lines.push("| --- | --- | --- | --- | --- |");
  for (const [name, bucket] of Object.entries(report.bySolidTarget)) {
    lines.push(
      `| ${name} | ${bucket.rows} | ${bucket.contractsGenerated} | ${rate(bucket.verified, bucket.rows)} | ${bucket.refused} |`
    );
  }
  lines.push("");

  lines.push("## Why verification refuses");
  lines.push("");
  lines.push(
    `${overall.refused} rows were refused. \`contract verify\` raises every blocker it finds rather than ` +
      "stopping at the first, so the row counts below sum to more than the number of refused rows."
  );
  lines.push("");
  lines.push("| Blocker (RFC 0002 §3) | Rows raising it | Blocker lines |");
  lines.push("| --- | --- | --- |");
  for (const [name, count] of sortedEntries(overall.blockerRows)) {
    lines.push(`| \`${name}\` | ${count} | ${overall.blockerLines[name] ?? 0} |`);
  }
  lines.push("");
  lines.push(
    "Attributed to one root cause per row instead. `probe-report-includes-evidence-write` is a " +
      "*consequence*: `contract probe --write` declines to write evidence once a probe failed or an " +
      "incompleteness was reported, so verification then sees passing claims that never reached the " +
      "contract. It is counted as a root cause only on a row where it stands alone."
  );
  lines.push("");
  lines.push("| Root cause | Refused rows |");
  lines.push("| --- | --- |");
  for (const [name, count] of sortedEntries(overall.rootCauses)) lines.push(`| \`${name}\` | ${count} |`);
  lines.push("");

  lines.push("## Drivability");
  lines.push("");
  lines.push("| Figure | Count |");
  lines.push("| --- | --- |");
  lines.push(`| Claims planned across every probed contract | ${overall.claims.total} |`);
  lines.push(`| Driven | ${rate(overall.claims.driven, overall.claims.total)} |`);
  lines.push(`| Passed | ${rate(overall.claims.passed, overall.claims.total)} |`);
  lines.push(`| Failed | ${overall.claims.failed} |`);
  lines.push(`| Undriven | ${rate(overall.claims.undriven, overall.claims.total)} |`);
  lines.push(`| Incompleteness findings | ${overall.claims.incompleteness} |`);
  lines.push("");
  lines.push("Undriven claims by reason:");
  lines.push("");
  lines.push("| Reason | Claims |");
  lines.push("| --- | --- |");
  for (const [name, count] of sortedEntries(overall.undriven)) lines.push(`| ${name} | ${count} |`);
  lines.push("");
  lines.push(
    "`no probe form: reactiveReads` and `no probe form: ownerRequirements` are family-A " +
      "compiler proofs that verification retains; *undriven* means no independent generic runtime " +
      "probe exists for them, not that the verified contract discarded those static claims. The " +
      "other rows must be read by their named reason: some become unknown, while a failed claim or " +
      "incompleteness remains a blocker."
  );
  lines.push("");

  const kindGaps = { rows: 0, ...emptyKindGaps(), ...(overall.kindGaps ?? {}) };
  const kindContradictions = {
    rows: 0,
    ...emptyKindGaps().contradictions,
    ...(kindGaps.contradictions ?? {})
  };
  lines.push("### Why a `kind` observation is missing");
  lines.push("");
  lines.push(
    "`kind` is the one claim schema v1 has no unknown sentinel for, so an unobserved one blocks " +
      "rather than converting — which makes *why* it was unobserved the number the rule's next " +
      "revision turns on. An **observation of absence** (`export-missing`: the namespace loaded and " +
      "the binding was not in it) says the export does not exist in that artifact, so there is no " +
      "consumer claim about that mode to certify. Every other non-observation is a **gap** — an " +
      "import that threw, a session that died, a mode never attempted, a mode where no unambiguous " +
      "summary resolves — and a gap must keep blocking. Every number in this section counts gaps " +
      "only: a mode that was observed and *disagreed* is a failing claim, and it has its own " +
      "section below rather than a row here, because amendment A9 forbids the two sharing a number."
  );
  lines.push("");
  lines.push(`- Rows with at least one gap in a stated \`kind\` mode: ${kindGaps.rows}`);
  lines.push(`- \`kind\` obligations with at least one gapped stated mode: ${kindGaps.claims}`);
  lines.push("");
  if (Object.keys(kindGaps.reasons).length) {
    lines.push("| Why the mode produced no passing `kind` observation | (claim, mode) pairs |");
    lines.push("| --- | --- |");
    for (const [name, count] of sortedEntries(kindGaps.reasons)) lines.push(`| ${name} | ${count} |`);
    lines.push("");
  }
  if (Object.keys(kindGaps.modes).length) {
    lines.push("| Mode | Gapped `kind` obligations |");
    lines.push("| --- | --- |");
    for (const [name, count] of sortedEntries(kindGaps.modes)) lines.push(`| \`${name}\` | ${count} |`);
    lines.push("");
  }

  lines.push("### `kind` claims the probe contradicted");
  lines.push("");
  lines.push(
    "A mode whose observation **exists and disagreed** with the contract. Nothing above counts " +
      "these, and nothing in any relaxation of the `kind` rule may absorb them: the package answered " +
      "the claim differently, which is a generator bug or a package change, and neither is fixed by " +
      "narrowing a mode away or converting a claim to unknown. They refuse the whole document today " +
      "and must keep doing so."
  );
  lines.push("");
  lines.push(`- Rows with at least one contradicted \`kind\` claim: ${kindContradictions.rows}`);
  lines.push(`- \`kind\` claims contradicted in at least one mode: ${kindContradictions.claims}`);
  lines.push("");
  if (Object.keys(kindContradictions.modes).length) {
    lines.push("| Mode | Contradicted `kind` claims |");
    lines.push("| --- | --- |");
    for (const [name, count] of sortedEntries(kindContradictions.modes)) {
      lines.push(`| \`${name}\` | ${count} |`);
    }
    lines.push("");
  }

  lines.push("## The probe environment");
  lines.push("");
  lines.push(
    `An entrypoint whose module cannot be imported yields no observation at all. ` +
      `${report.probeEnvironment.rowsWithAnEntrypointImportThrow} of the corpus's rows had at least one ` +
      "entrypoint import throw. The probe worker is a bare Node process: no DOM, no bundler, no JSX or " +
      "TypeScript loader, and only the packages the corpus manifest installs beside the probed one. " +
      "Some of these throws are facts about the package; others are facts about that environment, and " +
      "the two are not separated here."
  );
  lines.push("");
  lines.push("| Import failure | Claims left undriven |");
  lines.push("| --- | --- |");
  for (const [name, count] of sortedEntries(report.probeEnvironment.importThrows).slice(0, 20)) {
    lines.push(`| ${name.replace(/\|/g, "\\|")} | ${count} |`);
  }
  lines.push("");

  const shim = report.probeEnvironment.shim ?? { rowsShimmed: 0, shimmedGlobals: {}, modesShimmed: {} };
  lines.push("### The globals the probe worker faked");
  lines.push("");
  lines.push(
    "A module that reads `window` while it is being evaluated throws in a bare Node process, the " +
      "worker stops, and every claim of that entrypoint goes undriven — so nothing at all is observed " +
      "about the package. The worker therefore defines a small inert browser surface before it " +
      "imports anything, in the `client`, `development` and `production` sessions only."
  );
  lines.push("");
  lines.push(
    "**A claim observed under the shim is a weaker observation than one made in a browser.** The " +
      "fake `document` renders nothing, the fake `matchMedia` never matches, the fake `navigator` " +
      "says it is this checker. A package that branches on any of that was observed on the branch " +
      "the fake sent it down. Every `<contract>.probe.json` and `<contract>.verify.json` records the " +
      "per-mode list of faked names, so where the distinction matters the record says so rather than " +
      "the number implying a browser."
  );
  lines.push("");
  lines.push(
    "`server` sessions are never shimmed: an import that throws on `window` under `--conditions node` " +
      "is a truthful observation of that entrypoint in that mode, and faking it there would " +
      "manufacture a pass the package never earned."
  );
  lines.push("");
  lines.push(`- Rows where at least one session faked at least one global: ${shim.rowsShimmed}`);
  lines.push("");
  if (Object.keys(shim.shimmedGlobals).length) {
    lines.push("| Faked global | Rows |");
    lines.push("| --- | --- |");
    for (const [name, count] of sortedEntries(shim.shimmedGlobals)) {
      lines.push(`| \`${name}\` | ${count} |`);
    }
    lines.push("");
  }
  const sessions = report.probeEnvironment.sessions ?? { started: 0, restarts: 0, failed: 0 };
  lines.push("### Worker processes");
  lines.push("");
  lines.push(
    "A worker stops at its first throw and the mode is restarted for what is left — the only way to " +
      "un-halt a Solid 2.0 development runtime. A restart is not a failure; a row that needed many is " +
      "the shape behind a slow or timed-out probe."
  );
  lines.push("");
  lines.push("| Figure | Count |");
  lines.push("| --- | --- |");
  lines.push(`| Worker processes started | ${sessions.started} |`);
  lines.push(`| Of those, restarts after a throw | ${sessions.restarts} |`);
  lines.push(`| Sessions that died (crash, timeout, unreadable output) | ${sessions.failed} |`);
  lines.push("");

  const installEnvironment = report.installEnvironment ?? {};
  lines.push("## The install environment");
  lines.push("");
  lines.push(
    "Each row installs the pinned package, the Solid runtime the manifest row pins, and the " +
      "non-optional peers the installed artifact's own `package.json` declares. Peers are installed " +
      "in a second npm invocation so that no peer range can take part in resolving the pinned " +
      "versions; if it moves a pin anyway, the pinned-only tree is restored and the row is recorded " +
      "as such."
  );
  lines.push("");
  lines.push("| Figure | Rows |");
  lines.push("| --- | --- |");
  lines.push(
    `| Solid 2 rows given the \`@solidjs/web\` half of the runtime the row pinned only half of | ${installEnvironment.runtimeCompleted ?? 0} |`
  );
  lines.push(`| Rows with a completed peer install | ${installEnvironment.peerComplete ?? 0} |`);
  lines.push(`| Peer packages installed | ${installEnvironment.peersInstalled ?? 0} |`);
  lines.push(`| Rows whose peer install failed or moved a pin | ${installEnvironment.peerFailed ?? 0} |`);
  lines.push("");
  lines.push(
    "A package that **imports something it declares nowhere** — not a dependency, not a peer — is " +
      "outside what any install policy can supply, and is reported above as an import throw rather " +
      "than fixed here. Completing an undeclared import would mean this harness choosing a version " +
      "the package never named."
  );
  lines.push("");

  const probeFailures = report.probeFailures ?? { shapes: {}, rows: [] };
  lines.push("## Probe failures: claims the package answered differently");
  lines.push("");
  lines.push(
    "A **failure** is the strongest thing this measurement produces. The contract states a claim, the " +
      "probe drove it, and the package did something else — a generator bug or a package change, never " +
      "an environment gap and never an unreachable claim. Verification refuses the whole contract on " +
      "one of these, deliberately: converting a contradicted claim to the unknown sentinel would hide " +
      "it."
  );
  lines.push("");
  lines.push(`${probeFailures.rows.length} failing claim(s) across the corpus, by shape:`);
  lines.push("");
  lines.push("| Claim, claimed, observed | Claims |");
  lines.push("| --- | --- |");
  for (const [shape, count] of sortedEntries(probeFailures.shapes)) {
    lines.push(`| ${shape.replace(/\|/g, "\\|")} | ${count} |`);
  }
  lines.push("");
  if (probeFailures.rows.length) {
    const shown = probeFailures.rows.slice(0, 60);
    lines.push(
      shown.length < probeFailures.rows.length
        ? `The first ${shown.length}, in full (the JSON report carries all ${probeFailures.rows.length}):`
        : "Each one, in full:"
    );
    lines.push("");
    lines.push("| Probe | Export | Claim | Observed | Modes |");
    lines.push("| --- | --- | --- | --- | --- |");
    for (const failure of shown) {
      lines.push(
        `| \`${failure.probeId}\` | \`${failure.entrypoint}:${failure.export}\` | \`${failure.claim}\` | ` +
          `${failure.observed ?? "—"} | ${failure.modes.join(", ") || "—"} |`
      );
    }
    lines.push("");
  }

  lines.push("## Conversion volume");
  lines.push("");
  lines.push(
    'A conversion replaces one export\'s whole claim domain with the `{"status":"unknown"}` sentinel ' +
      "because the probe neither observed nor statically proved it."
  );
  lines.push("");
  lines.push("| Figure | Count |");
  lines.push("| --- | --- |");
  lines.push(`| Claim domains converted to unknown | ${overall.conversions} |`);
  lines.push(
    `| Exports carrying an unknown in the verified rows, at generation | ${rate(overall.exports.draftUnknownInVerified, overall.exports.draftExportsInVerified)} |`
  );
  lines.push(
    `| Exports carrying an unknown in the verified rows, after verification | ${rate(overall.exports.unknownInVerified, overall.exports.certifiedInVerified + overall.exports.unknownInVerified)} |`
  );
  lines.push("");
  lines.push("How much a verified contract actually certifies from observation:");
  lines.push("");
  lines.push("| Figure | Count |");
  lines.push("| --- | --- |");
  lines.push(
    `| Verified rows carrying at least one probed behavioral row | ${rate(overall.rowsWithProbedEvidence, overall.verified)} |`
  );
  lines.push(`| Probed behavioral row markers kept across the whole corpus | ${overall.probedRowsKept} |`);
  lines.push(`| Inferred row markers dropped by verification | ${overall.droppedInferredMarkers} |`);
  lines.push(
    `| Probed markers discarded as unwitnessed by this run's report | ${overall.staleProbedMarkers} |`
  );
  lines.push(
    `| Entrypoints verification refused inside a promoted document | ${overall.verificationRefusedEntrypoints} |`
  );
  lines.push(
    `| Verified rows carrying at least one such refusal | ${overall.rowsWithAVerificationRefusedEntrypoint} |`
  );
  lines.push("");
  lines.push(
    "The last two rows are a **cost made visible, not a regression**. An entrypoint whose `kind` " +
      "claims this run did not observe is refused and omitted, exactly as `contract generate` already " +
      "refuses an entrypoint it cannot certify, so the package's other entrypoints are not sunk by one " +
      "unimportable subpath. A refused entrypoint is absent from the contract, which is an explicit " +
      "uncertifiable result at the consumer rather than a wrong claim; a document where *no* " +
      "entrypoint would certify anything is still refused whole. The exports it dropped are their " +
      "own state in the composite below, still inside its denominator: a certified *share* that rose " +
      "because unobservable exports left the population would be measuring nothing."
  );
  lines.push("");
  lines.push("Converted domains by field:");
  lines.push("");
  lines.push("| Field | Conversions |");
  lines.push("| --- | --- |");
  for (const [name, count] of sortedEntries(overall.conversionFields)) {
    lines.push(`| \`${name}\` | ${count} |`);
  }
  lines.push("");

  // The denominator is the *draft* export count of every verified row plus every
  // export of the rows that never verified -- not the promoted documents' own
  // counts. A verification-refused entrypoint's exports are still exports the
  // corpus's generated contracts describe, so dropping them out of the
  // denominator would raise the certified share for a reason with no
  // certification behind it (RFC 0002 amendment A9's re-measurement forbids
  // exactly that movement). They are their own state instead.
  // `?? 0` on the new state alone, so a report written before it existed still
  // renders its other three rather than a table of `n/a`.
  const refusedInVerified = overall.exports.refusedInVerified ?? 0;
  const totalExports =
    overall.exports.certifiedInVerified +
    overall.exports.unknownInVerified +
    refusedInVerified +
    overall.exports.inUnverifiedContract;
  lines.push("## The composite a consumer feels");
  lines.push("");
  lines.push("Of every export the corpus's generated contracts describe:");
  lines.push("");
  lines.push("| State | Exports |");
  lines.push("| --- | --- |");
  lines.push(
    `| (a) certified by a verified contract | ${rate(overall.exports.certifiedInVerified, totalExports)} |`
  );
  lines.push(
    `| (b) honest unknown inside a verified contract | ${rate(overall.exports.unknownInVerified, totalExports)} |`
  );
  lines.push(
    `| (c) dropped from a verified contract with its refused entrypoint | ${rate(refusedInVerified, totalExports)} |`
  );
  lines.push(
    `| (d) inside a contract that never reached \`verified\` | ${rate(overall.exports.inUnverifiedContract, totalExports)} |`
  );
  lines.push("");
  lines.push(
    "(c) is the cost of amendment A9 stage 1 stated as a consumer-facing number: the row verified, " +
      "and these exports are absent from the document it promoted, so importing one is an explicit " +
      "uncertifiable result. They stay in the denominator — a certified *share* that rose because " +
      "unobservable exports left the population would be measuring nothing. (d) is every export of a " +
      "contract that was generated and then refused, timed out, or errored before a probe report " +
      "existed. Rows whose `npm install` or `contract generate` failed describe no exports at all " +
      "and are in none of the four states."
  );
  lines.push("");

  lines.push("## Wall time");
  lines.push("");
  lines.push("| Phase | Rows | Median | p90 | Max | Mean |");
  lines.push("| --- | --- | --- | --- | --- | --- |");
  for (const [name, value] of Object.entries(report.phaseWallMs)) {
    lines.push(
      `| ${name} | ${value.count} | ${value.medianMs} ms | ${value.p90Ms} ms | ${value.maxMs} ms | ${value.meanMs} ms |`
    );
  }
  lines.push("");
  lines.push(
    "`install` may run against a warm npm cache, so it is a lower bound; `pipelineWithoutInstall` is " +
      "the number that describes the checker's own cost."
  );
  lines.push("");

  const { installFailures, generateFailures, probeErrors, timeouts } = report.preContractFailures;
  const noRuntime = report.preContractFailures.noRuntime ?? [];
  lines.push("## Rows that never reached verification");
  lines.push("");
  lines.push("| Stage | Rows |");
  lines.push("| --- | --- |");
  lines.push(`| \`npm install\` failed | ${installFailures.length} |`);
  lines.push(`| \`contract generate\` failed | ${generateFailures.length} |`);
  lines.push(
    `| \`contract probe\` errored before writing a report | ${Object.values(probeErrors).reduce((sum, value) => sum + value, 0)} |`
  );
  lines.push(`| no Solid runtime the row could honestly be probed against | ${noRuntime.length} |`);
  lines.push(`| timed out under the harness budget | ${timeouts.length} |`);
  lines.push("");
  if (noRuntime.length) {
    lines.push(
      "The manifest pins the runtime each row runs against, and for these it pins no `solid-js` — " +
        "`@solidjs/signals` *is* the reactive core, so there is no second package to settle a probe " +
        "with. Pairing one in would be this harness auditing a combination the corpus deliberately " +
        "did not. They are their own class rather than an error:"
    );
    lines.push("");
    for (const row of noRuntime) lines.push(`- \`${row.probeId}\``);
    lines.push("");
  }
  if (Object.keys(probeErrors).length) {
    lines.push("Probe errors by cause:");
    lines.push("");
    lines.push("| Cause | Rows |");
    lines.push("| --- | --- |");
    for (const [name, count] of sortedEntries(probeErrors)) lines.push(`| ${name} | ${count} |`);
    lines.push("");
  }
  if (generateFailures.length) {
    const classes = {};
    for (const failure of generateFailures) classes[failure.class] = (classes[failure.class] ?? 0) + 1;
    lines.push("Generation failures by class:");
    lines.push("");
    lines.push("| Class | Rows |");
    lines.push("| --- | --- |");
    for (const [name, count] of sortedEntries(classes)) lines.push(`| \`${name}\` | ${count} |`);
    lines.push("");
  }
  if (timeouts.length) {
    lines.push("Timeouts, named individually because a timeout is never a verification result:");
    lines.push("");
    for (const timeout of timeouts) {
      lines.push(
        `- \`${timeout.probeId}\` — ${timeout.outcome} after ${timeout.totalMs} ms` +
          (timeout.budgetMs
            ? ` (budget ${timeout.budgetMs} ms for ${timeout.plannedClaims ?? "?"} planned claims)`
            : "")
      );
    }
    lines.push("");
  }

  lines.push("## Caveats, stated because these numbers are easy to over-read");
  lines.push("");
  lines.push(
    "- **`verified` is not `reviewed`.** A verified contract certifies what a machine observed or " +
      "statically proved and converts everything else to the unknown sentinel. It is a weaker claim " +
      "than the human `reviewed` tier, and a stronger one than the `inferred` draft the generation " +
      "benchmark measures."
  );
  lines.push(
    "- **Some observations were made against a fake DOM.** The probe worker defines a minimal inert " +
      "browser surface in the client, development and production sessions so that an import-time " +
      "`window` read does not cost the whole entrypoint. What is then observed is the package's " +
      "behavior *given that fake*, which is not the same fact as its behavior in a browser. Every " +
      "probe report and verify sidecar names the globals it faked; server sessions fake nothing."
  );
  lines.push(
    "- **The install is peer-complete, not project-complete.** It installs the probed package, the " +
      "Solid runtime the manifest row pins, and the peers the artifact declares. A package that " +
      "imports something it declares nowhere still fails to import, and that is a fact about the " +
      "package rather than about this harness."
  );
  lines.push(
    "- **A timeout is never a verification result.** Rows that exceeded the probe wall budget are " +
      "their own outcome class and are counted as neither verified nor refused. The budget now scales " +
      "with each row's planned claim count, so fewer rows hit one — which changes how many rows the " +
      "measurement can speak about, never what a timeout means."
  );
  lines.push(
    "- **Per probe row, not per package.** A package with a Solid 1.x row and two Solid 2.x rows " +
      "contributes three rows to every figure here."
  );
  lines.push(
    "- **This measurement executed package code.** Nothing here is a safety claim about any package; " +
      "it is a record of what happened when each one was imported and driven in a sandboxed child " +
      "process."
  );
  lines.push("");

  lines.push("## Every refusal");
  lines.push("");
  lines.push("| Probe | Family | Root cause | Blocker lines | Classes | Kind gaps |");
  lines.push("| --- | --- | --- | --- | --- | --- |");
  for (const refusal of report.refusals) {
    // The kind-gap column is the per-row half of "Why a `kind` observation is
    // missing": which rows are absences the rule could stop requiring, and which
    // are gaps that must keep blocking.
    const contradicted = refusal.kindGaps?.contradictions?.claims ?? 0;
    const gaps = [
      ...sortedEntries(refusal.kindGaps?.reasons ?? {}).map(([name, count]) => `${name} x${count}`),
      // Named apart from the gaps in the same cell, never folded into one of
      // them: this row's `kind` was contradicted, which no relaxation may absorb.
      ...(contradicted ? [`**contradicted** x${contradicted}`] : [])
    ].join(", ");
    lines.push(
      `| \`${refusal.probeId}\` | ${refusal.family} | \`${refusal.rootCause}\` | ${refusal.blockerCount} | ${refusal.blockerClasses.join(", ")} | ${gaps || "—"} |`
    );
  }
  lines.push("");

  lines.push("## Every verified contract");
  lines.push("");
  lines.push(
    "| Probe | Exports | Exports unknown | Conversions | Probed rows kept | Entrypoints refused |"
  );
  lines.push("| --- | --- | --- | --- | --- | --- |");
  for (const row of report.verified) {
    lines.push(
      `| \`${row.probeId}\` | ${row.exports} | ${row.exportsUnknown} | ${row.conversions} | ` +
        `${row.probedRowsKept} | ${(row.refusedEntrypoints ?? []).map(name => `\`${name}\``).join(", ") || "—"} |`
    );
  }
  lines.push("");

  // Every section pushes its own trailing separator, so the last one would
  // leave a blank line at end of file -- which `git diff --check` rejects.
  // Trim it here rather than special-casing the final section, so the written
  // artifact and a regeneration of it stay byte-identical.
  return `${lines.join("\n").replace(/\n+$/, "")}\n`;
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

function usage() {
  return `Usage: node scripts/ecosystem-benchmark/verify-corpus.mjs [options]

  --state-dir <DIR>      journal + scratch installs (default: a fresh temp dir;
                         pass an existing one to resume an interrupted run)
  --concurrency <N>      default ${DEFAULTS.concurrency}
  --every <N>            run every Nth manifest probe row (documented, deterministic
                         subsetting; the report still says how many rows ran)
  --offset <N>           offset for --every, default 0
  --ids <A,B>            run only these probe ids
  --limit <N>            stop after N selected rows
  --probe-timeout <MS>   per condition-mode child timeout (default ${DEFAULTS.probeModeTimeoutMs})
  --probe-budget <MS>    fixed whole-phase wall budget for every row. Default is
                         scaled instead: ${DEFAULTS.probeBudgetBaseMs} ms + ${DEFAULTS.probeBudgetPerClaimMs} ms per planned claim,
                         capped at ${DEFAULTS.probeBudgetCapMs} ms
  --no-environment-shim  do not let the probe worker define the minimal browser
                         globals in client/development/production sessions.
                         The state every measurement before this one ran in,
                         and the way to separate the shim's effect from the
                         engine's
  --no-peer-install      install only what the manifest row pins: no declared
                         peers, no completion of the Solid 2 runtime's
                         @solidjs/web half. Same purpose as the flag above
  --json <FILE>          default benchmarks/ecosystem/verification-report.json
  --markdown <FILE>      default benchmarks/ecosystem/verification-report.md
  --aggregate-only       rebuild the reports from the state dir's journal
  -h, --help

*** contract probe EXECUTES the installed packages' code. Run this where you
would run those packages' own test suites. ***

Requires SOLID_CHECKER_NATIVE_BIN and SOLID_TYPEFACTS_BIN to be set and to
exist. Copy both binaries somewhere stable before a long run: a concurrent
rebuild would otherwise change the engine mid-measurement, and the report
records each binary's hash as the identity the numbers belong to.
`;
}

function parseArgs(argv) {
  const options = {
    stateDir: null,
    concurrency: DEFAULTS.concurrency,
    every: null,
    offset: 0,
    ids: null,
    limit: null,
    probeModeTimeoutMs: DEFAULTS.probeModeTimeoutMs,
    // `null` means "scale it per row"; `--probe-budget` pins it flat.
    probeWallBudgetMs: null,
    environmentShim: true,
    peerInstall: true,
    json: join(REPORT_DIR, "verification-report.json"),
    markdown: join(REPORT_DIR, "verification-report.md"),
    aggregateOnly: false,
    help: false
  };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "-h" || argument === "--help") options.help = true;
    else if (argument === "--state-dir") options.stateDir = argv[++index];
    else if (argument === "--concurrency") options.concurrency = Number(argv[++index]);
    else if (argument === "--every") options.every = Number(argv[++index]);
    else if (argument === "--offset") options.offset = Number(argv[++index]);
    else if (argument === "--ids") options.ids = new Set(argv[++index].split(","));
    else if (argument === "--limit") options.limit = Number(argv[++index]);
    else if (argument === "--probe-timeout") options.probeModeTimeoutMs = Number(argv[++index]);
    else if (argument === "--probe-budget") options.probeWallBudgetMs = Number(argv[++index]);
    else if (argument === "--no-environment-shim") options.environmentShim = false;
    else if (argument === "--no-peer-install") options.peerInstall = false;
    else if (argument === "--json") options.json = argv[++index];
    else if (argument === "--markdown") options.markdown = argv[++index];
    else if (argument === "--aggregate-only") options.aggregateOnly = true;
    else throw new Error(`unrecognized option: ${argument}`);
  }
  return options;
}

function binaryIdentity(path) {
  const bytes = readFileSync(path);
  return {
    path,
    sha256: createHash("sha256").update(bytes).digest("hex"),
    size: bytes.length,
    mtime: statSync(path).mtime.toISOString()
  };
}

function readJournal(path) {
  if (!existsSync(path)) return [];
  const records = [];
  for (const line of readFileSync(path, "utf8").split("\n")) {
    if (!line.trim()) continue;
    try {
      records.push(JSON.parse(line));
    } catch {
      // A torn final line from an interrupted run is simply not resumed.
    }
  }
  return records;
}

async function main(argv = process.argv.slice(2)) {
  let options;
  try {
    options = parseArgs(argv);
  } catch (error) {
    console.error(usage());
    console.error(`solid-checker-verify-corpus: ${error.message}`);
    process.exit(2);
    return;
  }
  if (options.help) {
    console.log(usage());
    return;
  }

  const nativeBin = process.env.SOLID_CHECKER_NATIVE_BIN;
  const typeFactsBin = process.env.SOLID_TYPEFACTS_BIN;
  for (const [name, value] of [
    ["SOLID_CHECKER_NATIVE_BIN", nativeBin],
    ["SOLID_TYPEFACTS_BIN", typeFactsBin]
  ]) {
    if (!value || !existsSync(value)) {
      console.error(`solid-checker-verify-corpus: ${name} is not set to an existing file`);
      process.exit(2);
      return;
    }
  }

  const stateDir = options.stateDir
    ? resolve(options.stateDir)
    : await mkdtemp(join(tmpdir(), "solid-checker-verify-corpus-"));
  mkdirSync(stateDir, { recursive: true });
  const workDir = join(stateDir, "work");
  mkdirSync(workDir, { recursive: true });
  const journalPath = join(stateDir, "journal.jsonl");

  const manifest = JSON.parse(readFileSync(MANIFEST, "utf8"));
  // Explicit rather than spread over `options`: the report's budget block is
  // read as the run's configuration, and a stray `--ids` Set or report path in
  // it would be noise a reader has to discount.
  const budgets = {
    installTimeoutMs: DEFAULTS.installTimeoutMs,
    peerInstallTimeoutMs: DEFAULTS.peerInstallTimeoutMs,
    generateTimeoutMs: DEFAULTS.generateTimeoutMs,
    probeModeTimeoutMs: options.probeModeTimeoutMs,
    probeWallBudgetMs: options.probeWallBudgetMs,
    probeBudgetBaseMs: DEFAULTS.probeBudgetBaseMs,
    probeBudgetPerClaimMs: DEFAULTS.probeBudgetPerClaimMs,
    probeBudgetCapMs: DEFAULTS.probeBudgetCapMs,
    verifyTimeoutMs: DEFAULTS.verifyTimeoutMs,
    concurrency: options.concurrency,
    // Recorded in the report because they change what the numbers are a
    // measurement of: a run with these off measures the engine alone.
    environmentShim: options.environmentShim,
    peerInstall: options.peerInstall,
    ...(options.every ? { selection: `every ${options.every}th manifest probe row, offset ${options.offset}` } : {})
  };
  const checker = {
    note:
      "Copy both binaries somewhere stable before a long run and point the environment at the " +
      "copies, so a concurrent rebuild cannot change the engine mid-measurement.",
    nativeBin: binaryIdentity(nativeBin),
    typeFactsBin: binaryIdentity(typeFactsBin)
  };

  if (!options.aggregateOnly) {
    const { expandContract } = await import(
      pathToFileURL(join(ROOT, "packages/cli/scripts/contract-document.mjs")).href
    );
    // The same planner `contract probe` runs, so the wall budget a row gets is
    // scaled by the exact claim count that row is about to drive.
    const { buildProbePlan } = await import(
      pathToFileURL(join(ROOT, "packages/cli/scripts/contract-probe-driver.mjs")).href
    );
    let tasks = [];
    for (const row of sortRows([...manifest.rows])) {
      for (const probe of row.probes ?? []) tasks.push({ row, probe });
    }
    if (options.every)
      tasks = tasks.filter((_, index) => (index - options.offset) % options.every === 0);
    if (options.ids) tasks = tasks.filter(task => options.ids.has(task.probe.id));
    if (options.limit) tasks = tasks.slice(0, options.limit);

    const done = new Set(readJournal(journalPath).map(record => record.probeId));
    const pending = tasks.filter(task => !done.has(task.probe.id));
    process.stderr.write(
      `solid-checker-verify-corpus: ${tasks.length} rows selected, ${done.size} journaled, ${pending.length} to run\n`
    );

    const context = {
      workDir,
      budgets,
      cliEnv: { SOLID_CHECKER_NATIVE_BIN: nativeBin, SOLID_TYPEFACTS_BIN: typeFactsBin },
      expandContract,
      buildProbePlan,
      manifest,
      environmentShim: options.environmentShim,
      peerInstall: options.peerInstall
    };
    let cursor = 0;
    let completed = 0;
    await Promise.all(
      Array.from({ length: Math.max(1, options.concurrency) }, async () => {
        while (cursor < pending.length) {
          const task = pending[cursor++];
          const record = await runRow(task, context);
          record.finishedAt = new Date().toISOString();
          appendFileSync(journalPath, `${JSON.stringify(record)}\n`);
          completed += 1;
          process.stderr.write(
            `[${completed}/${pending.length}] ${record.probeId} -> ${record.outcome} (${record.totalMs}ms)\n`
          );
        }
      })
    );
  }

  const records = readJournal(journalPath);
  if (!records.length) {
    console.error(`solid-checker-verify-corpus: no journal records at ${journalPath}`);
    process.exit(2);
    return;
  }
  const report = buildVerificationReport({ records, manifest, budgets, checker });
  mkdirSync(dirname(options.json), { recursive: true });
  mkdirSync(dirname(options.markdown), { recursive: true });
  writeFileSync(options.json, `${JSON.stringify(report, null, 2)}\n`);
  writeFileSync(options.markdown, renderVerificationMarkdown(report));
  console.log(
    `solid-checker-verify-corpus: ${records.length} rows, ${report.overall.verified} verified, ` +
      `${report.overall.refused} refused; reports written to ${options.json} and ${options.markdown}`
  );
}

const isMain = process.argv[1] && pathToFileURL(resolve(process.argv[1])).href === import.meta.url;
if (isMain) {
  main().catch(error => {
    console.error(`solid-checker-verify-corpus: unhandled error: ${error?.stack ?? error}`);
    process.exit(2);
  });
}
