#!/usr/bin/env bun
// Network-enabled discovery CLI: the only part of the ecosystem benchmark
// that is allowed to touch the npm registry on its own account. Everything
// downstream (run.mjs) reads the manifest this writes and never reaches the
// network except to `bun install` the exact pinned versions the manifest
// already recorded.
//
// The pipeline itself is one injectable async function, `discover`, so it
// can be exercised offline in discover.test.mjs against a fake `fetchImpl`.
// `now` is threaded through as a parameter (never `new Date()` inside the
// pipeline) so two runs against the same registry snapshot serialize to the
// same bytes — the determinism rule in INTERFACES.md applies to this file
// exactly as it does to every other module.

import { parseArgs } from "node:util";
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { Registry } from "./lib/registry.mjs";
import { FAMILIES, SOLID_RUNTIME_PACKAGES, AUDITED_SOLID_1, classifyPackage, familyById, familyOrder } from "./lib/families.mjs";
import { solidReleaseCatalog, selectRow } from "./lib/select.mjs";
import { MANIFEST_SCHEMA_VERSION, validateManifest, serializeManifest, sortRows, diffManifests } from "./lib/manifest.mjs";

const SOLID_TARGETS = ["solid1", "solid2"];
const SCRIPT_DIR = dirname(fileURLToPath(import.meta.url));
const DEFAULT_OUTPUT = resolve(SCRIPT_DIR, "manifest.json");
const DEFAULT_SENTINEL = resolve(SCRIPT_DIR, "sentinel.json");

/**
 * The whole discovery pipeline, injectable and offline-testable:
 * - `registry` is a Registry instance (tests inject its `fetchImpl`).
 * - `families` defaults to the real FAMILIES table but may be a subset (the
 *   CLI's `--family` flag) to restrict which scopes/packages/search terms
 *   get enumerated this run. Classification itself always goes through the
 *   real `classifyPackage`/`familyById` (never through this parameter),
 *   because a package's family is a fact about the package, not about which
 *   families the caller happened to ask this run to look at.
 * - `now` is the ISO timestamp for `generatedAt`; the pipeline never calls
 *   `new Date()` itself so identical inputs always serialize identically.
 * - `limitations` seeds the returned limitations list (e.g. a `--family`
 *   restricted run wants to keep limitations recorded by a previous run for
 *   the families it isn't touching); the pipeline only ever adds to it.
 *
 * Returns a manifest object covering exactly the requested `families` — the
 * CLI wrapper is responsible for merging a restricted run back into the
 * manifest already on disk before validating and writing.
 */
