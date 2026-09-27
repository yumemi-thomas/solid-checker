// Consumer environments: the exact Solid 2 trees real consumers install, as
// reviewed data, so the compiled-in tier can carry contracts certified in them.
//
// The corpus certifies each package at its manifest floor and head. A bundle is
// admitted only where its own dependency environment is installed
// (bundle-accepted-contracts.mjs `bundleKey`), and a real consumer rarely
// installs either: kobalte's `solid2` branch pins the rc.3 runtime *with*
// `@solidjs/signals@2.0.0-rc.3`, where the corpus head installs rc.6. So every
// bundle the census run delivers is refused in the tree the census demand was
// swept from, however much it proves.
//
// An environment names that tree -- the runtime tuple, the closure pins, and
// the tier packages whose version and integrity equal a `solid2` manifest row --
// and `run.mjs --consumer-environment <id>` certifies exactly those rows in
// it. The rows are cloned from the manifest in memory; the committed manifest
// is never changed. Such a run is *delivery only* (owner decision, 2026-09-26):
// the census refuses it, and its contracts are measured by re-sweeping the
// consumer it came from rather than by the census pin.
//
// Everything here refuses rather than adapts. A package whose version or
// integrity differs from its manifest row is a different artifact from the one
// the row describes; a runtime package that is not an audited archive, or that
// is newer than the audited Solid 2 release, is a runtime no audit has read.

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { AUDITED_SOLID_2, SOLID_RUNTIME_PACKAGES } from "./families.mjs";
import { prereleaseChannel } from "./select.mjs";
import { compareVersions } from "./semver.mjs";

export const CONSUMER_ENVIRONMENTS_PATH = fileURLToPath(
  new URL("../consumer-environments.json", import.meta.url)
);

const ID = /^[a-z0-9][a-z0-9.-]*$/;
const COMMIT = /^[0-9a-f]{40}$/;
const DIGEST = /^sha256:[0-9a-f]{64}$/;
const INTEGRITY = /^sha512-[A-Za-z0-9+/]+={0,2}$/;

// The runtime packages whose release `AUDITED_SOLID_2` names. `@solidjs/signals`
// is versioned apart from them (a floor installs the rc.9 head's signals beside
// an rc.6 or rc.8 `solid-js`, and kobalte pins rc.3 beside rc.3), so it is held
// to the audited archives alone. The ceiling only refuses a release above the
// audited one; at or below it, a runtime is admitted by being an audited archive
// with that archive's integrity, and nothing else.
const CEILINGED_RUNTIME = ["solid-js", "@solidjs/web"];

/**
 * The reviewed environments, or a throw. Strict about shape for the reason
 * `loadAuditedArchives` is: a file that parses but lists nothing must not read
 * as "no environment disagrees".
 */
export function loadConsumerEnvironments(path = CONSUMER_ENVIRONMENTS_PATH) {
  const document = JSON.parse(readFileSync(path, "utf8"));
  if (document?.schemaVersion !== 1) {
    throw new Error(`${path}: unsupported schemaVersion ${document?.schemaVersion}`);
  }
  if (!Array.isArray(document.environments)) {
    throw new Error(`${path}: no environments array`);
  }
  const seen = new Set();
  for (const environment of document.environments) {
    if (typeof environment?.id !== "string" || !ID.test(environment.id)) {
      throw new Error(`${path}: environment id ${JSON.stringify(environment?.id)} is not a lowercase slug`);
    }
    if (seen.has(environment.id)) throw new Error(`${path}: environment ${environment.id} is listed twice`);
    seen.add(environment.id);
  }
  return document;
}

function solidV2Archives(auditedArchives) {
  const dialect = (auditedArchives?.dialects ?? []).find(entry => entry.id === "solid-v2");
  return dialect?.archives ?? [];
}

function exactIdentity(entry) {
  return (
    entry &&
    typeof entry.version === "string" &&
    entry.version.length > 0 &&
    typeof entry.integrity === "string" &&
    INTEGRITY.test(entry.integrity)
  );
}

/**
 * Every reason an environment cannot be run, or `[]`. Pure: the manifest and
 * the audited archives are handed in.
 */
