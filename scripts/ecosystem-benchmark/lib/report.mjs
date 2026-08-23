// Builds the ecosystem benchmark's report: the JSON figures `run.mjs` writes
// to disk, the Markdown a human reads, and the pass/fail verdict CI reads in
// `--thresholds` mode.
//
// This module owns no measurement — every fact it reports came from
// `lib/classify.mjs` (via each probe result's `class`/`signature`/`detail`)
// or from the manifest (`lib/manifest.mjs`). Its only job is aggregation:
// group probe results by family and Solid target, count, and present. That
// makes determinism the central constraint, because a benchmark report that
// silently reorders itself between two runs of the same data is worse than
// no report — a reviewer would read reordering as a real change. So every
// array that reaches the returned object is built through an explicit
// comparator, never left in whatever order `results` arrived in, and no
// wall-clock read (`Date.now()`, `new Date()`) ever happens in here — the
// only timestamps in the output are the `startedAt`/`finishedAt` values the
// caller supplies, and `durationMs`, which is derived from them with
// `Date.parse` (a pure string→number function, not a clock read).

import { FAMILIES } from "./families.mjs";
import { FAILURE_CLASSES } from "./classify.mjs";
import {
  BEHAVIORAL_ROW_KINDS,
  CLAIM_DOMAINS,
  emptyBehavioralRows,
  emptyDomainCounts
} from "./contract-content.mjs";
import { manifestStats } from "./manifest.mjs";

const SCHEMA_VERSION = 1;

// How much of a failing probe's stderr the Markdown report keeps inline. The
// JSON report always keeps the complete capture (see the "results" array
// below) — this limit exists only to keep the human-readable report
// skimmable, and every place it fires says so explicitly next to the text it
// cut, per INTERFACES.md's "must state that it truncated" requirement.
const MARKDOWN_STDERR_LIMIT = 400;

const CLASS_RANK = new Map(FAILURE_CLASSES.map((id, index) => [id, index]));

function classRank(id) {
  const rank = CLASS_RANK.get(id);
  return typeof rank === "number" ? rank : Number.POSITIVE_INFINITY;
}

function compareStrings(left, right) {
  if (left === right) return 0;
  return left < right ? -1 : 1;
}

// Sort key for a package-level result row: package name first (so a family's
// table reads alphabetically by package), then the probe id (unique per
// row) as a final tiebreak so two probes for the same package (floor/head)
// always land in the same relative order regardless of the input array's
// order.
function comparePackageThenProbe(left, right) {
  const packageDelta = compareStrings(left?.package ?? "", right?.package ?? "");
  if (packageDelta !== 0) return packageDelta;
  return compareStrings(left?.probeId ?? "", right?.probeId ?? "");
}

// Parses a timestamp without ever constructing a "now" — both branches are
// pure functions of the value the caller passed in, never of wall-clock
// time.
function toEpochMs(value) {
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value === "string") {
    const parsed = Date.parse(value);
    return Number.isNaN(parsed) ? null : parsed;
  }
  return null;
}

function computeDurationMs(startedAt, finishedAt) {
  const start = toEpochMs(startedAt);
  const finish = toEpochMs(finishedAt);
  if (start === null || finish === null) return null;
  return finish - start;
}

function sumField(items, field) {
  return items.reduce((total, item) => {
    const value = item?.[field];
    return total + (typeof value === "number" ? value : 0);
  }, 0);
}

// A success percentage without its denominator is a number nobody can trust
// — "83% success" means nothing without knowing whether that is 5/6 or
// 830/1000. `percentage` is therefore always paired with `successes`/`total`,
// and is `null` (never `0` and never `NaN`) when no probe ran at all, so an
// empty family reads as "nothing measured" rather than "everything failed".
function computeSuccessRate(successes, total) {
  return {
    percentage: total > 0 ? Math.round((successes / total) * 10000) / 100 : null,
    successes,
    total
  };
}

function groupBy(items, keyFn) {
  const map = new Map();
  for (const item of items) {
    const key = keyFn(item);
    const bucket = map.get(key);
    if (bucket) bucket.push(item);
    else map.set(key, [item]);
  }
  return map;
}

// Groups failing results by (class, signature) — the class.mjs-normalized
// signature already erases path/offset/version/callee-value noise (see
// classify.mjs's OBJECT_LITERAL_PATTERN, which keeps a Rust obligation's
// field *names* but drops their values), so two `reactive-dispatch-unresolved`
// failures against different packages with different callees already share
// one signature string and land in the same group here without any further
// normalization on this side.
function buildFailureGroups(results) {
  const failures = results.filter(result => result?.outcome === "failure");
  const groups = [];
  const byClass = groupBy(failures, result => result?.class ?? "unclassified");
  for (const [className, classResults] of byClass) {
    const bySignature = groupBy(classResults, result => result?.signature ?? "");
    for (const [signature, signatureResults] of bySignature) {
      const packages = [...new Set(signatureResults.map(result => result.package))].sort(compareStrings);
      groups.push({ class: className, signature, count: signatureResults.length, packages });
    }
  }
  groups.sort((left, right) => {
    if (left.count !== right.count) return right.count - left.count;
    const rankDelta = classRank(left.class) - classRank(right.class);
    if (rankDelta !== 0) return rankDelta;
    return compareStrings(left.signature, right.signature);
  });
  return groups;
}

// Every probe that put a contract on disk — complete or partial. Both
// contribute real generated entrypoints, so both are summed for the
// declared-versus-generated comparison; only the complete ones count as
// successes.
function contractProducing(results) {
  return results.filter(
    result => result.outcome === "success" || result.outcome === "partial-success"
  );
}

function buildFamilySection(family, results) {
  const familyResults = results.filter(result => result?.family === family.id);
  const successes = familyResults.filter(result => result.outcome === "success");
  const partials = familyResults.filter(result => result.outcome === "partial-success");
  const failures = familyResults.filter(result => result.outcome === "failure");
  const compatiblePackages = new Set(familyResults.map(result => result.package));

  return {
    family: family.id,
    label: family.label,
    compatiblePackageCount: compatiblePackages.size,
    probeCount: familyResults.length,
    // declaredEntrypoints is read from disk regardless of outcome (see
    // INTERFACES.md's "Entrypoint counting"), so it is summed over every
    // probe; generatedEntrypoints only exists where a contract was written, so
    // it is summed over the probes that wrote one rather than presenting
    // failures as contributing zero real entrypoints.
    declaredEntrypoints: sumField(familyResults, "declaredEntrypoints"),
    generatedEntrypoints: sumField(contractProducing(familyResults), "generatedEntrypoints"),
    // The entrypoints those partial contracts do NOT describe. Reported next
    // to the generated count because the pair is the whole point: a family can
    // generate contracts for every package and still leave entrypoints
    // uncertifiable.
    refusedEntrypoints: sumField(partials, "refusedEntrypoints"),
    successCount: successes.length,
    partialCount: partials.length,
    failureCount: failures.length,
    successRate: computeSuccessRate(successes.length, familyResults.length),
    failureGroups: buildFailureGroups(familyResults),
    results: [...familyResults].sort(comparePackageThenProbe)
  };
}

