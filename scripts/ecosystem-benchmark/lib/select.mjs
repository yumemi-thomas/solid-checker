// Core selection: for one ecosystem package and one Solid target ("solid1" or
// "solid2"), decide which published release of the package to test and which
// published release(s) of the Solid runtime to test it against.
//
// Pure and offline — every fact this module needs (published versions,
// declared ranges, dist-tags) is already sitting in the abbreviated
// packuments the caller collected during discovery. No network, no clock.

import { flattenRanges, solidRanges } from "./registry.mjs";
import { compareVersions, parseRange, satisfies, sortVersions } from "./semver.mjs";
import { AUDITED_SOLID_2, SOLID_RUNTIME_PACKAGES } from "./families.mjs";
import { scopedEntrypoints } from "./framework-scopes.mjs";

/**
 * Every published release of each SOLID_RUNTIME_PACKAGE, split by Solid
 * major. `packuments` may be missing an entry (network failure, unpublished
 * scope) — that package's catalog is simply empty, which correctly makes
 * every range against it unsatisfiable rather than throwing.
 */
export function solidReleaseCatalog(packuments, auditedSolid2 = AUDITED_SOLID_2) {
  const catalog = {};
  for (const name of SOLID_RUNTIME_PACKAGES) {
    const packument = packuments.get(name);
    const versions = packument ? Object.keys(packument.versions ?? {}) : [];
    const v1 = [];
    const v2 = [];
    for (const version of versions) {
      const major = parseVersionMajor(version);
      if (major === 1) v1.push(version);
      else if (major === 2 && compareVersions(version, auditedSolid2) <= 0) v2.push(version);
    }
    // What each release of this runtime package requires of its *siblings*.
    // `@solidjs/web@2.0.0-rc.1` peering `solid-js ^2.0.0-rc.1` is the fact
    // that makes some floor combinations impossible; without it the selector
    // can only floor each package in isolation and hope the tuple coexists.
    const peers = {};
    for (const version of versions) {
      const declared = flattenRanges(
        solidRanges(packument?.versions?.[version], SOLID_RUNTIME_PACKAGES)
      );
      delete declared[name];
      if (Object.keys(declared).length > 0) peers[version] = declared;
    }
    catalog[name] = {
      v1: sortVersions(v1),
      v2: sortVersions(v2),
      distTags: { ...(packument?.["dist-tags"] ?? {}) },
      peers
    };
  }
  return catalog;
}

function parseVersionMajor(version) {
  const match = /^v?(\d+)\./.exec(version.trim());
  return match ? Number(match[1]) : null;
}

/**
 * Channel of a version's prerelease identifier. Used to label probes
 * (`experimental`/`beta`/`rc` releases are meaningfully different maturity
 * levels, not just "some prerelease") without ever string-prefixing a
 * version — the identifier is read from the parsed prerelease array, the
 * same structure `satisfies` itself works from.
 */
export function prereleaseChannel(version) {
  const parts = String(version).split("-");
  if (parts.length < 2) return "stable";
  const first = parts[1].split(".")[0];
  if (first === "experimental") return "experimental";
  if (first === "beta") return "beta";
  if (first === "rc") return "rc";
  return "other";
}

export function distTagsFor(packument, version) {
  const tags = packument?.["dist-tags"] ?? {};
  return Object.entries(tags)
    .filter(([, value]) => value === version)
    .map(([tag]) => tag)
    .sort();
}

function isDeprecated(versionDoc) {
  return Boolean(versionDoc?.deprecated);
}

function deprecatedValue(versionDoc) {
  if (!isDeprecated(versionDoc)) return null;
  return typeof versionDoc.deprecated === "string" ? versionDoc.deprecated : true;
}

function resolveFamily(family) {
  if (typeof family === "string") return { id: family, requireSolidDependency: false };
  return { id: family.id, requireSolidDependency: Boolean(family.requireSolidDependency) };
}

function dedupeUnparsed(entries) {
  const byKey = new Map();
  for (const entry of entries) byKey.set(`${entry.package}\u0000${entry.range}`, entry);
  return [...byKey.values()].sort((a, b) => {
    if (a.package !== b.package) return a.package < b.package ? -1 : 1;
    if (a.range === b.range) return 0;
    return a.range < b.range ? -1 : 1;
  });
}

