// The pinned ecosystem manifest: validation, deterministic serialization,
// ordering, diffing, and summary statistics.
//
// The manifest is the sole input execution reads (see registry.mjs); every
// guarantee here exists to keep that input trustworthy without re-touching
// the network. A validation gap here is a silent corpus drift that no later
// stage would ever catch.

import { parseVersion } from "./semver.mjs";
import { AUDITED_SOLID_2, FAMILIES, classifyPackage, familyById, familyOrder } from "./families.mjs";

export const MANIFEST_SCHEMA_VERSION = 1;

const SOLID_TARGETS = ["solid1", "solid2"];
const SOLID_TARGET_RANK = { solid1: 0, solid2: 1 };

const MANIFEST_KEYS = [
  "schemaVersion",
  "generatedAt",
  "registry",
  "auditedSolid1",
  "auditedSolid2",
  "solidReleases",
  "rows",
  "exclusions",
  "supplemental",
  "limitations"
];
const ROW_KEYS = [
  "family",
  "status",
  "package",
  "solidTarget",
  "version",
  "distTags",
  "integrity",
  "deprecated",
  "dependencies",
  "peerDependencies",
  "optionalDependencies",
  "compatibleSolidVersions",
  "unparsedRanges",
  "probes"
];
const PROBE_KEYS = ["id", "kind", "channel", "solid", "entrypoints"];
const EXCLUSION_KEYS = ["family", "status", "package", "solidTarget", "reason", "detail"];
const SOLID_RELEASE_KEYS = ["distTags", "v1", "v2"];

function scopeOf(name) {
  if (typeof name !== "string" || !name.startsWith("@")) return null;
  const slash = name.indexOf("/");
  return slash === -1 ? null : name.slice(1, slash);
}

// Re-derives classifyPackage's documented contract (official via
// scopes/packages, supplemental via supplementalScopes/searchTerms) so a
// caller can inject a fake family table in tests without this module reaching
// into families.mjs's real, separately-owned implementation. The production
// path (no injected table) always defers to the real classifyPackage so this
// approximation never governs a real manifest.
function classifyWithFamilies(name, families) {
  if (typeof name !== "string") return null;
  const scope = scopeOf(name);
  for (const family of families) {
    if (Array.isArray(family.packages) && family.packages.includes(name)) {
      return { family: family.id, status: "official" };
    }
    if (scope && Array.isArray(family.scopes) && family.scopes.includes(scope)) {
      return { family: family.id, status: "official" };
    }
  }
  for (const family of families) {
    if (scope && Array.isArray(family.supplementalScopes) && family.supplementalScopes.includes(scope)) {
      return { family: family.id, status: "supplemental" };
    }
    if (Array.isArray(family.searchTerms) && family.searchTerms.some(term => name.includes(term))) {
      return { family: family.id, status: "supplemental" };
    }
  }
  return null;
}

function classifyFamily(name, families) {
  return families === FAMILIES ? classifyPackage(name) : classifyWithFamilies(name, families);
}

function findFamily(id, families) {
  return families === FAMILIES ? familyById(id) : families.find(family => family.id === id);
}

function orderOf(id, families) {
  if (families === FAMILIES) return familyOrder(id);
  const index = families.findIndex(family => family.id === id);
  if (index === -1) return Number.POSITIVE_INFINITY;
  const family = families[index];
  return typeof family.order === "number" ? family.order : index;
}

function compareRows(left, right, families) {
  const familyDelta = orderOf(left?.family, families) - orderOf(right?.family, families);
  if (familyDelta !== 0) return familyDelta;
  const leftPackage = left?.package ?? "";
  const rightPackage = right?.package ?? "";
  if (leftPackage !== rightPackage) return leftPackage < rightPackage ? -1 : 1;
  const leftRank = SOLID_TARGET_RANK[left?.solidTarget] ?? 2;
  const rightRank = SOLID_TARGET_RANK[right?.solidTarget] ?? 2;
  return leftRank - rightRank;
}

export function sortRows(rows, { families = FAMILIES } = {}) {
  return [...rows].sort((left, right) => compareRows(left, right, families));
}

function describeEntry(entry) {
  return `${entry?.package ?? "?"}/${entry?.solidTarget ?? "?"}`;
}