function buildTotals(results) {
  const successes = results.filter(result => result.outcome === "success");
  const partials = results.filter(result => result.outcome === "partial-success");
  const failures = results.filter(result => result.outcome === "failure");
  const compatiblePackages = new Set(results.map(result => result.package));
  return {
    compatiblePackageCount: compatiblePackages.size,
    probeCount: results.length,
    declaredEntrypoints: sumField(results, "declaredEntrypoints"),
    generatedEntrypoints: sumField(contractProducing(results), "generatedEntrypoints"),
    refusedEntrypoints: sumField(partials, "refusedEntrypoints"),
    successCount: successes.length,
    partialCount: partials.length,
    failureCount: failures.length,
    successRate: computeSuccessRate(successes.length, results.length),
    failureGroups: buildFailureGroups(results)
  };
}

// Every partial contract, as its own list: a partial probe is absent from
// `failureGroups` (it did not fail) and invisible in `successRate` (it is not
// a success), so without this the only trace of it in the Markdown would be a
// class name in one table cell.
function buildPartialContracts(results) {
  return results
    .filter(result => result?.outcome === "partial-success")
    .map(result => ({
      probeId: result.probeId,
      package: result.package,
      version: result.version,
      family: result.family,
      generatedEntrypoints: result.generatedEntrypoints ?? null,
      refusedEntrypoints: result.refusedEntrypoints ?? null
    }))
    .sort(comparePackageThenProbe);
}

function buildWorkerTimings(results) {
  const totalDurationMs = sumField(results, "durationMs");
  const installDurationMs = sumField(results, "installDurationMs");
  const generationDurationMs = sumField(results, "generationDurationMs");
  return {
    totalDurationMs,
    installDurationMs,
    generationDurationMs,
    harnessDurationMs: Math.max(0, totalDurationMs - installDurationMs - generationDurationMs)
  };
}

// beta-only / rc-only membership follows `lib/select.mjs`'s own definition of
// "only" exactly: a package gets a single `kind: "only"` probe precisely
// when its floor and head selection coincide, which is how a package with no
// stable Solid 2 release at all still gets exactly one probe. So "beta-only"
// here means literally "this package's one probe runs on a beta", not a
// looser heuristic over channels across multiple probes.
function buildChannelOnlyLists(solid2Results) {
  const betaOnly = [];
  const rcOnly = [];
  const byPackage = groupBy(solid2Results, result => result.package);
  for (const [packageName, packageResults] of byPackage) {
    if (packageResults.length !== 1) continue;
    const [only] = packageResults;
    if (only.probeKind !== "only") continue;
    const entry = { package: packageName, family: only.family, version: only.version, channel: only.channel };
    if (only.channel === "beta") betaOnly.push(entry);
    else if (only.channel === "rc") rcOnly.push(entry);
  }
  betaOnly.sort((left, right) => compareStrings(left.package, right.package));
  rcOnly.sort((left, right) => compareStrings(left.package, right.package));
  return { betaOnlyPackages: betaOnly, rcOnlyPackages: rcOnly };
}

// Probe outcomes are an ordered scale, not a boolean. `partial-success` means a
// contract was emitted but entrypoints were refused, so it sits strictly
// between a complete contract and no contract at all. Comparing two outcomes as
// "success vs not success" silently drops every move that starts or ends in the
// middle: a probe going `partial-success` -> `failure` lost the whole contract
// and matched neither regression nor fix, and `failure` -> `partial-success`
// gained one and counted as neither. Direction on this scale is the comparison;
// each entry also carries both outcomes, so a partial move is never read as a
// total one.
const OUTCOME_RANK = new Map([
  ["success", 2],
  ["partial-success", 1],
  ["failure", 0]
]);

// An unknown outcome ranks with `failure`: the comparison must never invent an
// improvement out of a value it does not understand.
function outcomeRank(outcome) {
  return OUTCOME_RANK.get(outcome) ?? 0;
}

// Floor/head comparisons only make sense for a package that actually probed
// both ends — a package selected via a single `only` probe (floor === head)
// has no floor/head delta to report and must never be forced into either
// list just because it has *a* probe.
function buildFloorHeadDiffs(solid2Results) {
  const worksOnFloorFailsAtHead = [];
  const failsOnFloorWorksAtHead = [];
  const byPackage = groupBy(solid2Results, result => result.package);
  for (const [packageName, packageResults] of byPackage) {
    const floor = packageResults.find(result => result.probeKind === "floor");
    const head = packageResults.find(result => result.probeKind === "head");
    if (!floor || !head) continue;
    // The package version is the SAME in both probes -- what differs is the
    // Solid environment each was installed against. Reporting only
    // `floorVersion`/`headVersion` printed the package's own version twice and
    // told the reader nothing about the thing that actually changed, which is
    // the entire question this figure answers ("works on an early beta, fails
    // on a newer RC"). The Solid environment and the failing class come too.
    const entry = {
      package: packageName,
      family: floor.family,
      packageVersion: floor.version,
      floorSolid: floor.solid,
      headSolid: head.solid,
      floorClass: floor.class,
      headClass: head.class,
      floorOutcome: floor.outcome,
      headOutcome: head.outcome
    };
    const move = outcomeRank(head.outcome) - outcomeRank(floor.outcome);
    if (move < 0) worksOnFloorFailsAtHead.push(entry);
    else if (move > 0) failsOnFloorWorksAtHead.push(entry);
  }
  worksOnFloorFailsAtHead.sort((left, right) => compareStrings(left.package, right.package));
  failsOnFloorWorksAtHead.sort((left, right) => compareStrings(left.package, right.package));
  return { worksOnFloorFailsAtHead, failsOnFloorWorksAtHead };
}