// Declared ranges for the SOLID_RUNTIME_PACKAGES on one version, collapsed
// across dependencies/peerDependencies/optionalDependencies with the fixed
// precedence `flattenRanges` already implements.
function flatRuntimeRanges(versionDoc) {
  return flattenRanges(solidRanges(versionDoc, SOLID_RUNTIME_PACKAGES));
}

/**
 * Rule 3: solid1 compatibility of one candidate version turns on its declared
 * `solid-js` range alone — @solidjs/web and @solidjs/signals have no 1.x
 * releases, so a range against them can never be satisfied by anything and a
 * package that declares only one of those (instead of solid-js) is correctly
 * heading toward `no-compatible-release`, not a false `no-solid-dependency`.
 * The exception baked in here is narrow: an official-style package
 * (`requireSolidDependency: false`) that skips solid-js in favor of another
 * runtime package is still let through to that (always-empty-for-v1) check,
 * so its exclusion reason ends up accurate instead of misleadingly
 * "no-solid-dependency" for a package that plainly does depend on Solid.
 */
function evaluateVersionSolid1(packageName, versionDoc, family, catalog, auditedSolid1) {
  if (packageName === "solid-js") {
    return { status: "compatible", compatibleVersions: { "solid-js": [auditedSolid1] }, unparsed: [] };
  }

  const flat = flatRuntimeRanges(versionDoc);
  if (typeof flat["solid-js"] === "string") {
    const range = flat["solid-js"];
    const parsed = parseRange(range);
    if (parsed === null) return { status: "unparsed", compatibleVersions: {}, unparsed: [{ package: "solid-js", range }] };
    if (satisfies(auditedSolid1, parsed)) {
      return { status: "compatible", compatibleVersions: { "solid-js": [auditedSolid1] }, unparsed: [] };
    }
    return { status: "incompatible", compatibleVersions: {}, unparsed: [] };
  }

  const otherPackages = SOLID_RUNTIME_PACKAGES.filter(pkg => pkg !== "solid-js" && typeof flat[pkg] === "string");
  if (otherPackages.length === 0 || resolveFamily(family).requireSolidDependency) {
    return { status: "no-range", compatibleVersions: {}, unparsed: [] };
  }

  const unparsed = [];
  for (const pkg of otherPackages) {
    const range = flat[pkg];
    const parsed = parseRange(range);
    if (parsed === null) {
      unparsed.push({ package: pkg, range });
      continue;
    }
    const pool = catalog[pkg]?.v1 ?? [];
    const matches = sortVersions(pool.filter(version => satisfies(version, parsed)));
    if (matches.length > 0) return { status: "compatible", compatibleVersions: { [pkg]: matches }, unparsed };
  }
  return { status: unparsed.length > 0 ? "unparsed" : "incompatible", compatibleVersions: {}, unparsed };
}

/**
 * Rule 4: solid2 compatibility is a union across every SOLID_RUNTIME_PACKAGE
 * the version declares a range for — any one non-empty set is enough. A
 * runtime package evaluating its own versions never needs to declare a
 * dependency on itself: it is compatible with every one of its own published
 * 2.x releases by definition.
 */
function evaluateVersionSolid2(packageName, versionDoc, catalog) {
  const compatibleVersions = {};
  const unparsed = [];
  let declaredAny = false;

  if (SOLID_RUNTIME_PACKAGES.includes(packageName)) {
    compatibleVersions[packageName] = [...(catalog[packageName]?.v2 ?? [])];
    declaredAny = true;
  }

  const flat = flatRuntimeRanges(versionDoc);
  for (const pkg of SOLID_RUNTIME_PACKAGES) {
    if (pkg === packageName) continue;
    if (typeof flat[pkg] !== "string") continue;
    declaredAny = true;
    const range = flat[pkg];
    const parsed = parseRange(range);
    if (parsed === null) {
      unparsed.push({ package: pkg, range });
      continue;
    }
    const pool = catalog[pkg]?.v2 ?? [];
    const matches = sortVersions(pool.filter(version => satisfies(version, parsed)));
    if (matches.length > 0) compatibleVersions[pkg] = matches;
  }

  const anyCompatible = Object.values(compatibleVersions).some(list => list.length > 0);
  let status;
  if (anyCompatible) status = "compatible";
  else if (!declaredAny) status = "no-range";
  else if (unparsed.length > 0) status = "unparsed";
  else status = "incompatible";
  return { status, compatibleVersions, unparsed };
}