function validateRowShapedEntries(entries, label, families, problems) {
  entries.forEach((row, index) => {
    const where = `${label}[${index}] (${describeEntry(row)})`;

    if (!findFamily(row?.family, families)) {
      problems.push(`${where}: unknown family ${JSON.stringify(row?.family)}`);
    }

    const classified = classifyFamily(row?.package, families);
    if (!classified || classified.family !== row?.family) {
      problems.push(
        `${where}: declared family ${JSON.stringify(row?.family)} disagrees with classifyPackage result ` +
          `${JSON.stringify(classified)}`
      );
    }

    if (typeof row?.integrity !== "string" || row.integrity.trim() === "") {
      problems.push(`${where}: integrity is missing or empty`);
    }

    if (!parseVersion(row?.version)) {
      problems.push(`${where}: version ${JSON.stringify(row?.version)} is not an exact semver version`);
    }

    const probes = Array.isArray(row?.probes) ? row.probes : [];
    if (probes.length !== 1 && probes.length !== 2) {
      problems.push(`${where}: expected 1 or 2 probes, got ${probes.length}`);
    }

    const compatible =
      row?.compatibleSolidVersions && typeof row.compatibleSolidVersions === "object" ? row.compatibleSolidVersions : {};
    for (const probe of probes) {
      const solid = probe?.solid && typeof probe.solid === "object" ? probe.solid : {};
      for (const [runtimePackage, version] of Object.entries(solid)) {
        const allowed = compatible[runtimePackage];
        if (!Array.isArray(allowed) || !allowed.includes(version)) {
          problems.push(
            `${where}: probe ${JSON.stringify(probe?.id)} solid version ${runtimePackage}@${version} is not in ` +
              "compatibleSolidVersions"
          );
        }
      }
      if (probe?.entrypoints !== undefined) {
        if (
          !Array.isArray(probe.entrypoints) ||
          probe.entrypoints.length === 0 ||
          probe.entrypoints.some(entrypoint => typeof entrypoint !== "string" || !entrypoint.startsWith(".")) ||
          new Set(probe.entrypoints).size !== probe.entrypoints.length
        ) {
          problems.push(`${where}: probe ${JSON.stringify(probe?.id)} entrypoints must be unique package subpaths`);
        }
      }
    }
  });
}

function validateExclusions(exclusions, families, problems) {
  exclusions.forEach((exclusion, index) => {
    const where = `exclusions[${index}] (${describeEntry(exclusion)})`;

    if (!findFamily(exclusion?.family, families)) {
      problems.push(`${where}: unknown family ${JSON.stringify(exclusion?.family)}`);
    }

    const classified = classifyFamily(exclusion?.package, families);
    if (!classified || classified.family !== exclusion?.family) {
      problems.push(
        `${where}: declared family ${JSON.stringify(exclusion?.family)} disagrees with classifyPackage result ` +
          `${JSON.stringify(classified)}`
      );
    }
  });
}

// Order is deterministic-by-construction (sortRows), not deterministic-by-luck:
// a manifest whose rows drifted out of order would otherwise still validate,
// and a later run's re-sort would silently reshuffle the diff.
function validateRowOrder(entries, label, families, problems) {
  for (let index = 1; index < entries.length; index++) {
    if (compareRows(entries[index - 1], entries[index], families) > 0) {
      problems.push(
        `${label} is not in deterministic order at index ${index}: ${describeEntry(entries[index - 1])} should sort ` +
          `after ${describeEntry(entries[index])}`
      );
      return;
    }
  }
}

function validateDuplicateProbeIds(entries, problems) {
  const seen = new Map();
  for (const entry of entries) {
    for (const probe of Array.isArray(entry?.probes) ? entry.probes : []) {
      const id = probe?.id;
      if (id === undefined) continue;
      if (seen.has(id)) {
        problems.push(`duplicate probe id ${JSON.stringify(id)} on ${describeEntry(entry)} and ${seen.get(id)}`);
      } else {
        seen.set(id, describeEntry(entry));
      }
    }
  }
}

// A family with neither a row nor an exclusion never ran a selection attempt
// at all — that is corpus shrinkage, not "nothing to report", and it must
// fail the same way a wrong schemaVersion does.
function validateFamilyCompleteness(rows, exclusions, families, problems) {
  for (const family of families) {
    const hasRow = rows.some(row => row?.family === family.id);
    const hasExclusion = exclusions.some(exclusion => exclusion?.family === family.id);
    if (!hasRow && !hasExclusion) {
      problems.push(`family ${JSON.stringify(family.id)} has zero rows and zero exclusions`);
    }
  }
}