// The shared-blocker figure is the whole point of computing this: a single
// missing dependency contract (e.g. `@tanstack/query-core`) can be the
// reported cause of many packages' failures, and the report's job is to say
// how many packages a single fix would actually unlock. Counting a package
// under every blocker module it happens to mention would overstate that —
// a package failing for two independent reasons is not "unlocked" by fixing
// either one alone, since the other failure remains. So a package whose
// `dependency-contract-obligation` failures name more than one distinct
// module is pulled out into `multiBlockerPackages` and excluded from every
// single-blocker's `estimatedPackagesUnlocked` count entirely, rather than
// being (over-)counted under each module it touches.
// Every class whose failure names a dependency whose contract is the thing
// standing in the way. Restricting this to `dependency-contract-obligation`
// hid the largest blockers in the ecosystem: the two consumer-side contract
// classes together accounted for 83 failures naming @solidjs/web and solid-js,
// and none of them reached this analysis -- so the report's
// "packages unlocked per blocker" figure, whose entire purpose is to rank what
// to fix first, listed only ten one-package blockers and missed the one worth
// dozens.
const BLOCKER_CLASSES = new Set([
  "dependency-contract-obligation",
  "package-contract-environment-dependent",
  "package-contract-export-missing"
]);

function buildSharedBlockers(results) {
  const obligationFailures = results.filter(
    result =>
      BLOCKER_CLASSES.has(result?.class) &&
      result?.outcome === "failure" &&
      typeof result?.detail?.module === "string" &&
      result.detail.module.length > 0
  );
  const byPackage = groupBy(obligationFailures, result => result.package);

  const unlockedByModule = new Map();
  const multiBlockerPackages = [];

  for (const [packageName, packageResults] of byPackage) {
    const modules = [...new Set(packageResults.map(result => result.detail.module))].sort(compareStrings);
    if (modules.length === 1) {
      const [module] = modules;
      const bucket = unlockedByModule.get(module);
      if (bucket) bucket.add(packageName);
      else unlockedByModule.set(module, new Set([packageName]));
    } else {
      multiBlockerPackages.push({ package: packageName, modules });
    }
  }

  const sharedBlockers = [...unlockedByModule.entries()].map(([module, packages]) => ({
    module,
    estimatedPackagesUnlocked: packages.size,
    packages: [...packages].sort(compareStrings)
  }));
  sharedBlockers.sort((left, right) => {
    if (left.estimatedPackagesUnlocked !== right.estimatedPackagesUnlocked) {
      return right.estimatedPackagesUnlocked - left.estimatedPackagesUnlocked;
    }
    return compareStrings(left.module, right.module);
  });
  multiBlockerPackages.sort((left, right) => compareStrings(left.package, right.package));

  return { sharedBlockers, multiBlockerPackages };
}

// ---------------------------------------------------------------------------
// Contract content: what the emitted contracts CLAIM, not whether they exist.
//
// Every other figure in this report is a reachability figure — a probe either
// produced a contract or it did not. This block is the only one that opens the
// document, and it answers the question a machine-verification scheme actually
// asks: under a scheme where an unknown stays uncertifiable, how clean is a
// typical package's generated draft?
//
// Three properties are kept strictly separate here, because collapsing any two
// of them would overstate how clean the corpus is:
//
// - an UNKNOWN is the `{"status":"unknown"}` sentinel on a claim domain;
// - a REFUSAL is a whole entrypoint the generator declined to describe;
// - a CLOSURE NOTE is a runtime-module closure that could not be fully
//   enumerated or hashed, so the contract cannot be byte-attested at all.
//
// `fullyProven` requires the absence of all three. A probe missing its content
// block entirely (a caller that did not supply one) is counted as UNMEASURED
// and named, never folded into either side of the ratio.
// ---------------------------------------------------------------------------

function addDomainCounts(target, source) {
  for (const domain of CLAIM_DOMAINS) {
    const value = source?.[domain];
    if (typeof value === "number") target[domain] += value;
  }
}

function addBehavioralRows(target, source) {
  for (const kind of BEHAVIORAL_ROW_KINDS) {
    const value = source?.[kind];
    if (typeof value === "number") target[kind] += value;
  }
}

function percentageOf(part, whole) {
  return whole > 0 ? Math.round((part / whole) * 10000) / 100 : null;
}

// The single claim domain most responsible for a contract's unknowns — or the
// honest refusal to name one.
//
// When every unknown export is unknown in ALL five domains, the five columns
// are one fact, not five, and picking the first of five equal counts would
// invent a cause ("mostly callbacks") that the data does not support. That
// case reports `all-domains` instead: the generator could say nothing at all
// about those exports, and which domain a reader looks at is irrelevant.
function dominantDomain(content) {
  const unknownByDomain = content?.unknownByDomain;
  const withUnknown = content?.exportsWithUnknown ?? 0;
  if (withUnknown > 0 && (content?.exportsAllDomainsUnknown ?? 0) === withUnknown) return "all-domains";
  let best = null;
  let bestCount = 0;
  // CLAIM_DOMAINS order is the tiebreak, so two domains at the same count
  // always resolve the same way across runs.
  for (const domain of CLAIM_DOMAINS) {
    const count = unknownByDomain?.[domain] ?? 0;
    if (count > bestCount) {
      best = domain;
      bestCount = count;
    }
  }
  return best;
}

function emptyContentAccumulator() {
  return {
    probesMeasured: 0,
    probesFullyProven: 0,
    probesWithUnknowns: 0,
    probesWithRefusals: 0,
    probesWithClosureNotes: 0,
    entrypointsEmitted: 0,
    entrypointsRefused: 0,
    exportsTotal: 0,
    exportsProven: 0,
    exportsWithUnknown: 0,
    exportsAllDomainsUnknown: 0,
    exportsUnknownOnlyInVariants: 0,
    exportsWithoutSummary: 0,
    unknownTotal: 0,
    unknownByDomain: emptyDomainCounts(),
    behavioralRows: emptyBehavioralRows(),
    closureNotes: 0,
    packageStates: new Map()
  };
}