export function environmentProblems(
  environment,
  { manifest, auditedArchives, auditedSolid2 = AUDITED_SOLID_2 }
) {
  const problems = [];
  const id = environment?.id ?? "(unnamed)";
  const source = environment?.source ?? {};
  if (typeof source.repository !== "string" || !source.repository) problems.push(`${id}: source.repository is missing`);
  if (typeof source.branch !== "string" || !source.branch) problems.push(`${id}: source.branch is missing`);
  if (typeof source.commit !== "string" || !COMMIT.test(source.commit)) {
    problems.push(`${id}: source.commit is not a full 40-character commit`);
  }
  if (typeof source.lockfileDigest !== "string" || !DIGEST.test(source.lockfileDigest)) {
    problems.push(`${id}: source.lockfileDigest is not a sha256 digest`);
  }

  const runtime = environment?.runtime ?? {};
  const archives = solidV2Archives(auditedArchives);
  if (archives.length === 0) problems.push(`${id}: the audited archives carry no solid-v2 dialect`);
  for (const name of SOLID_RUNTIME_PACKAGES) {
    const pin = runtime[name];
    if (!exactIdentity(pin)) {
      problems.push(`${id}: runtime ${name} needs an exact version and sha512 integrity`);
      continue;
    }
    if (CEILINGED_RUNTIME.includes(name) && compareVersions(pin.version, auditedSolid2) > 0) {
      problems.push(
        `${id}: runtime ${name}@${pin.version} is above the audited Solid 2 release ${auditedSolid2}`
      );
      continue;
    }
    const archive = archives.find(entry => entry.name === name && entry.version === pin.version);
    if (!archive) {
      problems.push(`${id}: runtime ${name}@${pin.version} is not an audited archive`);
    } else if (archive.integrity !== pin.integrity) {
      problems.push(
        `${id}: runtime ${name}@${pin.version} integrity ${pin.integrity} is not the audited archive's ${archive.integrity}`
      );
    }
  }
  for (const name of Object.keys(runtime)) {
    if (!SOLID_RUNTIME_PACKAGES.includes(name)) problems.push(`${id}: runtime names ${name}, which is not a Solid runtime package`);
  }

  const pins = environment?.pins ?? {};
  for (const [name, pin] of Object.entries(pins)) {
    if (SOLID_RUNTIME_PACKAGES.includes(name)) {
      problems.push(`${id}: pin ${name} belongs in runtime, not pins`);
    } else if (!exactIdentity(pin)) {
      problems.push(`${id}: pin ${name} needs an exact version and sha512 integrity`);
    }
  }

  const rows = new Map();
  for (const row of manifest?.rows ?? []) {
    if (row.solidTarget === "solid2") rows.set(row.package, row);
  }
  const packages = Array.isArray(environment?.packages) ? environment.packages : [];
  if (packages.length === 0) problems.push(`${id}: lists no packages`);
  const listed = new Set();
  for (const entry of packages) {
    const name = entry?.package;
    if (typeof name !== "string" || !exactIdentity(entry)) {
      problems.push(`${id}: package ${JSON.stringify(name)} needs an exact version and sha512 integrity`);
      continue;
    }
    if (listed.has(name)) {
      problems.push(`${id}: package ${name} is listed twice`);
      continue;
    }
    listed.add(name);
    if (SOLID_RUNTIME_PACKAGES.includes(name)) {
      problems.push(`${id}: ${name} is the runtime, not a tier package`);
      continue;
    }
    const row = rows.get(name);
    if (!row) {
      problems.push(`${id}: ${name}@${entry.version} has no solid2 manifest row`);
      continue;
    }
    if (row.version !== entry.version) {
      problems.push(`${id}: ${name}@${entry.version} is not the manifest row's ${row.version}`);
    } else if (row.integrity !== entry.integrity) {
      problems.push(
        `${id}: ${name}@${entry.version} integrity ${entry.integrity} is not the manifest row's ${row.integrity}`
      );
    }
    const pin = pins[name];
    if (pin && (pin.version !== entry.version || pin.integrity !== entry.integrity)) {
      problems.push(`${id}: ${name} is pinned at ${pin.version} but listed at ${entry.version}`);
    }
  }
  return problems;
}

/// The probe id a consumer-environment clone of a row runs under.
export function environmentProbeId(row, environment) {
  return `${row.package}@${row.version}|solid2|environment-${environment.id}`;
}

/**
 * The manifest a `--consumer-environment` run executes: one row per listed
 * package, cloned from its `solid2` manifest row, each with a single
 * `kind: "environment"` probe that installs the environment's runtime tuple
 * and carries its pins. Throws, naming every problem, when the environment
 * disagrees with the manifest or the audited archives.
 */