function buildDeclaredRanges(versionDoc) {
  const ranges = solidRanges(versionDoc, SOLID_RUNTIME_PACKAGES);
  const result = {};
  for (const field of ["dependencies", "peerDependencies", "optionalDependencies"]) {
    const declared = ranges[field] ?? {};
    const sorted = {};
    for (const key of Object.keys(declared).sort()) sorted[key] = declared[key];
    result[field] = sorted;
  }
  return result;
}

function buildCompatibleSolidVersions(map) {
  const output = {};
  for (const key of Object.keys(map).sort()) {
    const list = map[key];
    if (list && list.length > 0) output[key] = sortVersions([...list]);
  }
  return output;
}

// The channel a probe is labeled with: solid-js's own channel when solid-js
// is part of the probe's environment, otherwise the first SOLID_RUNTIME_PACKAGES
// entry (in that fixed report order) that is present.
function computeChannel(env) {
  for (const pkg of SOLID_RUNTIME_PACKAGES) {
    if (typeof env[pkg] === "string") return prereleaseChannel(env[pkg]);
  }
  return "other";
}

// The oldest Solid 2.x prerelease a floor probe will select. The 2.x line spent
// a long time in `experimental` and `beta`, and those releases are no longer
// what the ecosystem builds against: a package published this month declares a
// range whose *formal* lower bound is still some old beta, but its own
// dependencies have moved on, so installing that beta produces a peer conflict
// that says nothing about the checker or about a compatibility window anyone
// supports. Anchoring the floor at `rc.0` measures the oldest release still
// worth measuring.
const SOLID2_FLOOR = "2.0.0-rc.0";

/**
 * The floor release for one runtime package on one Solid target.
 *
 * For solid1 and for any set with no rc-or-later member this is simply the
 * oldest compatible release. For solid2 the floor is raised to the oldest
 * compatible release at or after [`SOLID2_FLOOR`] *when one exists*. The
 * guard matters: a package whose declared range genuinely accepts nothing
 * newer than a beta keeps that beta as its floor, because substituting an rc
 * there would attribute rc behavior to a window the author never declared —
 * the same rule that keeps a beta-only package off a newer rc at the head.
 */
function floorVersion(list, solidTarget) {
  if (solidTarget !== "solid2") return list[0];
  const atOrAfterFloor = list.find(candidate => compareVersions(candidate, SOLID2_FLOOR) >= 0);
  return atOrAfterFloor ?? list[0];
}

/**
 * Raise a floor tuple until the runtime packages in it actually accept each
 * other, or answer `null` when no such tuple exists inside the compatible sets.
 *
 * Flooring each runtime package independently can synthesize an environment
 * that has never existed. `@tanstack/solid-router@2.0.0-rc.1` pins
 * `@solidjs/web@^2.0.0-rc.1`, and that web release peers `solid-js
 * ^2.0.0-rc.1` — so a floor of `solid-js@2.0.0-rc.0` with `@solidjs/web@2.0.0-rc.1`
 * is refused by npm before the checker ever runs, and the resulting
 * `install-failure` describes the benchmark's own arithmetic rather than the
 * package. Only ever raising a version keeps this a tightening of the floor,
 * never a substitution outside the declared range.
 */