function accumulateContent(accumulator, result) {
  const content = result?.contractContent;
  accumulator.probesMeasured += 1;
  accumulator.entrypointsEmitted += content.entrypointsEmitted ?? 0;
  accumulator.entrypointsRefused += content.entrypointsRefused ?? 0;
  accumulator.exportsTotal += content.exportsTotal ?? 0;
  accumulator.exportsProven += content.exportsProven ?? 0;
  accumulator.exportsWithUnknown += content.exportsWithUnknown ?? 0;
  accumulator.exportsAllDomainsUnknown += content.exportsAllDomainsUnknown ?? 0;
  accumulator.exportsUnknownOnlyInVariants += content.exportsUnknownOnlyInVariants ?? 0;
  accumulator.exportsWithoutSummary += content.exportsWithoutSummary ?? 0;
  accumulator.unknownTotal += content.unknownTotal ?? 0;
  addDomainCounts(accumulator.unknownByDomain, content.unknownByDomain);
  addBehavioralRows(accumulator.behavioralRows, content.behavioralRows);
  accumulator.closureNotes += content.closureNotes ?? 0;
  if (content.fullyProven) accumulator.probesFullyProven += 1;
  if ((content.exportsWithUnknown ?? 0) > 0) accumulator.probesWithUnknowns += 1;
  if ((content.entrypointsRefused ?? 0) > 0) accumulator.probesWithRefusals += 1;
  if ((content.closureNotes ?? 0) > 0) accumulator.probesWithClosureNotes += 1;

  // A package is only fully proven when EVERY probe that produced a contract
  // for it is. A package clean under Solid 1.x and full of unknowns at a
  // Solid 2 head is not a clean package, and reporting it as one would hide
  // exactly the divergence the floor/head model exists to surface.
  const previous = accumulator.packageStates.get(result.package);
  accumulator.packageStates.set(result.package, (previous ?? true) && Boolean(content.fullyProven));
}

function finalizeContentAccumulator(accumulator) {
  const packages = [...accumulator.packageStates.values()];
  const { packageStates, ...counts } = accumulator;
  return {
    ...counts,
    packagesMeasured: packages.length,
    packagesFullyProven: packages.filter(Boolean).length,
    exportsProvenPercentage: percentageOf(accumulator.exportsProven, accumulator.exportsTotal),
    probesFullyProvenPercentage: percentageOf(accumulator.probesFullyProven, accumulator.probesMeasured),
    packagesFullyProvenPercentage: percentageOf(
      packages.filter(Boolean).length,
      packages.length
    )
  };
}

// The N probes carrying the most unknown claims, with the domain that
// dominates each. This is the "what would a verification scheme actually have
// to answer first" list, so it is ranked by absolute unknown count rather than
// by ratio: a 3-of-4 package is a worse ratio than a 60-of-900 one and a far
// smaller amount of work.
function buildTopUnknownProbes(measured, limit = 15) {
  return measured
    .filter(result => (result.contractContent.unknownTotal ?? 0) > 0)
    .map(result => ({
      probeId: result.probeId,
      package: result.package,
      version: result.version,
      family: result.family,
      solidTarget: result.solidTarget,
      exportsTotal: result.contractContent.exportsTotal ?? 0,
      exportsWithUnknown: result.contractContent.exportsWithUnknown ?? 0,
      exportsAllDomainsUnknown: result.contractContent.exportsAllDomainsUnknown ?? 0,
      exportsUnknownOnlyInVariants: result.contractContent.exportsUnknownOnlyInVariants ?? 0,
      unknownTotal: result.contractContent.unknownTotal ?? 0,
      unknownByDomain: { ...result.contractContent.unknownByDomain },
      dominantDomain: dominantDomain(result.contractContent)
    }))
    .sort((left, right) => {
      if (left.unknownTotal !== right.unknownTotal) return right.unknownTotal - left.unknownTotal;
      return compareStrings(left.probeId, right.probeId);
    })
    .slice(0, limit);
}

function buildContractContentSummary(results) {
  const produced = contractProducing(results);
  const measured = produced.filter(result => result?.contractContent?.measured === true);
  const unmeasured = produced.filter(result => result?.contractContent?.measured !== true);

  const overall = emptyContentAccumulator();
  const byFamily = new Map(FAMILIES.map(family => [family.id, emptyContentAccumulator()]));
  for (const result of measured) {
    accumulateContent(overall, result);
    const familyAccumulator = byFamily.get(result.family);
    if (familyAccumulator) accumulateContent(familyAccumulator, result);
  }

  return {
    ...finalizeContentAccumulator(overall),
    // Named, never counted: a probe whose contract could not be read is a hole
    // in the measurement, and a hole reported as zero unknowns is the one
    // wrong answer this whole block exists to avoid.
    unmeasuredProbes: unmeasured
      .map(result => ({
        probeId: result.probeId,
        package: result.package,
        note: result.contractContent?.note ?? "no contract content recorded"
      }))
      .sort((left, right) => compareStrings(left.probeId, right.probeId)),
    families: FAMILIES.map(family => ({
      family: family.id,
      label: family.label,
      ...finalizeContentAccumulator(byFamily.get(family.id))
    })),
    topUnknownProbes: buildTopUnknownProbes(measured)
  };
}

function summarizeOutcomes(results) {
  const successes = results.filter(result => result.outcome === "success").length;
  const partials = results.filter(result => result.outcome === "partial-success").length;
  const failures = results.filter(result => result.outcome === "failure").length;
  const total = results.length;
  // Counted explicitly rather than as `total - successes`: with three
  // outcomes that subtraction silently reported every partial contract as a
  // failure.
  return {
    probeCount: total,
    successCount: successes,
    partialCount: partials,
    failureCount: failures,
    successRate: computeSuccessRate(successes, total)
  };
}

function buildFamilyComparison(solid1Results, solid2Results) {
  return FAMILIES.map(family => ({
    family: family.id,
    label: family.label,
    solid1: summarizeOutcomes(solid1Results.filter(result => result.family === family.id)),
    solid2: summarizeOutcomes(solid2Results.filter(result => result.family === family.id))
  }));
}

// Records metadata gaps in the *inputs*, not measurement outcomes — a
// missing checker binary path or a success probe with no
// generatedEntrypoints/checklistItems is a hole in what the caller gave this
// module, and hiding it as "0" would misreport a known-unknown as a
// measured zero.
function buildUnavailableMetadata({ checker, manifest, results }) {
  const notes = [];
  if (!checker?.nativeBin) notes.push("checker.nativeBin was not provided to buildReport");
  if (!checker?.typeFactsBin) notes.push("checker.typeFactsBin was not provided to buildReport");
  if (!manifest?.generatedAt) notes.push("manifest.generatedAt was not provided");

  // Both hold for a partial contract too: it wrote a contract and a review
  // plan, so a missing count is the same known-unknown it would be on a
  // complete one.
  const produced = contractProducing(results);

  const missingGenerated = produced.filter(
    result => result.generatedEntrypoints === null || result.generatedEntrypoints === undefined
  ).length;
  if (missingGenerated > 0) notes.push(`${missingGenerated} contract-producing probe(s) missing generatedEntrypoints`);

  const missingChecklist = produced.filter(
    result => result.checklistItems === null || result.checklistItems === undefined
  ).length;
  if (missingChecklist > 0) notes.push(`${missingChecklist} contract-producing probe(s) missing checklistItems`);

  const missingRefused = results.filter(
    result =>
      result.outcome === "partial-success" &&
      (result.refusedEntrypoints === null || result.refusedEntrypoints === undefined)
  ).length;
  if (missingRefused > 0) notes.push(`${missingRefused} partial probe(s) missing refusedEntrypoints`);

  return notes.sort(compareStrings);
}