export function consumerEnvironmentManifest(manifest, environment, options = {}) {
  const problems = environmentProblems(environment, { manifest, ...options });
  if (problems.length) {
    throw new Error(
      `consumer environment ${environment?.id ?? "(unnamed)"} refused:\n  - ${problems.join("\n  - ")}`
    );
  }
  const runtime = Object.fromEntries(
    SOLID_RUNTIME_PACKAGES.map(name => [
      name,
      { version: environment.runtime[name].version, integrity: environment.runtime[name].integrity }
    ])
  );
  const pins = Object.fromEntries(
    Object.keys(environment.pins ?? {})
      .sort()
      .map(name => [name, { version: environment.pins[name].version, integrity: environment.pins[name].integrity }])
  );
  const rows = environment.packages.map(entry => {
    const row = manifest.rows.find(candidate => candidate.solidTarget === "solid2" && candidate.package === entry.package);
    const entrypoints = (row.probes ?? []).find(probe => Array.isArray(probe.entrypoints))?.entrypoints;
    const probe = {
      id: environmentProbeId(row, environment),
      kind: "environment",
      channel: prereleaseChannel(runtime["solid-js"].version),
      solid: Object.fromEntries(SOLID_RUNTIME_PACKAGES.map(name => [name, runtime[name].version])),
      environment: { id: environment.id, runtime, pins },
      ...(entrypoints ? { entrypoints: [...entrypoints] } : {})
    };
    return { ...row, probes: [probe] };
  });
  return { ...manifest, rows, supplemental: [], exclusions: [] };
}

const PATCHED_REASON =
  "patched by the consumer (pnpm patchedDependencies): the installed bytes are not the published archive";

function stripPeers(version) {
  return typeof version === "string" ? version.split("(")[0] : version;
}

// The lock key a dependency resolves to. pnpm writes an npm alias
// (`"h3-v2": "npm:h3@2.0.1-rc.20"`) as `h3-v2: h3@2.0.1-rc.20`: the value names
// the real package, which is what `packages` is keyed by and what a pin (read
// back through the installed lock's own `name@version`) is about.
function dependencyKey(name, version) {
  return stripPeers(version).includes("@") ? version : `${name}@${version}`;
}

function nameOf(key) {
  const base = stripPeers(key);
  const at = base.lastIndexOf("@");
  return at > 0 ? base.slice(0, at) : base;
}

function versionOf(key) {
  const base = stripPeers(key);
  const at = base.lastIndexOf("@");
  return at > 0 ? base.slice(at + 1) : null;
}

/**
 * An environment entry from a parsed pnpm v9 lockfile, for review.
 *
 * - `runtime`: the one Solid 2 release of each runtime package the lock
 *   records, with its integrity.
 * - `packages`: every package in the dependency closure of `importers` whose
 *   version and integrity equal a `solid2` manifest row, unless the consumer
 *   patches it or anything its own install reaches (pnpm
 *   `patchedDependencies`): the lock keeps the published integrity for a
 *   patched package, so identity alone would deliver a contract about bytes
 *   the consumer does not run.
 * - `pins`: the closure of those packages and of the runtime, minus the
 *   runtime itself, as the lock resolves it.
 * - `unmatched`: every closure package that has a `solid2` manifest row, or is
 *   under a scope a tier package lives in, and does not match it -- with why.
 *
 * Pure. `environmentProblems` still judges the result; deriving it proves
 * nothing about the runtime.
 */