function coherentFloor(floorEnv, compatibleSolidVersions, catalog) {
  const names = Object.keys(floorEnv);
  const env = { ...floorEnv };
  // Each pass can only raise a version, and each version can only be raised
  // to a member of its own finite compatible set, so this terminates; the
  // bound is a guard against a cyclic requirement, not the expected exit.
  for (let pass = 0; pass <= names.length; pass++) {
    let raised = false;
    for (const holder of names) {
      const required = catalog[holder]?.peers?.[env[holder]] ?? {};
      for (const target of names) {
        if (target === holder) continue;
        const range = required[target];
        if (!range || satisfies(env[target], range)) continue;
        const next = compatibleSolidVersions[target].find(
          candidate =>
            compareVersions(candidate, env[target]) >= 0 && satisfies(candidate, range)
        );
        if (!next) return null;
        env[target] = next;
        raised = true;
      }
    }
    if (!raised) return env;
  }
  return null;
}

/**
 * Rule 8: floor/head come from `compatibleSolidVersions`, the range-filtered
 * set — never from the runtime package's full catalog. That distinction is
 * what keeps a beta-only package pinned to its one accepted beta: if the
 * declared range only ever matches `2.0.0-beta.17`, the "head" of that set is
 * still `2.0.0-beta.17`, not the newest rc/beta the runtime has ever shipped.
 * Only when every runtime package's floor and head genuinely differ do we
 * emit two probes instead of collapsing to one `kind: "only"` probe.
 */
function buildProbes(packageName, version, solidTarget, compatibleSolidVersions, catalog) {
  const independentFloor = {};
  const headEnv = {};
  for (const pkg of Object.keys(compatibleSolidVersions)) {
    const list = compatibleSolidVersions[pkg];
    independentFloor[pkg] = floorVersion(list, solidTarget);
    headEnv[pkg] = list[list.length - 1];
  }
  // No coherent older environment means there is nothing distinct to measure
  // below the head, so the row collapses to one probe rather than carrying a
  // floor that cannot install. If the head is itself incoherent that probe
  // still fails, which is the package's own graph talking.
  const floorEnv = coherentFloor(independentFloor, compatibleSolidVersions, catalog) ?? headEnv;
  const sameEverywhere = Object.keys(floorEnv).every(pkg => floorEnv[pkg] === headEnv[pkg]);
  let probes;
  if (sameEverywhere) {
    probes = [
      { id: `${packageName}@${version}|${solidTarget}|only`, kind: "only", channel: computeChannel(floorEnv), solid: floorEnv }
    ];
  } else {
    probes = [
      { id: `${packageName}@${version}|${solidTarget}|floor`, kind: "floor", channel: computeChannel(floorEnv), solid: floorEnv },
      { id: `${packageName}@${version}|${solidTarget}|head`, kind: "head", channel: computeChannel(headEnv), solid: headEnv }
    ];
  }
  const entrypoints = scopedEntrypoints(packageName, version);
  if (entrypoints) {
    for (const probe of probes) probe.entrypoints = [...entrypoints];
  }
  return probes;
}

function buildRow({ packageName, familyId, status, solidTarget, version, versionDoc, packument, compatibleSolidVersions, unparsedRanges, catalog }) {
  const declared = buildDeclaredRanges(versionDoc);
  const compat = buildCompatibleSolidVersions(compatibleSolidVersions);
  return {
    family: familyId,
    status,
    package: packageName,
    solidTarget,
    version,
    distTags: distTagsFor(packument, version),
    integrity: versionDoc?.dist?.integrity ?? null,
    deprecated: deprecatedValue(versionDoc),
    dependencies: declared.dependencies,
    peerDependencies: declared.peerDependencies,
    optionalDependencies: declared.optionalDependencies,
    compatibleSolidVersions: compat,
    unparsedRanges: dedupeUnparsed(unparsedRanges),
    probes: buildProbes(packageName, version, solidTarget, compat, catalog)
  };
}

function exclusion(familyId, status, packageName, solidTarget, reason, detail, unparsedRanges = []) {
  const base = { family: familyId, status, package: packageName, solidTarget, reason, detail };
  const deduped = dedupeUnparsed(unparsedRanges);
  return deduped.length > 0 ? { ...base, unparsedRanges: deduped } : base;
}

/**
 * Selects exactly one row (a chosen package version plus the Solid
 * version(s) to probe it with) or records why the package/target pair is
 * excluded. Never both, never neither.
 */