function buildBaselineComparison(baseline, currentResults) {
  if (!baseline) return { provided: false };

  const baselineResults = Array.isArray(baseline) ? baseline : Array.isArray(baseline?.results) ? baseline.results : [];
  const baselineByProbe = new Map(baselineResults.map(result => [result.probeId, result]));
  const currentByProbe = new Map(currentResults.map(result => [result.probeId, result]));

  const regressions = [];
  const fixes = [];
  const newProbes = [];
  const removedProbes = [];

  for (const [probeId, current] of currentByProbe) {
    const prior = baselineByProbe.get(probeId);
    if (!prior) {
      newProbes.push(probeId);
      continue;
    }
    // Compared by direction on the ordered outcome scale, not against `success`
    // specifically. A probe that used to emit a complete contract and now emits
    // a partial one lost entrypoints and is a regression; so is one that used to
    // emit a partial contract and now emits none at all -- a move the old
    // `success`/`not success` test matched on neither side, which is precisely
    // the run where the contract disappeared.
    const move = outcomeRank(current.outcome) - outcomeRank(prior.outcome);
    if (move < 0) {
      regressions.push({
        probeId,
        package: current.package,
        previousClass: prior.class,
        currentClass: current.class,
        previousOutcome: prior.outcome,
        currentOutcome: current.outcome
      });
    } else if (move > 0) {
      fixes.push({
        probeId,
        package: current.package,
        previousClass: prior.class,
        currentClass: current.class,
        previousOutcome: prior.outcome,
        currentOutcome: current.outcome
      });
    }
  }
  for (const probeId of baselineByProbe.keys()) {
    if (!currentByProbe.has(probeId)) removedProbes.push(probeId);
  }

  regressions.sort((left, right) => compareStrings(left.probeId, right.probeId));
  fixes.sort((left, right) => compareStrings(left.probeId, right.probeId));
  newProbes.sort(compareStrings);
  removedProbes.sort(compareStrings);

  return {
    provided: true,
    regressionCount: regressions.length,
    fixCount: fixes.length,
    regressions,
    fixes,
    newProbes,
    removedProbes
  };
}

/**
 * Builds the deterministic, JSON-serializable ecosystem benchmark report.
 *
 * `checker` is not one of the parameters INTERFACES.md names for this
 * function, but the report's documented top-level shape requires
 * `checker: { nativeBin, typeFactsBin }` and no other input carries that
 * data (it is not part of a probe result or the manifest) — so it is
 * accepted here as an additional, optional destructured field that
 * `run.mjs` can supply; omitting it changes nothing for a caller using only
 * the five documented parameters.
 */
// One line naming what this report covers. A full run says so plainly; a
// filtered one names its filters and how many probes it actually ran, so the
// number in the manifest line above is never mistaken for the run's own.
function describeScope(scope) {
  const ran = scope?.probesRun ?? 0;
  if (!scope || scope.kind === "full") {
    return `full corpus (${ran} probes run)`;
  }
  const filters = [];
  if (scope.sentinel) filters.push("sentinel subset");
  for (const family of scope.families ?? []) filters.push(`family ${family}`);
  for (const target of scope.solidTargets ?? []) filters.push(`solid${target}`);
  return (
    `PARTIAL -- ${filters.join(", ")} (${ran} probes run). ` +
    "Not comparable to a full-corpus run."
  );
}

export function buildReport({
  manifest,
  results,
  startedAt,
  finishedAt,
  baseline = null,
  checker = null,
  scope = null
}) {
  const everyResult = Array.isArray(results) ? results.slice() : [];
  // Supplemental rows are unofficial forks and lookalikes. They are reported,
  // but strictly on their own: folding them into a family's probe count or
  // success rate would attribute a fork's behavior to the official project.
  // A fork failing says nothing about @kobalte/core.
  const supplementalResults = everyResult.filter(result => result?.status === "supplemental");
  const allResults = everyResult.filter(result => result?.status !== "supplemental");
  const solid1Results = allResults.filter(result => result?.solidTarget === "solid1");
  const solid2Results = allResults.filter(result => result?.solidTarget === "solid2");

  const solid1FamilySections = FAMILIES.map(family => buildFamilySection(family, solid1Results));
  const solid2FamilySections = FAMILIES.map(family => buildFamilySection(family, solid2Results));

  const { betaOnlyPackages, rcOnlyPackages } = buildChannelOnlyLists(solid2Results);
  const { worksOnFloorFailsAtHead, failsOnFloorWorksAtHead } = buildFloorHeadDiffs(solid2Results);
  const { sharedBlockers, multiBlockerPackages } = buildSharedBlockers(allResults);

  const stats = manifest ? manifestStats(manifest) : { rowCount: 0, probeCount: 0 };

  return {
    schemaVersion: SCHEMA_VERSION,
    startedAt,
    finishedAt,
    durationMs: computeDurationMs(startedAt, finishedAt),
    checker: {
      nativeBin: checker?.nativeBin ?? null,
      typeFactsBin: checker?.typeFactsBin ?? null
    },
    manifest: {
      generatedAt: manifest?.generatedAt ?? null,
      rowCount: stats.rowCount,
      probeCount: stats.probeCount
    },
    // Which subset this run actually covered. `manifest` above describes the
    // corpus the run was selected *from*, which is not the same number: a
    // sentinel run reports 417 manifest probes while containing 23 results.
    // Recording the scope is what makes a partial report self-identifying
    // instead of something a reader has to infer from the row counts.
    scope: {
      kind: scope?.kind ?? "full",
      sentinel: scope?.sentinel ?? false,
      families: scope?.families ?? [],
      solidTargets: scope?.solidTargets ?? [],
      includeSupplemental: scope?.includeSupplemental ?? false,
      probesRun: everyResult.length
    },
    solid1: {
      families: solid1FamilySections,
      totals: buildTotals(solid1Results)
    },
    solid2: {
      families: solid2FamilySections,
      totals: buildTotals(solid2Results),
      betaOnlyPackages,
      rcOnlyPackages,
      worksOnFloorFailsAtHead,
      failsOnFloorWorksAtHead
    },
    // Reported, never mixed in: forks and lookalikes get their own section so
    // a reader can see what was found without any of it landing in an
    // official family's numbers.
    supplemental: {
      probeCount: supplementalResults.length,
      results: supplementalResults
        .slice()
        .sort((left, right) =>
          left.probeId < right.probeId ? -1 : left.probeId > right.probeId ? 1 : 0
        )
    },
    combined: {
      topFailureSignatures: buildFailureGroups(allResults),
      partialContracts: buildPartialContracts(allResults),
      // Additive: every field above and below describes generation
      // reachability, and this one alone describes the content of what was
      // generated. No existing consumer reads it, and it changes none of them.
      contractContent: buildContractContentSummary(allResults),
      sharedBlockers,
      multiBlockerPackages,
      familyComparison: buildFamilyComparison(solid1Results, solid2Results),
      workerTimings: buildWorkerTimings(allResults),
      durationMs: computeDurationMs(startedAt, finishedAt),
      unavailableMetadata: buildUnavailableMetadata({ checker, manifest, results: allResults }),
      // Copied verbatim: discovery already owns curating and ordering this
      // list (see lib/manifest.mjs's document shape); re-sorting it here
      // would risk disagreeing with the manifest a reader has open next to
      // this report.
      discoveryLimitations: Array.isArray(manifest?.limitations) ? [...manifest.limitations] : [],
      baseline: buildBaselineComparison(baseline, allResults)
    },
    results: allResults.slice().sort(comparePackageThenProbe)
  };
}