export function deriveConsumerEnvironment({ lock, id, source, importers, manifest }) {
  const packages = lock?.packages ?? {};
  const snapshots = lock?.snapshots ?? {};
  const integrityOf = key => packages[stripPeers(key)]?.resolution?.integrity ?? null;

  const runtime = {};
  for (const name of SOLID_RUNTIME_PACKAGES) {
    const releases = Object.keys(packages).filter(key => nameOf(key) === name && versionOf(key)?.startsWith("2."));
    if (releases.length !== 1) {
      throw new Error(`${name}: the lock records ${releases.length} Solid 2 releases, not one`);
    }
    runtime[name] = { version: versionOf(releases[0]), integrity: integrityOf(releases[0]) };
  }

  // pnpm applies `patchedDependencies` to the installed files but keeps the
  // published archive's integrity in `resolution`, so a patched package matches
  // its manifest row by version and integrity while the consumer runs other
  // bytes. The lock names every patch at the top level and in the snapshot key
  // (`(patch_hash=...)`); either one makes the package patched.
  const patched = new Set(Object.keys(lock?.patchedDependencies ?? {}).map(stripPeers));
  for (const key of Object.keys(snapshots)) if (key.includes("(patch_hash=")) patched.add(stripPeers(key));

  const closureOf = roots => {
    const seen = new Map();
    const stack = [...roots];
    while (stack.length) {
      const key = stack.pop();
      const base = stripPeers(key);
      if (seen.has(base)) continue;
      seen.set(base, integrityOf(key));
      const snapshot = snapshots[key] ?? snapshots[base] ?? {};
      for (const [dependency, version] of Object.entries({
        ...(snapshot.dependencies ?? {}),
        ...(snapshot.optionalDependencies ?? {})
      })) {
        if (typeof version === "string" && !version.startsWith("link:")) stack.push(dependencyKey(dependency, version));
      }
    }
    return seen;
  };

  const importerRoots = [];
  for (const importer of importers) {
    const entry = lock?.importers?.[importer];
    if (!entry) throw new Error(`the lock has no importer ${importer}`);
    for (const [name, dependency] of Object.entries({
      ...(entry.dependencies ?? {}),
      ...(entry.optionalDependencies ?? {})
    })) {
      const version = dependency?.version;
      if (typeof version === "string" && !version.startsWith("link:")) importerRoots.push(dependencyKey(name, version));
    }
  }
  const consumerClosure = closureOf(importerRoots);

  const rows = new Map();
  for (const row of manifest?.rows ?? []) if (row.solidTarget === "solid2") rows.set(row.package, row);
  const scopes = new Set(
    [...rows.keys()].map(name => (name.startsWith("@") ? name.split("/")[0] : null)).filter(Boolean)
  );

  const snapshotKeyOf = base => Object.keys(snapshots).find(candidate => stripPeers(candidate) === base) ?? base;

  // Every delivered package runs against the runtime, so a patch there leaves
  // nothing this entry could deliver: refuse the lock rather than record it.
  const runtimePatched = [
    ...closureOf(SOLID_RUNTIME_PACKAGES.map(name => snapshotKeyOf(`${name}@${runtime[name].version}`))).keys()
  ].filter(base => patched.has(base));
  if (runtimePatched.length) {
    throw new Error(`the consumer patches ${runtimePatched.sort().join(", ")}, which the Solid runtime installs`);
  }

  const candidates = [];
  const unmatched = [];
  for (const [key, integrity] of [...consumerClosure].sort(([left], [right]) => left.localeCompare(right))) {
    const name = nameOf(key);
    const version = versionOf(key);
    if (SOLID_RUNTIME_PACKAGES.includes(name)) continue;
    const row = rows.get(name);
    if (row && row.version === version && row.integrity === integrity) {
      if (patched.has(key)) unmatched.push({ package: name, version, integrity, reason: PATCHED_REASON });
      else candidates.push({ package: name, version, integrity });
      continue;
    }
    const scope = name.startsWith("@") ? name.split("/")[0] : null;
    if (!row && !(scope && scopes.has(scope))) continue;
    unmatched.push({
      package: name,
      version,
      integrity,
      reason: !row
        ? "no solid2 manifest row"
        : row.version !== version
          ? `the solid2 manifest row is ${row.version}`
          : `the solid2 manifest row's integrity is ${row.integrity}`
    });
  }

  // A package whose install reaches a patched one would be certified, by a
  // delivery run, in a tree where that dependency is the published archive --
  // not the tree the consumer installs -- so it is recorded, not delivered.
  const matched = [];
  for (const entry of candidates) {
    const reached = [...closureOf([snapshotKeyOf(`${entry.package}@${entry.version}`)]).keys()]
      .filter(base => patched.has(base))
      .sort();
    if (reached.length) {
      unmatched.push({ ...entry, reason: `its dependency closure installs ${reached.join(", ")}, which the consumer patches` });
    } else {
      matched.push(entry);
    }
  }
  unmatched.sort((left, right) => left.package.localeCompare(right.package));

  const pinClosure = closureOf([
    ...matched.map(entry => snapshotKeyOf(`${entry.package}@${entry.version}`)),
    ...SOLID_RUNTIME_PACKAGES.map(name => snapshotKeyOf(`${name}@${runtime[name].version}`))
  ]);
  const pins = {};
  for (const [key, integrity] of [...pinClosure].sort(([left], [right]) => left.localeCompare(right))) {
    const name = nameOf(key);
    if (SOLID_RUNTIME_PACKAGES.includes(name)) continue;
    if (pins[name] && pins[name].version !== versionOf(key)) {
      throw new Error(`${name}: the closure resolves two releases, ${pins[name].version} and ${versionOf(key)}`);
    }
    pins[name] = { version: versionOf(key), integrity };
  }

  return { id, source: { ...source, importers: [...importers] }, runtime, pins, packages: matched, unmatched };
}