/**
 * The version a package currently ships: its `latest` dist-tag when it has
 * one, otherwise its newest published release.
 */
function currentRelease(packument) {
  const latest = packument["dist-tags"]?.latest;
  if (latest && packument.versions?.[latest]) return { version: latest, versionDoc: packument.versions[latest] };
  const versions = sortVersions(Object.keys(packument.versions ?? {}));
  const newest = versions[versions.length - 1];
  return newest ? { version: newest, versionDoc: packument.versions[newest] } : null;
}

/**
 * Whether the package's CURRENT release still declares any Solid runtime or
 * peer dependency.
 *
 * Selection otherwise walks backwards to the newest release that accepts the
 * target, which is correct for a package that moved from Solid 1.x to 2.x --
 * its older 1.x line is genuinely the release a 1.x user installs. It is
 * wrong for a package that has LEFT the Solid ecosystem entirely:
 * @tanstack/react-router-devtools@1.121.2 declared `solid-js: ^1.9.5`, but the
 * package's current release depends on @tanstack/router-devtools-core and
 * peers on React. Walking back to that stale release put a React package into
 * a Solid benchmark and reported it as current TanStack Solid coverage.
 *
 * So a package whose current release declares no Solid dependency at all is
 * excluded outright. A package whose current release declares Solid for a
 * DIFFERENT major is not affected -- it still declares Solid, and the
 * walk-back to its newest target-compatible release is the intended behavior.
 *
 * The Solid runtime packages themselves are exempt: solid-js does not depend
 * on solid-js.
 */
function currentReleaseDeclaresSolid(packageName, packument) {
  if (SOLID_RUNTIME_PACKAGES.includes(packageName)) return true;
  const current = currentRelease(packument);
  if (!current) return false;
  const ranges = flattenRanges(solidRanges(current.versionDoc, SOLID_RUNTIME_PACKAGES));
  return Object.keys(ranges).length > 0;
}