function truncateForMarkdown(text, limit = MARKDOWN_STDERR_LIMIT) {
  const normalized = (typeof text === "string" ? text : "").replace(/\s+/g, " ").trim();
  if (normalized.length <= limit) return { text: normalized, truncated: false };
  return { text: `${normalized.slice(0, limit)}...`, truncated: true };
}

function formatRate(rate) {
  if (!rate || rate.total === 0) return "0/0 (no probes run)";
  return `${rate.successes}/${rate.total} (${rate.percentage}%)`;
}

function renderFamilySection(section) {
  const lines = [];
  lines.push(`### ${section.label}`);
  lines.push("");
  lines.push(`- Compatible packages: ${section.compatiblePackageCount}`);
  lines.push(`- Probes run: ${section.probeCount}`);
  lines.push(`- Declared entrypoints: ${section.declaredEntrypoints}`);
  lines.push(`- Generated entrypoints: ${section.generatedEntrypoints}`);
  lines.push(`- Refused entrypoints (partial contracts): ${section.refusedEntrypoints ?? 0}`);
  lines.push(`- Success (complete contracts): ${formatRate(section.successRate)}`);
  lines.push(`- Partial contracts: ${section.partialCount ?? 0}`);
  lines.push(`- Failures: ${section.failureCount}`);
  lines.push("");

  if (section.results.length > 0) {
    lines.push("| Package | Version | Probe | Outcome | Class |");
    lines.push("| --- | --- | --- | --- | --- |");
    for (const result of section.results) {
      lines.push(`| ${result.package} | ${result.version} | ${result.probeKind} | ${result.outcome} | ${result.class} |`);
    }
    lines.push("");
  }

  if (section.failureGroups.length > 0) {
    lines.push("Failure groups:");
    for (const group of section.failureGroups) {
      lines.push(`- ${group.count}x ${group.class}: ${group.signature} (packages: ${group.packages.join(", ")})`);
    }
    lines.push("");
  }

  const failing = section.results.filter(result => result.outcome === "failure");
  if (failing.length > 0) {
    lines.push("Failure details:");
    for (const result of failing) {
      const { text, truncated } = truncateForMarkdown(result.stderr);
      const truncatedNote = truncated ? " _(stderr truncated for readability)_" : "";
      lines.push(`- **${result.package}@${result.version}** (${result.probeKind}, ${result.class}): ${text}${truncatedNote}`);
    }
    lines.push("");
  }

  return lines.join("\n");
}

function renderChannelLists(solid2) {
  const lines = [];
  lines.push("### Beta-only packages");
  lines.push("");
  if (solid2.betaOnlyPackages.length === 0) lines.push("None.");
  else for (const entry of solid2.betaOnlyPackages) lines.push(`- ${entry.package}@${entry.version} (${entry.family})`);
  lines.push("");

  lines.push("### RC-only packages");
  lines.push("");
  if (solid2.rcOnlyPackages.length === 0) lines.push("None.");
  else for (const entry of solid2.rcOnlyPackages) lines.push(`- ${entry.package}@${entry.version} (${entry.family})`);
  lines.push("");

  return lines.join("\n");
}

// The outcome transition is printed on every row because these lists are now
// ordered-scale moves: `success -> partial-success` belongs under "worse at
// head" without meaning the package stopped working there.
function renderFloorHeadDiffs(solid2) {
  const lines = [];
  lines.push("### Worse at head than at floor");
  lines.push("");
  if (solid2.worksOnFloorFailsAtHead.length === 0) lines.push("None.");
  else {
    for (const entry of solid2.worksOnFloorFailsAtHead) {
      lines.push(`- ${entry.package} (${entry.family}): ${entry.floorOutcome} -> ${entry.headOutcome}`);
    }
  }
  lines.push("");

  lines.push("### Better at head than at floor");
  lines.push("");
  if (solid2.failsOnFloorWorksAtHead.length === 0) lines.push("None.");
  else {
    for (const entry of solid2.failsOnFloorWorksAtHead) {
      lines.push(`- ${entry.package} (${entry.family}): ${entry.floorOutcome} -> ${entry.headOutcome}`);
    }
  }
  lines.push("");

  return lines.join("\n");
}

function formatCount(part, whole, percentage) {
  if (!whole) return `${part}/0 (nothing measured)`;
  return `${part}/${whole} (${percentage}%)`;
}