// minimumPackages names the packages a family report is meaningless without
// (e.g. the runtime itself). Missing one on one Solid target is exactly the
// kind of partial regeneration a rerun-after-crash could produce unnoticed.
function validateMinimumPackages(rows, exclusions, families, problems) {
  for (const family of families) {
    const minimumPackages = Array.isArray(family.minimumPackages) ? family.minimumPackages : [];
    for (const packageName of minimumPackages) {
      for (const solidTarget of SOLID_TARGETS) {
        const present =
          rows.some(row => row?.package === packageName && row?.solidTarget === solidTarget) ||
          exclusions.some(exclusion => exclusion?.package === packageName && exclusion?.solidTarget === solidTarget);
        if (!present) {
          problems.push(
            `family ${JSON.stringify(family.id)} minimum package ${JSON.stringify(packageName)} is missing as a row ` +
              `or exclusion on ${solidTarget}`
          );
        }
      }
    }
  }
}

export function validateManifest(manifest, { families = FAMILIES } = {}) {
  if (!manifest || typeof manifest !== "object") return ["manifest must be an object"];

  const problems = [];

  if (manifest.schemaVersion !== MANIFEST_SCHEMA_VERSION) {
    problems.push(`schemaVersion must be ${MANIFEST_SCHEMA_VERSION}, got ${JSON.stringify(manifest.schemaVersion)}`);
  }
  if (manifest.auditedSolid2 !== AUDITED_SOLID_2) {
    problems.push(`auditedSolid2 must be ${AUDITED_SOLID_2}, got ${JSON.stringify(manifest.auditedSolid2)}`);
  }

  const rows = Array.isArray(manifest.rows) ? manifest.rows : [];
  const supplemental = Array.isArray(manifest.supplemental) ? manifest.supplemental : [];
  const exclusions = Array.isArray(manifest.exclusions) ? manifest.exclusions : [];

  validateRowShapedEntries(rows, "rows", families, problems);
  validateRowShapedEntries(supplemental, "supplemental", families, problems);
  validateExclusions(exclusions, families, problems);
  validateRowOrder(rows, "rows", families, problems);
  validateRowOrder(supplemental, "supplemental", families, problems);
  validateDuplicateProbeIds([...rows, ...supplemental], problems);
  validateFamilyCompleteness(rows, exclusions, families, problems);
  validateMinimumPackages(rows, exclusions, families, problems);

  return problems;
}

function pick(source, keys) {
  const target = {};
  for (const key of keys) {
    if (Object.prototype.hasOwnProperty.call(source, key)) target[key] = source[key];
  }
  // Unknown keys are forward-compatible data, not noise: keep them, but sort
  // so their presence still serializes deterministically.
  for (const key of Object.keys(source).sort()) {
    if (!keys.includes(key)) target[key] = source[key];
  }
  return target;
}

function sortedDictionary(source) {
  if (!source || typeof source !== "object") return source;
  const target = {};
  for (const key of Object.keys(source).sort()) {
    target[key] = source[key];
  }
  return target;
}

function canonicalProbe(probe) {
  const ordered = pick(probe ?? {}, PROBE_KEYS);
  if (ordered.solid) ordered.solid = sortedDictionary(ordered.solid);
  return ordered;
}

function canonicalRow(row) {
  const ordered = pick(row ?? {}, ROW_KEYS);
  ordered.dependencies = sortedDictionary(ordered.dependencies ?? {});
  ordered.peerDependencies = sortedDictionary(ordered.peerDependencies ?? {});
  ordered.optionalDependencies = sortedDictionary(ordered.optionalDependencies ?? {});
  ordered.compatibleSolidVersions = sortedDictionary(ordered.compatibleSolidVersions ?? {});
  ordered.probes = (Array.isArray(ordered.probes) ? ordered.probes : []).map(canonicalProbe);
  return ordered;
}

function canonicalExclusion(exclusion) {
  return pick(exclusion ?? {}, EXCLUSION_KEYS);
}

function canonicalSolidRelease(entry) {
  const ordered = pick(entry ?? {}, SOLID_RELEASE_KEYS);
  ordered.distTags = sortedDictionary(ordered.distTags ?? {});
  return ordered;
}