export async function discover({ registry, families = FAMILIES, now, limitations: seedLimitations = [] }) {
  if (typeof now !== "string" || now === "") {
    throw new Error("discover() requires an injected ISO timestamp string `now`");
  }

  const limitations = new Set(seedLimitations);
  const candidateNames = new Set();

  // Step 1: official membership. The org listing endpoint is authoritative
  // (see families.mjs), so anything it returns is trusted as official; an
  // explicit `packages` entry (a standalone, unscoped name like "solid-js")
  // is trusted the same way without a network round trip.
  for (const family of families) {
    for (const scope of family.scopes) {
      const names = await registry.orgPackages(scope);
      if (names.length === 0) {
        limitations.add(`org listing for scope "${scope}" (family "${family.id}") returned no packages`);
      }
      for (const name of names) candidateNames.add(name);
    }
    for (const name of family.packages) candidateNames.add(name);
  }

  // Step 2: supplemental fork detection ONLY. `searchAll` is a
  // relevance-ranked text search, not an authoritative listing, so a hit
  // here never promotes a package into the official family on its own —
  // `classifyPackage` below still decides official-vs-supplemental purely
  // from the name's actual npm scope. A genuinely official package that an
  // org listing happened to miss is still classified official from its
  // scope; every other hit (a fork, a clone, an unrelated package that just
  // mentions the family's name) falls through to supplemental. That is the
  // entire reason this call exists only for families with `searchTerms`.
  for (const family of families) {
    for (const term of family.searchTerms) {
      const found = await registry.searchAll(term);
      for (const name of found) candidateNames.add(name);
    }
  }

  // Classify every candidate now, once. A name that matches no family at all
  // (unrelated search noise, e.g. an unrelated package that happens to share
  // an org listing quirk) is dropped here rather than carried further.
  const classified = new Map();
  for (const name of candidateNames) {
    const result = classifyPackage(name);
    if (result) classified.set(name, result);
  }

  // The Solid runtime packages are always fetched, even under a `--family`
  // restriction that excludes official-solid entirely, because every solid2
  // selection for every OTHER family depends on the shared release catalog
  // built from their packuments.
  const packumentTargets = new Set([...classified.keys(), ...SOLID_RUNTIME_PACKAGES]);
  const sortedTargets = [...packumentTargets].sort();

  const packumentEntries = await registry.mapConcurrent(sortedTargets, async name => [name, await registry.packument(name)]);
  const packuments = new Map(packumentEntries);

  for (const name of sortedTargets) {
    const packument = packuments.get(name) ?? null;
    if (packument === null) {
      // A registry gap we actually hit: a 404/missing packument for a name
      // an authoritative listing or an explicit family entry told us to
      // expect. Never let this pass silently — selectRow below turns it
      // into a `not-published` exclusion, but the *reason discovery hit a
      // gap at all* belongs in limitations too.
      limitations.add(`packument for "${name}" is unavailable (registry returned nothing for it)`);
    } else if (!packument.versions || Object.keys(packument.versions).length === 0) {
      limitations.add(`"${name}" has zero published versions`);
    }
  }

  const runtimePackuments = new Map(SOLID_RUNTIME_PACKAGES.map(name => [name, packuments.get(name) ?? null]));
  const catalog = solidReleaseCatalog(runtimePackuments);

  const rows = [];
  const exclusions = [];
  const supplemental = [];

  const recordUnparsed = (packageName, solidTarget, unparsedRanges) => {
    for (const entry of unparsedRanges ?? []) {
      limitations.add(
        `${packageName} (${solidTarget}) declares an unparsed range for ${entry.package}: ${JSON.stringify(entry.range)}`
      );
    }
  };

  for (const name of [...classified.keys()].sort()) {
    const { family: familyId, status } = classified.get(name);
    // The real family object (not just its id) is required here: selectRow's
    // `requireSolidDependency` contract (the TanStack rule) reads it off the
    // family, and a bare id would silently default that flag to false.
    const family = familyById(familyId);
    const packument = packuments.get(name) ?? null;

    for (const solidTarget of SOLID_TARGETS) {
      const result = selectRow({ packageName: name, packument, family, status, solidTarget, catalog, auditedSolid1: AUDITED_SOLID_1 });
      if (result.kind === "row") {
        recordUnparsed(name, solidTarget, result.row.unparsedRanges);
        // status decides the bucket, never the family: a fork can be a
        // perfectly installable, Solid-compatible package and still must
        // never land in `rows` alongside the family it merely resembles.
        if (status === "official") rows.push(result.row);
        else supplemental.push(result.row);
      } else {
        recordUnparsed(name, solidTarget, result.exclusion.unparsedRanges);
        exclusions.push(result.exclusion);
      }
    }
  }

  return {
    schemaVersion: MANIFEST_SCHEMA_VERSION,
    generatedAt: now,
    registry: registry.registry,
    auditedSolid1: AUDITED_SOLID_1,
    solidReleases: catalog,
    rows: sortRows(rows),
    exclusions: sortExclusions(exclusions),
    supplemental: sortRows(supplemental),
    limitations: [...limitations].sort()
  };
}

function compareExclusions(left, right) {
  const familyDelta = (familyOrder(left?.family) ?? Number.POSITIVE_INFINITY) - (familyOrder(right?.family) ?? Number.POSITIVE_INFINITY);
  if (familyDelta !== 0) return familyDelta;
  const leftPackage = left?.package ?? "";
  const rightPackage = right?.package ?? "";
  if (leftPackage !== rightPackage) return leftPackage < rightPackage ? -1 : 1;
  const rank = { solid1: 0, solid2: 1 };
  const rankDelta = (rank[left?.solidTarget] ?? 2) - (rank[right?.solidTarget] ?? 2);
  if (rankDelta !== 0) return rankDelta;
  const leftReason = left?.reason ?? "";
  const rightReason = right?.reason ?? "";
  return leftReason < rightReason ? -1 : leftReason > rightReason ? 1 : 0;
}