export function selectRow({
  packageName,
  packument,
  family,
  status,
  solidTarget,
  catalog,
  auditedSolid1,
  auditedSolid2 = AUDITED_SOLID_2
}) {
  const familyId = resolveFamily(family).id;

  if (!packument || !packument.versions || Object.keys(packument.versions).length === 0) {
    return { kind: "exclusion", exclusion: exclusion(familyId, status, packageName, solidTarget, "not-published", `${packageName} has no published versions`) };
  }

  // A package that has left the Solid ecosystem is ignored rather than
  // represented by whatever stale release still mentioned Solid.
  if (!currentReleaseDeclaresSolid(packageName, packument)) {
    const current = currentRelease(packument);
    return {
      kind: "exclusion",
      exclusion: exclusion(
        familyId,
        status,
        packageName,
        solidTarget,
        "no-solid-dependency",
        `${packageName}'s current release ${current ? current.version : "(unknown)"} declares no range for ${SOLID_RUNTIME_PACKAGES.join(", ")}`
      )
    };
  }

  // solid-js is the audited runtime itself: for solid1 it is pinned to
  // auditedSolid1 exactly rather than searched for among its own releases.
  if (packageName === "solid-js" && solidTarget === "solid1") {
    const versionDoc = packument.versions[auditedSolid1];
    if (!versionDoc) {
      return {
        kind: "exclusion",
        exclusion: exclusion(familyId, status, packageName, solidTarget, "not-published", `solid-js@${auditedSolid1} is not published`)
      };
    }
    return {
      kind: "row",
      row: buildRow({
        packageName,
        familyId,
        status,
        solidTarget,
        version: auditedSolid1,
        versionDoc,
        packument,
        compatibleSolidVersions: { "solid-js": [auditedSolid1] },
        unparsedRanges: [],
        catalog
      })
    };
  }

  // The official Solid 2 runtime packages define the audited tuple itself.
  // Their row version and probe environment are therefore exact, like
  // solid-js on solid1. This is intentionally different from ecosystem rows:
  // an older floor there measures a package's declared compatibility window,
  // while mixing generations inside the official runtime would measure a
  // tuple Solid never released. Including the complete tuple also gives the
  // @solidjs/signals row a real solid-js runtime against which observations
  // can be settled instead of producing a synthetic `no-runtime` outcome.
  if (SOLID_RUNTIME_PACKAGES.includes(packageName) && solidTarget === "solid2") {
    const versionDoc = packument.versions[auditedSolid2];
    if (!versionDoc) {
      return {
        kind: "exclusion",
        exclusion: exclusion(
          familyId,
          status,
          packageName,
          solidTarget,
          "not-published",
          `${packageName}@${auditedSolid2} is not published`
        )
      };
    }
    const missingRuntime = SOLID_RUNTIME_PACKAGES.find(
      runtimePackage => !(catalog[runtimePackage]?.v2 ?? []).includes(auditedSolid2)
    );
    if (missingRuntime) {
      return {
        kind: "exclusion",
        exclusion: exclusion(
          familyId,
          status,
          packageName,
          solidTarget,
          "no-compatible-release",
          `${missingRuntime}@${auditedSolid2} is missing from the audited Solid 2 runtime tuple`
        )
      };
    }
    const auditedTuple = Object.fromEntries(
      SOLID_RUNTIME_PACKAGES.map(runtimePackage => [runtimePackage, [auditedSolid2]])
    );
    return {
      kind: "row",
      row: buildRow({
        packageName,
        familyId,
        status,
        solidTarget,
        version: auditedSolid2,
        versionDoc,
        packument,
        compatibleSolidVersions: auditedTuple,
        unparsedRanges: [],
        catalog
      })
    };
  }

  const versionIds = Object.keys(packument.versions);
  const newestFirst = [...sortVersions(versionIds)].reverse();
  const evaluate = solidTarget === "solid1"
    ? versionDoc => evaluateVersionSolid1(packageName, versionDoc, family, catalog, auditedSolid1)
    : versionDoc => evaluateVersionSolid2(packageName, versionDoc, catalog);

  const evaluations = newestFirst.map(version => {
    const versionDoc = packument.versions[version];
    return { version, versionDoc, result: evaluate(versionDoc) };
  });

  const compatible = evaluations.filter(entry => entry.result.status === "compatible");

  if (compatible.length === 0) {
    const sawDeclared = evaluations.some(entry => entry.result.status !== "no-range");
    const unparsedRanges = dedupeUnparsed(evaluations.flatMap(entry => entry.result.unparsed));
    if (!sawDeclared) {
      return {
        kind: "exclusion",
        exclusion: exclusion(
          familyId,
          status,
          packageName,
          solidTarget,
          "no-solid-dependency",
          `${packageName} declares no range for ${SOLID_RUNTIME_PACKAGES.join(", ")}`
        )
      };
    }
    if (unparsedRanges.length > 0) {
      return {
        kind: "exclusion",
        exclusion: exclusion(
          familyId,
          status,
          packageName,
          solidTarget,
          "unparsed-range",
          `${packageName} declares an unparsed range: ${unparsedRanges.map(entry => `${entry.package}@${entry.range}`).join(", ")}`,
          unparsedRanges
        )
      };
    }
    const newest = newestFirst[0];
    return {
      kind: "exclusion",
      exclusion: exclusion(
        familyId,
        status,
        packageName,
        solidTarget,
        "no-compatible-release",
        `newest ${newest} has no release of ${SOLID_RUNTIME_PACKAGES.join("/")} satisfying its declared range for ${solidTarget}`
      )
    };
  }

  // Deprecation only breaks ties among otherwise-compatible releases: prefer
  // a non-deprecated one, but a package deprecated at every compatible
  // release is still selected (at its newest) rather than excluded outright.
  const nonDeprecated = compatible.filter(entry => !isDeprecated(entry.versionDoc));
  const pool = nonDeprecated.length > 0 ? nonDeprecated : compatible;
  const selected = pool[0];

  return {
    kind: "row",
    row: buildRow({
      packageName,
      familyId,
      status,
      solidTarget,
      version: selected.version,
      versionDoc: selected.versionDoc,
      packument,
      compatibleSolidVersions: selected.result.compatibleVersions,
      unparsedRanges: selected.result.unparsed,
      catalog
    })
  };
}