function canonicalManifest(manifest) {
  const ordered = pick(manifest ?? {}, MANIFEST_KEYS);
  ordered.solidReleases = sortedDictionary(
    Object.fromEntries(
      Object.entries(ordered.solidReleases ?? {}).map(([name, entry]) => [name, canonicalSolidRelease(entry)])
    )
  );
  ordered.rows = (Array.isArray(ordered.rows) ? ordered.rows : []).map(canonicalRow);
  ordered.exclusions = (Array.isArray(ordered.exclusions) ? ordered.exclusions : []).map(canonicalExclusion);
  ordered.supplemental = (Array.isArray(ordered.supplemental) ? ordered.supplemental : []).map(canonicalRow);
  return ordered;
}

// Byte-deterministic regardless of the input object's own key insertion
// order: two manifests describing the same facts must serialize identically
// so a git diff reflects a real corpus change, never object-literal shuffle.
export function serializeManifest(manifest) {
  return `${JSON.stringify(canonicalManifest(manifest), null, 2)}\n`;
}

function rowKey(row) {
  return `${row?.package}|${row?.solidTarget}`;
}

function rowKeyedEntries(rows) {
  return (Array.isArray(rows) ? rows : []).map(row => [rowKey(row), row]);
}

// An integrity change with no version change (a registry republish under the
// same version, or a prior recording bug) is exactly the kind of change a
// version-only diff would swallow. It must always surface as its own entry.
/**
 * One row's probe set as a stable, comparable string.
 *
 * Ordered by kind and rendered with each runtime package's exact version, so a
 * floor moving from `2.0.0-rc.0` to `2.0.0-rc.1`, a row collapsing from
 * floor/head to a single `only`, and a probe appearing or disappearing are all
 * visible as a plain before/after rather than as a silent rewrite.
 */
function describeProbes(probes) {
  if (!Array.isArray(probes)) return "";
  return probes
    .map(probe => {
      const solid = Object.entries(probe?.solid ?? {})
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([name, version]) => `${name}@${version}`)
        .join("+");
      return `${probe?.kind ?? "?"}=${solid}`;
    })
    .sort()
    .join(", ");
}