// The claim-content section. Deliberately separate from every reachability
// section above it, and deliberately carrying its caveats inline: read without
// them, "84% of exports proven" sounds like a statement about the ecosystem
// when it is a statement about an unreviewed generated draft.
function renderContractContentSection(content) {
  const lines = [];
  lines.push("## Contract content (what the emitted contracts claim)");
  lines.push("");
  if (!content || content.probesMeasured === 0) {
    lines.push("No contract content measured.");
    lines.push("");
    return lines.join("\n");
  }

  lines.push(
    `- Contracts measured: ${content.probesMeasured} probe(s) across ${content.packagesMeasured} package(s)`
  );
  lines.push(
    `- Probes fully proven (no unknown claim, no refused entrypoint, no closure note): ` +
      `${formatCount(content.probesFullyProven, content.probesMeasured, content.probesFullyProvenPercentage)}`
  );
  lines.push(
    `- Packages fully proven (every one of their probes): ` +
      `${formatCount(content.packagesFullyProven, content.packagesMeasured, content.packagesFullyProvenPercentage)}`
  );
  lines.push(`- Probes with at least one unknown claim: ${content.probesWithUnknowns}`);
  lines.push(`- Probes with at least one refused entrypoint: ${content.probesWithRefusals}`);
  lines.push(`- Probes with at least one closure note: ${content.probesWithClosureNotes}`);
  lines.push(
    `- Exports proven: ${formatCount(content.exportsProven, content.exportsTotal, content.exportsProvenPercentage)}` +
      ` (with unknown: ${content.exportsWithUnknown}, without a summary: ${content.exportsWithoutSummary})`
  );
  lines.push(
    `- Of those unknown exports: ${content.exportsAllDomainsUnknown} unknown in ALL five domains ` +
      `(the generator said nothing about them at all), ` +
      `${content.exportsUnknownOnlyInVariants} unknown only inside a conditional variant ` +
      "(the default resolution is fully claimed)"
  );
  lines.push(
    `- Entrypoints: ${content.entrypointsEmitted} emitted, ${content.entrypointsRefused} refused`
  );
  lines.push(`- Closure notes (block byte-attested verification): ${content.closureNotes}`);
  lines.push("");

  lines.push("### Unknown claims by domain");
  lines.push("");
  lines.push("| Domain | Exports carrying an unknown |");
  lines.push("| --- | --- |");
  for (const domain of CLAIM_DOMAINS) {
    lines.push(`| ${domain} | ${content.unknownByDomain?.[domain] ?? 0} |`);
  }
  lines.push(`| **total** | **${content.unknownTotal}** |`);
  lines.push("");
  lines.push(
    "Read the five columns together, not separately: " +
      `${content.exportsAllDomainsUnknown} of the ${content.exportsWithUnknown} unknown exports are unknown in ` +
      "every domain at once, so most of each column is the same exports counted five times."
  );
  lines.push("");

  lines.push("### Positive behavioral rows (what a probe step would have to drive)");
  lines.push("");
  lines.push("| Row kind | Count |");
  lines.push("| --- | --- |");
  for (const kind of BEHAVIORAL_ROW_KINDS) {
    lines.push(`| ${kind} | ${content.behavioralRows?.[kind] ?? 0} |`);
  }
  lines.push("");

  lines.push("### Contract content by family");
  lines.push("");
  lines.push(
    "| Family | Contracts | Fully proven | With unknowns | With refusals | Exports proven | Unknown claims |"
  );
  lines.push("| --- | --- | --- | --- | --- | --- | --- |");
  for (const family of content.families ?? []) {
    lines.push(
      `| ${family.label} | ${family.probesMeasured} | ` +
        `${formatCount(family.probesFullyProven, family.probesMeasured, family.probesFullyProvenPercentage)} | ` +
        `${family.probesWithUnknowns} | ${family.probesWithRefusals} | ` +
        `${formatCount(family.exportsProven, family.exportsTotal, family.exportsProvenPercentage)} | ` +
        `${family.unknownTotal} |`
    );
  }
  lines.push("");

  lines.push("### Most unknown claims");
  lines.push("");
  if (!content.topUnknownProbes || content.topUnknownProbes.length === 0) {
    lines.push("None.");
  } else {
    lines.push(
      "| Package | Solid | Unknown claims | Exports with unknown / total | All five domains | Variant-only | Dominant cause |"
    );
    lines.push("| --- | --- | --- | --- | --- | --- | --- |");
    for (const entry of content.topUnknownProbes) {
      lines.push(
        `| ${entry.package}@${entry.version} | ${entry.solidTarget} | ${entry.unknownTotal} | ` +
          `${entry.exportsWithUnknown}/${entry.exportsTotal} | ${entry.exportsAllDomainsUnknown} | ` +
          `${entry.exportsUnknownOnlyInVariants} | ${entry.dominantDomain ?? "none"} |`
      );
    }
  }
  lines.push("");

  if (content.unmeasuredProbes && content.unmeasuredProbes.length > 0) {
    lines.push("### Contracts that could not be read");
    lines.push("");
    for (const entry of content.unmeasuredProbes) lines.push(`- ${entry.probeId}: ${entry.note}`);
    lines.push("");
  }

  lines.push(
    "These figures describe the GENERATED DRAFT, not consumer findings. An unknown claim " +
      "becomes a finding only when a consumer actually touches that surface, so a package with " +
      "many unknowns on exports nobody imports costs a real project nothing. Nothing here has " +
      "been reviewed or probed: every claim counted as proven is still inferred evidence " +
      "awaiting review, and a closure note means the contract cannot be byte-attested at all."
  );
  lines.push("");

  return lines.join("\n");
}