function sortExclusions(exclusions) {
  return [...exclusions].sort(compareExclusions);
}

// ---------------------------------------------------------------------------
// CLI wrapper. Everything above is pure/injectable; everything below reads
// argv, touches the filesystem, and is the only place `new Date()` appears.
// ---------------------------------------------------------------------------

function helpText() {
  return `Usage: bun discover.mjs [options]

Refreshes the ecosystem benchmark manifest from the npm registry. This is the
only network-enabled entry point under scripts/ecosystem-benchmark/ — run.mjs
reads the manifest this writes and never reaches the registry on its own
account except to install the exact pinned versions it already recorded.

Options:
  --output <FILE>    Manifest path (default scripts/ecosystem-benchmark/manifest.json)
  --check             Do not write; exit 1 if the manifest on disk differs
  --family <ID>       Restrict discovery to one family (repeatable)
  --print-diff        Print the diff against the file on disk without writing
  --sentinel <FILE>   Also write the pinned sentinel subset
                      (default scripts/ecosystem-benchmark/sentinel.json)
  -h, --help          Show this help

Exit codes: 0 written or unchanged, 1 validation failure or --check drift,
2 registry unavailable.
`;
}

function parseCliArgs(argv) {
  const { values } = parseArgs({
    args: argv,
    options: {
      output: { type: "string" },
      check: { type: "boolean", default: false },
      family: { type: "string", multiple: true, default: [] },
      "print-diff": { type: "boolean", default: false },
      sentinel: { type: "string" },
      help: { type: "boolean", short: "h", default: false }
    },
    allowPositionals: false
  });
  return {
    output: values.output,
    check: values.check,
    family: values.family,
    printDiff: values["print-diff"],
    sentinel: values.sentinel,
    help: values.help
  };
}

function readExisting(filePath) {
  try {
    const text = readFileSync(filePath, "utf8");
    return { text, manifest: JSON.parse(text) };
  } catch (error) {
    if (error?.code === "ENOENT") return { text: null, manifest: null };
    throw error;
  }
}

// A `--family` restricted discovery only ever describes the families it was
// asked to look at; every other family's rows/exclusions/supplemental must
// be carried over unchanged from the manifest already on disk, otherwise a
// one-family refresh would silently erase the rest of the corpus.
function mergeManifest(previous, partial, restrictedFamilyIds, { restricted } = {}) {
  const keepPrevious = entry => entry && !restrictedFamilyIds.has(entry.family);
  const previousRows = Array.isArray(previous?.rows) ? previous.rows.filter(keepPrevious) : [];
  const previousExclusions = Array.isArray(previous?.exclusions) ? previous.exclusions.filter(keepPrevious) : [];
  const previousSupplemental = Array.isArray(previous?.supplemental) ? previous.supplemental.filter(keepPrevious) : [];
  // Limitations carry no family tag, so they cannot be filtered the way rows
  // and exclusions are. A FULL run has just re-derived every one of them, so
  // it must publish only what it observed this time -- carrying the previous
  // list forward unconditionally made limitations write-only, and a fixed gap
  // stayed in the manifest forever. That is worse than no limitation at all:
  // it told the reader a range was unparseable after it had started parsing.
  // A restricted run genuinely did not look at the other families, so there it
  // still carries the previous list forward; those entries can be stale until
  // the next full run re-derives them.
  const previousLimitations = restricted && Array.isArray(previous?.limitations) ? previous.limitations : [];

  return {
    schemaVersion: partial.schemaVersion,
    generatedAt: partial.generatedAt,
    registry: partial.registry,
    auditedSolid1: partial.auditedSolid1,
    // Always fresh: the release catalog is refetched on every invocation
    // regardless of --family, so it is never a stale carry-over.
    solidReleases: partial.solidReleases,
    rows: sortRows([...previousRows, ...partial.rows]),
    exclusions: sortExclusions([...previousExclusions, ...partial.exclusions]),
    supplemental: sortRows([...previousSupplemental, ...partial.supplemental]),
    limitations: [...new Set([...previousLimitations, ...partial.limitations])].sort()
  };
}