export function diffManifests(previous, next, { families = FAMILIES } = {}) {
  const previousRows = new Map(rowKeyedEntries(previous?.rows));
  const nextRows = new Map(rowKeyedEntries(next?.rows));

  const added = [];
  const removed = [];
  const changed = [];

  for (const [key, row] of nextRows) {
    if (!previousRows.has(key)) added.push(row);
  }
  for (const [key, row] of previousRows) {
    if (!nextRows.has(key)) removed.push(row);
  }
  for (const [key, previousRow] of previousRows) {
    const nextRow = nextRows.get(key);
    if (!nextRow) continue;
    if (previousRow.version !== nextRow.version) {
      changed.push({
        kind: "version",
        family: nextRow.family,
        package: nextRow.package,
        solidTarget: nextRow.solidTarget,
        from: previousRow.version,
        to: nextRow.version
      });
    }
    if (previousRow.integrity !== nextRow.integrity) {
      changed.push({
        kind: "integrity",
        family: nextRow.family,
        package: nextRow.package,
        solidTarget: nextRow.solidTarget,
        from: previousRow.integrity,
        to: nextRow.integrity
      });
    }
    // Probes are what actually gets installed and run, and they move for two
    // reasons a version/integrity comparison cannot see: a new runtime release
    // shifting a head, or a change to selection policy itself. `--check`
    // compares the whole serialized document, so it already refuses such a
    // manifest -- which is exactly why this has to be diffed. Without it the
    // reviewer reads "(no changes)" and is then told the file is stale.
    const previousProbes = describeProbes(previousRow.probes);
    const nextProbes = describeProbes(nextRow.probes);
    if (previousProbes !== nextProbes) {
      changed.push({
        kind: "probes",
        family: nextRow.family,
        package: nextRow.package,
        solidTarget: nextRow.solidTarget,
        from: previousProbes,
        to: nextProbes
      });
    }
  }

  const sortedAdded = sortRows(added, { families });
  const sortedRemoved = sortRows(removed, { families });
  const sortedChanged = [...changed].sort((left, right) => {
    const rowDelta = compareRows(left, right, families);
    if (rowDelta !== 0) return rowDelta;
    return left.kind < right.kind ? -1 : left.kind > right.kind ? 1 : 0;
  });

  // Rows are not the whole manifest. A refresh can change only which packages
  // are EXCLUDED (a package leaving the ecosystem, a range becoming
  // unparseable) or only which registry gaps were hit, and a diff that reports
  // "no changes" while `--check` refuses the file as drifted is worse than no
  // diff at all -- the reviewer is told nothing moved and then told the file is
  // stale. Exclusions and limitations are therefore diffed too.
  const exclusionKey = entry => `${entry.family}\u0000${entry.package}\u0000${entry.solidTarget}`;
  const previousExclusions = new Map(
    (Array.isArray(previous?.exclusions) ? previous.exclusions : []).map(entry => [exclusionKey(entry), entry])
  );
  const nextExclusions = new Map(
    (Array.isArray(next?.exclusions) ? next.exclusions : []).map(entry => [exclusionKey(entry), entry])
  );
  const exclusionsAdded = [];
  const exclusionsRemoved = [];
  const exclusionsChanged = [];
  for (const [key, entry] of nextExclusions) {
    if (!previousExclusions.has(key)) exclusionsAdded.push(entry);
  }
  for (const [key, entry] of previousExclusions) {
    if (!nextExclusions.has(key)) exclusionsRemoved.push(entry);
  }
  for (const [key, previousEntry] of previousExclusions) {
    const nextEntry = nextExclusions.get(key);
    if (!nextEntry) continue;
    if (previousEntry.reason !== nextEntry.reason) {
      exclusionsChanged.push({
        kind: "exclusion-reason",
        family: nextEntry.family,
        package: nextEntry.package,
        solidTarget: nextEntry.solidTarget,
        from: previousEntry.reason,
        to: nextEntry.reason
      });
    }
  }
  const compareExclusions = (left, right) =>
    exclusionKey(left) < exclusionKey(right) ? -1 : exclusionKey(left) > exclusionKey(right) ? 1 : 0;

  const previousLimitations = new Set(Array.isArray(previous?.limitations) ? previous.limitations : []);
  const nextLimitations = new Set(Array.isArray(next?.limitations) ? next.limitations : []);
  const limitationsAdded = [...nextLimitations].filter(entry => !previousLimitations.has(entry)).sort((left, right) => (left < right ? -1 : left > right ? 1 : 0));
  const limitationsRemoved = [...previousLimitations].filter(entry => !nextLimitations.has(entry)).sort((left, right) => (left < right ? -1 : left > right ? 1 : 0));

  const exclusions = {
    added: exclusionsAdded.sort(compareExclusions),
    removed: exclusionsRemoved.sort(compareExclusions),
    changed: exclusionsChanged.sort(compareExclusions)
  };
  const limitations = { added: limitationsAdded, removed: limitationsRemoved };

  return {
    added: sortedAdded,
    removed: sortedRemoved,
    changed: sortedChanged,
    exclusions,
    limitations,
    summary: {
      addedCount: sortedAdded.length,
      removedCount: sortedRemoved.length,
      changedCount: sortedChanged.length,
      exclusionsAddedCount: exclusions.added.length,
      exclusionsRemovedCount: exclusions.removed.length,
      exclusionsChangedCount: exclusions.changed.length,
      limitationsAddedCount: limitations.added.length,
      limitationsRemovedCount: limitations.removed.length
    }
  };
}

export function manifestStats(manifest, { families = FAMILIES } = {}) {
  const rows = Array.isArray(manifest?.rows) ? manifest.rows : [];
  const supplemental = Array.isArray(manifest?.supplemental) ? manifest.supplemental : [];
  const exclusions = Array.isArray(manifest?.exclusions) ? manifest.exclusions : [];

  const perFamily = new Map();
  const ensure = id => {
    if (!perFamily.has(id)) {
      perFamily.set(id, { family: id, rowCount: 0, probeCount: 0, exclusionCount: 0, supplementalCount: 0 });
    }
    return perFamily.get(id);
  };
  for (const family of families) ensure(family.id);

  let probeCount = 0;
  for (const row of rows) {
    const stats = ensure(row?.family);
    stats.rowCount += 1;
    const rowProbes = Array.isArray(row?.probes) ? row.probes.length : 0;
    stats.probeCount += rowProbes;
    probeCount += rowProbes;
  }
  for (const exclusion of exclusions) {
    ensure(exclusion?.family).exclusionCount += 1;
  }
  for (const row of supplemental) {
    ensure(row?.family).supplementalCount += 1;
  }

  const orderedFamilies = [...perFamily.values()].sort(
    (left, right) => orderOf(left.family, families) - orderOf(right.family, families)
  );

  return {
    families: orderedFamilies,
    rowCount: rows.length,
    probeCount,
    exclusionCount: exclusions.length,
    supplementalCount: supplemental.length
  };
}