function renderCombinedSection(combined) {
  const lines = [];

  const timings = combined.workerTimings;
  lines.push("### Worker timings");
  lines.push("");
  lines.push(`- Worker time: ${timings.totalDurationMs} ms`);
  lines.push(
    `- Phases: install ${timings.installDurationMs} ms, generation ${timings.generationDurationMs} ms, harness ${timings.harnessDurationMs} ms`
  );
  lines.push("");

  lines.push("### Top failure signatures");
  lines.push("");
  if (combined.topFailureSignatures.length === 0) lines.push("None.");
  else {
    for (const group of combined.topFailureSignatures) {
      lines.push(`- ${group.count}x ${group.class}: ${group.signature} (packages: ${group.packages.join(", ")})`);
    }
  }
  lines.push("");

  lines.push("### Partial contracts");
  lines.push("");
  if (!combined.partialContracts || combined.partialContracts.length === 0) lines.push("None.");
  else {
    for (const entry of combined.partialContracts) {
      lines.push(
        `- ${entry.package}@${entry.version} (${entry.family}): ${entry.generatedEntrypoints ?? "unknown"} entrypoint(s) generated, ${entry.refusedEntrypoints ?? "unknown"} refused`
      );
    }
  }
  lines.push("");

  lines.push("### Shared dependency blockers");
  lines.push("");
  if (combined.sharedBlockers.length === 0) lines.push("None.");
  else {
    for (const blocker of combined.sharedBlockers) {
      lines.push(
        `- ${blocker.module}: estimated ${blocker.estimatedPackagesUnlocked} package(s) unlocked (${blocker.packages.join(", ")})`
      );
    }
  }
  lines.push("");

  lines.push("### Multi-blocker packages");
  lines.push("");
  if (combined.multiBlockerPackages.length === 0) lines.push("None.");
  else for (const entry of combined.multiBlockerPackages) lines.push(`- ${entry.package}: ${entry.modules.join(", ")}`);
  lines.push("");

  lines.push("### Family comparison (Solid 1.x vs Solid 2.x)");
  lines.push("");
  // "success" here is a COMPLETE contract; a partial one is counted in the
  // denominator, never the numerator.
  lines.push("| Family | Solid 1.x complete/total | Solid 2.x complete/total |");
  lines.push("| --- | --- | --- |");
  for (const entry of combined.familyComparison) {
    lines.push(`| ${entry.label} | ${formatRate(entry.solid1.successRate)} | ${formatRate(entry.solid2.successRate)} |`);
  }
  lines.push("");

  lines.push("### Discovery limitations");
  lines.push("");
  if (combined.discoveryLimitations.length === 0) lines.push("None recorded.");
  else for (const item of combined.discoveryLimitations) lines.push(`- ${item}`);
  lines.push("");

  lines.push("### Unavailable metadata");
  lines.push("");
  if (combined.unavailableMetadata.length === 0) lines.push("None.");
  else for (const item of combined.unavailableMetadata) lines.push(`- ${item}`);
  lines.push("");

  lines.push("### Baseline comparison");
  lines.push("");
  if (!combined.baseline?.provided) {
    lines.push("No baseline supplied.");
  } else {
    lines.push(`- Regressions: ${combined.baseline.regressionCount}`);
    for (const entry of combined.baseline.regressions) {
      lines.push(`  - ${entry.probeId}: ${entry.previousClass} -> ${entry.currentClass}`);
    }
    lines.push(`- Fixes: ${combined.baseline.fixCount}`);
    for (const entry of combined.baseline.fixes) {
      // The destination is the probe's actual class, not a hardcoded `success`:
      // a probe that came back as a *partial* contract improved without
      // becoming complete, and printing "-> success" would overstate it.
      lines.push(`  - ${entry.probeId}: ${entry.previousClass} -> ${entry.currentClass}`);
    }
    if (combined.baseline.newProbes.length > 0) lines.push(`- New probes: ${combined.baseline.newProbes.join(", ")}`);
    if (combined.baseline.removedProbes.length > 0) {
      lines.push(`- Removed probes: ${combined.baseline.removedProbes.join(", ")}`);
    }
  }
  lines.push("");

  return lines.join("\n");
}

export function renderMarkdown(report) {
  const lines = [];
  lines.push("# Ecosystem Benchmark Report");
  lines.push("");
  lines.push(`- Started: ${report.startedAt}`);
  lines.push(`- Finished: ${report.finishedAt}`);
  lines.push(`- Duration: ${report.durationMs === null ? "unknown" : `${report.durationMs} ms`}`);
  lines.push(`- Checker native binary: ${report.checker?.nativeBin ?? "unknown"}`);
  lines.push(`- Type Facts binary: ${report.checker?.typeFactsBin ?? "unknown"}`);
  lines.push(
    `- Manifest generated at: ${report.manifest?.generatedAt ?? "unknown"} ` +
      `(rows: ${report.manifest?.rowCount ?? 0}, probes: ${report.manifest?.probeCount ?? 0})`
  );
  lines.push(`- Scope: ${describeScope(report.scope)}`);
  lines.push("");

  lines.push("## Solid 1.x");
  lines.push("");
  for (const section of report.solid1.families) lines.push(renderFamilySection(section));
  lines.push(
    `**Solid 1.x totals:** ${formatRate(report.solid1.totals.successRate)} complete, ` +
      `${report.solid1.totals.partialCount ?? 0} partial, ${report.solid1.totals.failureCount} failed`
  );
  lines.push("");

  lines.push("## Solid 2.x");
  lines.push("");
  for (const section of report.solid2.families) lines.push(renderFamilySection(section));
  lines.push(
    `**Solid 2.x totals:** ${formatRate(report.solid2.totals.successRate)} complete, ` +
      `${report.solid2.totals.partialCount ?? 0} partial, ${report.solid2.totals.failureCount} failed`
  );
  lines.push("");
  lines.push(renderChannelLists(report.solid2));
  lines.push(renderFloorHeadDiffs(report.solid2));

  lines.push(renderContractContentSection(report.combined?.contractContent));

  lines.push("## Combined");
  lines.push("");
  lines.push(renderCombinedSection(report.combined));

  return lines.join("\n");
}

// Threshold rules recognized:
//   { global: { minSuccessCount }, families: { <familyId>: { minSuccessCount } } }
// A family absent from `thresholds.families` has no threshold at all — it is
// simply never visited by the loop below, so it can never contribute a
// failure. That is deliberate: a threshold file only expresses opinions
// about the families it names.
export function evaluateThresholds(report, thresholds = {}) {
  const failures = [];

  const globalMinimum = thresholds?.global?.minSuccessCount;
  if (typeof globalMinimum === "number") {
    const actual = (report.solid1?.totals?.successCount ?? 0) + (report.solid2?.totals?.successCount ?? 0);
    if (actual < globalMinimum) {
      failures.push({ scope: "global", metric: "successCount", actual, minimum: globalMinimum });
    }
  }

  const familyThresholds = thresholds?.families ?? {};
  for (const [familyId, rule] of Object.entries(familyThresholds)) {
    const minimum = rule?.minSuccessCount;
    if (typeof minimum !== "number") continue;

    const comparison = report.combined?.familyComparison?.find(entry => entry.family === familyId);
    if (!comparison) {
      // A threshold naming a family id the report never produced (a typo,
      // or a family retired from FAMILIES) is a configuration problem, not
      // silence — it must not be treated the same as "no threshold".
      failures.push({
        scope: `family:${familyId}`,
        metric: "successCount",
        actual: 0,
        minimum,
        note: "family not present in report"
      });
      continue;
    }

    const actual = (comparison.solid1?.successCount ?? 0) + (comparison.solid2?.successCount ?? 0);
    if (actual < minimum) {
      failures.push({ scope: `family:${familyId}`, metric: "successCount", actual, minimum });
    }
  }

  failures.sort((left, right) => compareStrings(left.scope, right.scope));
  return { ok: failures.length === 0, failures };
}