function renderDiff(diff) {
  const lines = [
    `manifest diff: +${diff.summary.addedCount} added, -${diff.summary.removedCount} removed, ~${diff.summary.changedCount} changed; exclusions +${diff.summary.exclusionsAddedCount ?? 0}/-${diff.summary.exclusionsRemovedCount ?? 0}/~${diff.summary.exclusionsChangedCount ?? 0}; limitations +${diff.summary.limitationsAddedCount ?? 0}/-${diff.summary.limitationsRemovedCount ?? 0}`
  ];
  for (const row of diff.added) lines.push(`  + ${row.package} (${row.solidTarget}) [${row.family}] @ ${row.version}`);
  for (const row of diff.removed) lines.push(`  - ${row.package} (${row.solidTarget}) [${row.family}] @ ${row.version}`);
  for (const change of diff.changed) {
    // Integrity changes are called out by name, never folded into a generic
    // "changed" line, so a republish-under-the-same-version never slips by
    // unnoticed in a reviewer's skim of the diff.
    const label =
      change.kind === "integrity"
        ? "integrity changed"
        : change.kind === "probes"
          ? "probes changed"
          : `${change.kind} changed`;
    lines.push(`  ~ ${change.package} (${change.solidTarget}) [${change.family}] ${label}: ${change.from} -> ${change.to}`);
  }
  // Exclusions and limitations are part of the manifest too. Reporting only
  // row changes let a refresh that moved nothing but an exclusion reason print
  // "(no changes)" while `--check` still refused the file as drifted, which
  // tells the reviewer the opposite of the truth.
  const exclusions = diff.exclusions ?? { added: [], removed: [], changed: [] };
  const limitations = diff.limitations ?? { added: [], removed: [] };
  for (const entry of exclusions.added) {
    lines.push(`  + excluded ${entry.package} (${entry.solidTarget}) [${entry.family}]: ${entry.reason}`);
  }
  for (const entry of exclusions.removed) {
    lines.push(`  - excluded ${entry.package} (${entry.solidTarget}) [${entry.family}]: ${entry.reason}`);
  }
  for (const change of exclusions.changed) {
    lines.push(
      `  ~ excluded ${change.package} (${change.solidTarget}) [${change.family}] reason changed: ${change.from} -> ${change.to}`
    );
  }
  for (const entry of limitations.added) lines.push(`  + limitation: ${entry}`);
  for (const entry of limitations.removed) lines.push(`  - limitation: ${entry}`);

  const total =
    diff.added.length +
    diff.removed.length +
    diff.changed.length +
    exclusions.added.length +
    exclusions.removed.length +
    exclusions.changed.length +
    limitations.added.length +
    limitations.removed.length;
  if (total === 0) lines.push("  (no changes)");
  return `${lines.join("\n")}\n`;
}

function rowsFor(manifest, familyId, solidTarget) {
  return manifest.rows
    .filter(row => row.family === familyId && row.solidTarget === solidTarget)
    .slice()
    .sort((left, right) => (left.package < right.package ? -1 : left.package > right.package ? 1 : 0));
}

function probeId(row, kind = null) {
  if (!row) return null;
  if (kind) {
    const probe = row.probes.find(candidate => candidate.kind === kind);
    if (probe) return probe.id;
  }
  return row.probes[0]?.id ?? null;
}

function pickByExactPackage(rows, packageName) {
  return rows.find(row => row.package === packageName) ?? null;
}

// Deterministic sentinel selection: every choice below is a documented rule
// over the discovered manifest (sorted-name order, an exact known package
// name, or "prefer the beta-only representative"), never a random pick.
// The failure-class and known-success coverage the sentinel.json spec also
// asks for comes from the last full run.mjs report, which discover.mjs never
// sees — that half of the invariant has to be maintained by whoever edits
// sentinel.json after a benchmark run, per README.md's "Adding a probe"
// section; this function only ever proposes the discovery-derived half.
function buildSentinel(manifest) {
  const probes = new Set();
  const add = id => {
    if (id) probes.add(id);
  };

  for (const solidTarget of SOLID_TARGETS) {
    const officialRows = rowsFor(manifest, "official-solid", solidTarget);
    add(probeId(pickByExactPackage(officialRows, "solid-js")));
    add(probeId(pickByExactPackage(officialRows, "@solidjs/router")));
    add(probeId(pickByExactPackage(officialRows, "@solidjs/meta")));

    const primitives = rowsFor(manifest, "solid-primitives", solidTarget);
    for (const row of primitives.slice(0, 3)) add(probeId(row));

    const kobalte = rowsFor(manifest, "kobalte", solidTarget);
    add(probeId(kobalte[0]));

    const corvu = rowsFor(manifest, "corvu", solidTarget);
    if (solidTarget === "solid2") {
      const betaOnly = corvu.find(row => row.probes.length === 1 && row.probes[0].kind === "only" && row.probes[0].channel === "beta");
      add(probeId(betaOnly ?? corvu[0]));
    } else {
      add(probeId(corvu[0]));
    }

    // TanStack rows are already Solid adapters only (a non-Solid TanStack
    // package never becomes a row at all), so the first by name is enough.
    const tanstack = rowsFor(manifest, "tanstack", solidTarget);
    add(probeId(tanstack[0]));

    // "when compatible": only add a probe if discovery actually produced a
    // row for this target — never force one that does not exist.
    const devtools = rowsFor(manifest, "solid-devtools", solidTarget);
    add(probeId(devtools[0]));

    add(probeId(pickByExactPackage(rowsFor(manifest, "solid-recharts", solidTarget), "solid-recharts")));
    add(probeId(pickByExactPackage(rowsFor(manifest, "motion-solidjs", solidTarget), "motion-solidjs")));
  }

  return { schemaVersion: 1, probes: [...probes].sort() };
}

async function main(argv) {
  let options;
  try {
    options = parseCliArgs(argv);
  } catch (error) {
    process.stderr.write(`${error.message}\n\n${helpText()}`);
    return 1;
  }

  if (options.help) {
    process.stdout.write(helpText());
    return 0;
  }

  const outputPath = resolve(options.output ?? DEFAULT_OUTPUT);
  const sentinelPath = resolve(options.sentinel ?? DEFAULT_SENTINEL);

  let families = FAMILIES;
  if (options.family.length > 0) {
    const unknown = options.family.filter(id => !familyById(id));
    if (unknown.length > 0) {
      process.stderr.write(`unknown family id(s): ${unknown.join(", ")}\n`);
      return 1;
    }
    families = FAMILIES.filter(family => options.family.includes(family.id));
  }
  const restrictedFamilyIds = new Set(families.map(family => family.id));

  const registry = new Registry();
  const now = new Date().toISOString();

  let partial;
  try {
    partial = await discover({ registry, families, now });
  } catch (error) {
    process.stderr.write(`ecosystem discovery failed: registry unavailable: ${error?.message ?? error}\n`);
    return 2;
  }

  const { text: previousText, manifest: previousManifest } = readExisting(outputPath);
  const manifest = mergeManifest(previousManifest, partial, restrictedFamilyIds, {
    restricted: options.family.length > 0
  });

  const problems = validateManifest(manifest);
  if (problems.length > 0) {
    process.stderr.write("discovered manifest failed validation:\n");
    for (const problem of problems) process.stderr.write(`  - ${problem}\n`);
    return 1;
  }

  const diff = diffManifests(previousManifest, manifest);
  process.stdout.write(renderDiff(diff));

  const serialized = serializeManifest(manifest);

  if (options.check) {
    // `generatedAt` is when discovery ran, not something about the ecosystem:
    // comparing it would make --check fail on every invocation, including the
    // CI job whose whole purpose is to notice REAL drift. Compare the
    // discovered content by re-serializing with the previous run's timestamp,
    // so a differing timestamp alone is never reported as staleness.
    const comparable = serializeManifest({
      ...manifest,
      generatedAt: previousManifest?.generatedAt ?? manifest.generatedAt
    });
    if (previousText !== comparable) {
      process.stderr.write(`${outputPath} is out of date; run without --check to refresh it.\n`);
      return 1;
    }
    return 0;
  }

  if (options.printDiff) {
    return 0;
  }

  mkdirSync(dirname(outputPath), { recursive: true });
  writeFileSync(outputPath, serialized);

  const sentinel = buildSentinel(manifest);
  mkdirSync(dirname(sentinelPath), { recursive: true });
  writeFileSync(sentinelPath, `${JSON.stringify(sentinel, null, 2)}\n`);

  return 0;
}

const isMainModule = process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isMainModule) {
  main(process.argv.slice(2)).then(
    code => {
      process.exitCode = code;
    },
    error => {
      console.error(error);
      process.exitCode = 2;
    }
  );
}
