#!/usr/bin/env bun
import { spawn, spawnSync } from "node:child_process";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import {
  expandContract,
  normalizeContract,
} from "../packages/cli/scripts/contract-document.mjs";
import { loadDialectManifests, root } from "./dialect-manifests.mjs";
import { packageIntegrity } from "./lib/package-integrity.mjs";
import { gateConcurrency, mapPool } from "./lib/pool.mjs";

// Probing is grouped by dialect, and each group installs into its own root.
// One shared install cannot host them: @solid-primitives/scheduled peers on
// solid-js@^1.6.12 while the 2.0 contracts pin 2.0.0-rc.0, and Bun refuses the
// combination outright. A dialect's non-probed packages are installed
// alongside its probed ones so those peers resolve to the audited release
// rather than whatever Bun would pick.
const probeModes = [
  { name: "client", conditions: ["browser"] },
  { name: "server", conditions: ["node"] },
  { name: "development", conditions: ["browser", "development"] },
  { name: "production", conditions: ["browser", "production"] },
];

/**
 * The condition modes a contract's claims are stated for.
 *
 * A contract may deliberately describe fewer than all four. Solid 1.x resolves
 * a genuinely different artifact under `node` — one where createEffect never
 * runs and memos never re-run — so a contract that states client semantics is
 * not making a claim about it. Restricting the modes records that boundary
 * instead of leaving the suite to check a claim the contract never made; the
 * server build needs its own contract, which is a separate artifact and not yet
 * written.
 */
const contractModes = contract =>
  contract.probeModes
    ? probeModes.filter(mode => contract.probeModes.includes(mode.name))
    : probeModes;
let failures = 0;
const fail = message => {
  failures++;
  console.error(`FAIL ${message}`);
};
const pass = message => console.log(`ok   ${message}`);
const runtimeLockPath = join(root, "pkg/contracts/bundled/runtime-lock.json");
let runtimeLock = { schemaVersion: 1, packages: {} };
try {
  runtimeLock = JSON.parse(readFileSync(runtimeLockPath, "utf8"));
} catch (error) {
  fail(`cannot read ${runtimeLockPath}: ${error.message}`);
}
if (runtimeLock.schemaVersion !== 1 || !runtimeLock.packages || typeof runtimeLock.packages !== "object") {
  fail(`${runtimeLockPath} must use runtime-lock schemaVersion 1`);
}

const manifests = loadDialectManifests({ requireArtifacts: true });
// A dialect may need several workers: the 1.x core and the scheduled overlay
// import different packages, and a worker's bare imports are what tie it to the
// install it runs in. Every worker of a dialect runs in every condition mode.
const probeWorkers = {
  "solid-v1": [
    "scripts/contract-probes-solid-v1-core.mjs",
    "scripts/contract-probes-solid-v1.mjs",
    "scripts/contract-probes-solid-v1-debounce.mjs",
    "scripts/contract-probes-solid-v1-rootless.mjs",
  ],
  "solid-v2": [
    "scripts/contract-probes.mjs",
    "scripts/contract-probes-solid-v2-web-root.mjs",
    "scripts/contract-probes-solid-v2-web.mjs",
  ],
};
const definitions = manifests.flatMap(manifest =>
  manifest.contracts
    .filter(contract => contract.probeRuntime)
    .map(contract => ({
      file: contract.bundledContract,
      name: contract.package,
      dialect: manifest.id,
      modes: contractModes(contract),
    })),
);
const peerDefinitions = manifests.flatMap(manifest =>
  manifest.contracts
    .filter(contract => !contract.probeRuntime)
    .map(contract => ({
      file: contract.bundledContract,
      name: contract.package,
      dialect: manifest.id,
    })),
);
const write = process.argv.includes("--write");
function modeApplies(entrypoint, mode) {
  const conditions = new Set(entrypoint.conditions ?? []);
  const environment = new Set(["browser", "node", "client", "server", "development", "production"]);
  const selected = [...environment].filter(condition => conditions.has(condition));
  if (selected.length === 0) return true;
  if (
    conditions.has("development") &&
    !conditions.has("browser") &&
    !conditions.has("node") &&
    !conditions.has("client") &&
    !conditions.has("server")
  ) {
    return mode.name === "development";
  }
  if (
    conditions.has("production") &&
    !conditions.has("browser") &&
    !conditions.has("node") &&
    !conditions.has("client") &&
    !conditions.has("server")
  ) {
    return mode.name === "production";
  }
  if (mode.name === "server") return conditions.has("server") || conditions.has("node");
  if (mode.name === "client") return conditions.has("client") || conditions.has("browser");
  if (mode.name === "development") {
    return conditions.has("development") || conditions.has("client") || conditions.has("browser");
  }
  if (mode.name === "production") {
    return conditions.has("production") || conditions.has("client") || conditions.has("browser");
  }
  return selected.some(condition => mode.conditions.includes(condition));
}

/**
 * The callback rows a summary states, or none when the field is the schema's
 * unknown sentinel.
 *
 * `{ "status": "unknown" }` is a valid schema-v1 value for `callbacks`, and it
 * is the *opposite* of a row set: it claims nothing, so there is nothing to
 * probe and no negative for discovery to contradict. Reading it as an array
 * threw, which made the one honest encoding for a claim a contract cannot state
 * unusable in a bundled artifact.
 */
const callbackRows = summary =>
  Array.isArray(summary?.callbacks) ? summary.callbacks : [];

const callbacksAreUnknown = summary =>
  Boolean(summary?.callbacks) && !Array.isArray(summary.callbacks);

// This bundled-contract harness consumes package-specific oracle results, whose
// historical claim vocabulary has no relational-return spellings. The generic
// `contract probe` driver does establish these relations with parameter-indexed
// strict-identity probes; omitting them here prevents an unrelated oracle row
// from being mistaken for that evidence.
const returnClaim = summary => {
  const kind = summary?.returns?.kind;
  return typeof kind === "string" &&
    !["argument", "callback-result", "callback-result-function"].includes(kind)
    ? `returns=${kind}`
    : undefined;
};

function probeEvidence(resultsForClaim) {
  if (resultsForClaim.length === 0 || resultsForClaim.some(result => !result.ok)) {
    return undefined;
  }
  return {
    kind: "probed",
    modes: [...new Set(resultsForClaim.map(result => result.mode))].sort(),
    calls: Math.max(...resultsForClaim.map(result => result.calls ?? 1)),
  };
}

function conditionsMatchMode(conditions, mode) {
  const active = new Set([...mode.conditions, "import"]);
  return conditions.every(condition => condition === "default" || active.has(condition));
}

function summaryForMode(summary, mode) {
  if (!summary.variants?.length) return summary;
  const matches = summary.variants
    .filter(variant => conditionsMatchMode(variant.conditions, mode))
    .sort((left, right) => right.conditions.length - left.conditions.length);
  if (!matches.length) return undefined;
  const mostSpecific = matches.filter(
    variant => variant.conditions.length === matches[0].conditions.length,
  );
  if (mostSpecific.length > 1) {
    const canonical = new Set(mostSpecific.map(variant => JSON.stringify(variant.summary)));
    if (canonical.size > 1) return undefined;
  }
  return mostSpecific[0].summary;
}

function writeProbeEvidence(summary, dialect, packageName, entrypoint, name, allowedModes = probeModes) {
  const claimResults = claim =>
    observed.probes.filter(
      result =>
        result.dialect === dialect &&
        result.pkg === packageName &&
        result.entrypoint === entrypoint &&
        result.name === name &&
        result.claim === claim &&
        allowedModes.some(mode => mode.name === result.mode),
    );
  const next = { ...summary };
  const exportResults = [
    ...callbackRows(summary).map(callback =>
      claimResults(`callbacks[${callback.parameter}]=${callback.execution}`),
    ),
    ...(returnClaim(summary) ? [claimResults(returnClaim(summary))] : []),
  ].flat();
  const evidence = probeEvidence(exportResults);
  if (evidence && (!next.evidence || next.evidence.kind === "inferred")) {
    next.evidence = evidence;
  }
  if (Array.isArray(summary.callbacks)) {
    next.callbacks = summary.callbacks.map(callback => {
      const callbackEvidence = probeEvidence(
        claimResults(`callbacks[${callback.parameter}]=${callback.execution}`),
      );
      return callbackEvidence && (!callback.evidence || callback.evidence.kind === "inferred")
        ? { ...callback, evidence: callbackEvidence }
        : callback;
    });
  }
  if (returnClaim(summary)) {
    const returnEvidence = probeEvidence(claimResults(returnClaim(summary)));
    if (returnEvidence && (!summary.returns.evidence || summary.returns.evidence.kind === "inferred")) {
      next.returns = { ...summary.returns, evidence: returnEvidence };
    }
  }
  if (summary.variants?.length) {
    next.variants = summary.variants.map(variant => ({
      ...variant,
      summary: writeProbeEvidence(
        variant.summary,
        dialect,
        packageName,
        entrypoint,
        name,
        probeModes.filter(mode => conditionsMatchMode(variant.conditions, mode)),
      ),
    }));
  }
  return next;
}

const contracts = definitions.map(definition => {
  const path = join(root, definition.file);
  const contract = expandContract(JSON.parse(readFileSync(path, "utf8")));
  if (contract.package.name !== definition.name) {
    throw new Error(`${definition.file} declares ${contract.package.name}`);
  }
  return { ...definition, path, contract };
});

const contractDeclaresDefault = contract =>
  Object.values(contract.entrypoints).some(entrypoint =>
    Object.hasOwn(entrypoint.exports ?? {}, "default"),
  );

/** The exact release a declared contract pins, for install specifiers. */
const pinnedVersion = definition =>
  JSON.parse(readFileSync(join(root, definition.file), "utf8")).package?.version;

/** One install root per dialect, with the probe worker that can drive it. */
const installations = manifests
  .filter(manifest => contracts.some(contract => contract.dialect === manifest.id))
  .map(manifest => {
    const probed = contracts.filter(contract => contract.dialect === manifest.id);
    const peers = peerDefinitions
      .filter(definition => definition.dialect === manifest.id)
      .map(definition => ({ name: definition.name, version: pinnedVersion(definition) }))
      .filter(peer => peer.version);
    const specifiers = [
      ...probed.map(({ name, contract }) => `${name}@${contract.package.version}`),
      ...peers.map(peer => `${peer.name}@${peer.version}`),
    ].sort();
    const workers = probeWorkers[manifest.id];
    if (!workers?.length) {
      throw new Error(
        `${manifest.id} declares probeRuntime contracts but has no probe worker; add one to probeWorkers`,
      );
    }
    const cacheKey = specifiers.join("_").replace(/[^\w.@-]+/g, "-");
    const directory = join(tmpdir(), `solid-checker-contract-conformance-${cacheKey}`);
    mkdirSync(directory, { recursive: true });
    writeFileSync(
      join(directory, "package.json"),
      `${JSON.stringify({
        name: `solid-checker-contract-conformance-${manifest.id}`,
        version: "0.0.0",
        private: true,
      }, null, 2)}\n`,
    );
    return { dialect: manifest.id, probed, specifiers, workers, directory };
  });

const readInstalledManifest = (directory, name) => {
  const path = join(directory, "node_modules", ...name.split("/"), "package.json");
  return existsSync(path) ? JSON.parse(readFileSync(path, "utf8")) : undefined;
};

const probeTasks = [];
for (const installation of installations) {
  const satisfied = installation.probed.every(
    ({ name, contract }) =>
      readInstalledManifest(installation.directory, name)?.version === contract.package.version &&
      packageIntegrity(installation.directory, name),
  );
  if (!satisfied) {
    const result = spawnSync(
      "bun",
      [
        "install",
        "--ignore-scripts",
        "--no-progress",
        ...installation.specifiers,
      ],
      { cwd: installation.directory, stdio: "inherit" },
    );
    if (result.status !== 0) process.exit(result.status ?? 1);
  }

  // Workers run from inside the install so their bare imports resolve to that
  // dialect's releases; the shared harness has to travel with them.
  mkdirSync(join(installation.directory, "lib"), { recursive: true });
  copyFileSync(
    join(root, "scripts/lib/contract-probe-harness.mjs"),
    join(installation.directory, "lib", "contract-probe-harness.mjs"),
  );
  const packages = installation.probed.map(({ name }) => ({
    name,
    directory: join(installation.directory, "node_modules", ...name.split("/")),
    includeDefault: contractDeclaresDefault(
      installation.probed.find(contract => contract.name === name).contract,
    ),
  }));
  const dialectModes = probeModes.filter(mode =>
    installation.probed.some(contract => contract.modes.includes(mode)),
  );
  for (const source of installation.workers) {
    const worker = join(installation.directory, source.split("/").at(-1));
    copyFileSync(join(root, source), worker);
    for (const mode of dialectModes) {
      probeTasks.push({ installation, worker, mode, packages });
    }
  }
}

const runProbe = ({ installation, worker, mode, packages }) => new Promise((resolve, reject) => {
  const child = spawn(
    "bun",
    [
      ...mode.conditions.flatMap(condition => ["--conditions", condition]),
      worker,
      JSON.stringify({ mode: mode.name, packages }),
    ],
    { encoding: "utf8" },
  );
  const stdout = [];
  const stderr = [];
  child.stdout.on("data", chunk => stdout.push(chunk));
  child.stderr.on("data", chunk => stderr.push(chunk));
  child.once("error", reject);
  child.once("close", (status, signal) => {
    const errorOutput = Buffer.concat(stderr).toString("utf8");
    if (status !== 0) {
      reject(new Error(errorOutput || `${worker} exited ${status ?? signal}`));
      return;
    }
    try {
      resolve({
        dialect: installation.dialect,
        ...JSON.parse(Buffer.concat(stdout).toString("utf8")),
      });
    } catch (error) {
      reject(new Error(`${worker} returned invalid JSON: ${error.message}\n${errorOutput}`));
    }
  });
});

// Each worker/mode pair is a separate process over a read-only installation.
// Preserve task order in the observations while avoiding 28 serial Bun
// startups; the shared gate cap prevents the process tree from oversubscribing.
const observations = await mapPool(probeTasks, runProbe, {
  concurrency: gateConcurrency(),
});

/** The install root for one dialect's probed contracts. */
const installationOfDialect = dialect =>
  installations.find(installation => installation.dialect === dialect);

/**
 * The install that holds a package at an exact version.
 *
 * solid-js is installed in both roots at different versions, so a name-only
 * lookup would answer with whichever dialect sorted first and compare the 2.0
 * lock entry against the 1.x tree.
 */
const installationWithVersion = (name, version) =>
  installations.find(
    installation => readInstalledManifest(installation.directory, name)?.version === version,
  );

const installedVersionIn = (installation, name) =>
  readInstalledManifest(installation?.directory ?? "", name)?.version ?? null;
/** Probed packages are identified by dialect and name: solid-js is declared by
 * both dialects, at different versions, so a name-only key would merge them. */
const probeKey = (dialect, name) => `${dialect}/${name}`;
const observed = {
  packages: Object.fromEntries(
    contracts.map(({ dialect, name }) => {
      const surfaces = observations
        .filter(observation => observation.dialect === dialect)
        .map(observation => observation.packages[name])
        .filter(Boolean);
      const entrypoints = {};
      for (const [index, surface] of surfaces.entries()) {
        for (const [entrypoint, value] of Object.entries(surface.entrypoints)) {
          const current = entrypoints[entrypoint] ?? {
            exports: {},
            conditions: [],
          };
          for (const [name, kind] of Object.entries(value.exports)) {
            if (current.exports[name] && current.exports[name] !== kind) {
              fail(
                `${name}@${entrypoint} has runtime kinds ${current.exports[name]} and ${kind} across ${probeModes[index].name} and an earlier mode`,
              );
            }
            current.exports[name] ??= kind;
          }
          current.conditions = [...new Set([...current.conditions, ...value.conditions])].sort();
          entrypoints[entrypoint] = current;
        }
      }
      return [probeKey(dialect, name), { version: surfaces[0]?.version, entrypoints }];
    }),
  ),
  probes: observations.flatMap(observation =>
    observation.probes.map(probe => ({ ...probe, dialect: observation.dialect })),
  ),
  discoveredClaims: observations.flatMap(observation =>
    (observation.discoveredClaims ?? []).map(claim => ({
      ...claim,
      dialect: observation.dialect,
    })),
  ),
};
const installedIntegrityIn = (installation, name) =>
  packageIntegrity(installation?.directory ?? "", name);

function splitPackageVersion(identifier) {
  const separator = identifier.lastIndexOf("@");
  if (separator <= 0 || separator === identifier.length - 1) return undefined;
  return { name: identifier.slice(0, separator), version: identifier.slice(separator + 1) };
}

function checkRuntimeLock() {
  for (const [identifier, record] of Object.entries(runtimeLock.packages ?? {})) {
    const expected = splitPackageVersion(identifier);
    if (!expected || !record || typeof record !== "object") {
      fail(`${runtimeLockPath} has malformed package entry ${identifier}`);
      continue;
    }
    // Resolve the entry in the install that actually holds that exact release,
    // so a lock entry is checked against the tree it describes.
    const installation = installationWithVersion(expected.name, expected.version);
    if (!installation) {
      const seen = installations
        .map(candidate => installedVersionIn(candidate, expected.name))
        .filter(Boolean);
      fail(
        `${runtimeLockPath} pins ${identifier}, installed ${seen.length ? seen.join(", ") : "nowhere"}`,
      );
      continue;
    }
    const manifest = readInstalledManifest(installation.directory, expected.name);
    if (!manifest) {
      fail(`${runtimeLockPath} package ${identifier} is not installed`);
      continue;
    }
    for (const [dependencyKind, edges] of [
      ["dependencies", record.dependencies ?? {}],
      ["peerDependencies", record.peerDependencies ?? {}],
    ]) {
      for (const name of Object.keys(manifest[dependencyKind] ?? {})) {
        if (!(name in edges)) {
          fail(`${runtimeLockPath} has no pinned ${dependencyKind} edge ${identifier} -> ${name}`);
        }
      }
      for (const [name, edge] of Object.entries(edges)) {
        const declared = manifest[dependencyKind]?.[name];
        if (!edge || typeof edge !== "object" || !edge.range || !edge.version || !edge.integrity) {
          fail(`${runtimeLockPath} has malformed ${dependencyKind} edge ${identifier} -> ${name}`);
          continue;
        }
        if (declared !== edge.range) {
          fail(
            `${runtimeLockPath} expects ${identifier} ${dependencyKind} ${name}@${edge.range}, installed manifest declares ${declared ?? "missing"}`,
          );
        }
        const resolvedVersion = installedVersionIn(installation, name);
        const resolvedIntegrity = installedIntegrityIn(installation, name);
        if (resolvedVersion !== edge.version || resolvedIntegrity !== edge.integrity) {
          fail(
            `${runtimeLockPath} expects ${name}@${edge.version} (${edge.integrity}), installed ${name}@${resolvedVersion ?? "missing"} (${resolvedIntegrity ?? "no integrity"})`,
          );
        }
      }
    }
  }
}

checkRuntimeLock();

if (write) {
  for (const item of contracts) {
    const runtime = observed.packages[probeKey(item.dialect, item.name)];
    item.contract.package.version = runtime.version;
    const integrity = installedIntegrityIn(installationOfDialect(item.dialect), item.name);
    if (integrity) item.contract.package.integrity = integrity;
    item.contract.entrypoints = Object.fromEntries(
      Object.entries(item.contract.entrypoints)
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([entrypoint, surface]) => {
          const runtimeSurface = runtime.entrypoints[entrypoint];
          const exports = Object.fromEntries(
            Object.entries(surface.exports).map(([name, summary]) => [
              name,
              runtimeSurface?.exports[name]
                ? writeProbeEvidence(summary, item.dialect, item.name, entrypoint, name)
                : summary,
            ]),
          );
          return [entrypoint, { ...surface, exports }];
        }),
    );
    writeFileSync(
      item.path,
      `${JSON.stringify(normalizeContract(item.contract), null, 2)}\n`,
    );
  }
}

for (const item of contracts) {
  const runtime = observed.packages[probeKey(item.dialect, item.name)];
  if (runtime.version !== item.contract.package.version) {
    fail(
      `${item.file} pins ${item.contract.package.version}, installed ${runtime.version}`,
    );
  }
  if (!item.contract.package.integrity) {
    fail(`${item.file} does not pin package integrity`);
  } else if (
    item.contract.package.integrity !==
    installedIntegrityIn(installationOfDialect(item.dialect), item.name)
  ) {
    fail(`${item.file} package integrity does not match the installed release`);
  }
  const contractEntrypoints = item.contract.entrypoints ?? {};
  for (const [entrypoint, surface] of Object.entries(runtime.entrypoints)) {
    const contracted = contractEntrypoints[entrypoint]?.exports;
    if (!contracted) {
      fail(`${item.file} is missing entrypoint ${entrypoint}`);
      continue;
    }
    const missing = Object.keys(surface.exports).filter(name => !(name in contracted));
    const stale = Object.keys(contracted).filter(name => !(name in surface.exports));
    for (const name of missing) fail(`${item.file} ${entrypoint} misses export ${name}`);
    for (const name of stale) fail(`${item.file} ${entrypoint} has stale export ${name}`);
    for (const [name, summary] of Object.entries(contracted)) {
      if (surface.exports[name] && surface.exports[name] !== summary.kind) {
        fail(
          `${item.file} ${entrypoint}:${name} is ${summary.kind}, runtime is ${surface.exports[name]}`,
        );
      }
    }
  }
  for (const entrypoint of Object.keys(contractEntrypoints)) {
    if (!(entrypoint in runtime.entrypoints)) {
      fail(`${item.file} has stale entrypoint ${entrypoint}`);
    }
  }
  if (failures === 0) {
    pass(
      `${item.file} covers ${Object.keys(runtime.entrypoints).length} runtime entrypoints`,
    );
  }
}

const results = new Map();
for (const result of observed.probes) {
  const key = `${result.dialect}:${result.pkg}:${result.entrypoint}:${result.name}:${result.claim}`;
  const modeResults = results.get(key) ?? [];
  modeResults.push(result);
  results.set(key, modeResults);
}
const declaredByTarget = new Map();
const incompleteness = [];
const targetKey = (dialect, packageName, entrypoint, name) =>
  `${dialect}:${packageName}:${entrypoint}:${name}`;
const claimFamily = claim => claim.replace(/=.*/, "");
for (const item of contracts) {
  for (const [entrypoint, entry] of Object.entries(item.contract.entrypoints)) {
    for (const [name, summary] of Object.entries(entry.exports)) {
      declaredByTarget.set(targetKey(item.dialect, item.name, entrypoint, name), summary);
      for (const mode of item.modes.filter(candidate => modeApplies(entry, candidate))) {
        const selected = summaryForMode(summary, mode);
        if (!selected) {
          fail(`${item.file} ${entrypoint}:${name} has no unambiguous summary in ${mode.name}`);
          continue;
        }
        const claims = [
          ...callbackRows(selected).map(
            callback => `callbacks[${callback.parameter}]=${callback.execution}`,
          ),
          ...(returnClaim(selected) ? [returnClaim(selected)] : []),
        ];
        for (const claim of claims) {
          const key = `${item.dialect}:${item.name}:${entrypoint}:${name}:${claim}`;
          const modeResults = results.get(key) ?? [];
          const result = modeResults.find(candidate => candidate.mode === mode.name);
          if (!result) {
            fail(`${item.file} ${entrypoint}:${name} ${claim} has no probe in ${mode.name}`);
          } else if (!result.ok) {
            fail(
              `${item.file} ${entrypoint}:${name} ${claim} failed in ${mode.name}${result.error ? `: ${result.error}` : ""}`,
            );
          } else {
            pass(`${item.name} ${entrypoint}:${name} ${claim} (${mode.name}, ${result.calls} calls)`);
          }
        }
      }
    }
  }
}
for (const observation of observed.discoveredClaims) {
  const stating = contracts.find(
    item => item.dialect === observation.dialect && item.name === observation.pkg,
  );
  if (stating && !stating.modes.some(mode => mode.name === observation.mode)) continue;
  const target = targetKey(
    observation.dialect,
    observation.pkg,
    observation.entrypoint,
    observation.name,
  );
  const summary = declaredByTarget.get(target);
  const mode = probeModes.find(candidate => candidate.name === observation.mode);
  const selected = summary && mode ? summaryForMode(summary, mode) : undefined;
  const declared = selected
    ? [
        ...callbackRows(selected).map(
          callback => `callbacks[${callback.parameter}]=${callback.execution}`,
        ),
        ...(returnClaim(selected) ? [returnClaim(selected)] : []),
    ]
    : [];
  if (declared.includes(observation.claim)) continue;
  // An unknown `callbacks` claims nothing, so an observed callback neither
  // contradicts a negative nor disagrees with a stated row.
  if (callbacksAreUnknown(selected) && observation.claim.startsWith("callbacks[")) continue;
  if (declared.some(claim => claimFamily(claim) === claimFamily(observation.claim))) {
    fail(
      `${target} observed ${observation.claim} in ${observation.mode} but the contract states a different ${claimFamily(observation.claim)} claim`,
    );
  } else {
    const report = `${target} observed ${observation.claim} in ${observation.mode} but the contract has no such claim`;
    incompleteness.push(report);
    fail(`INCOMPLETENESS ${report}`);
  }
}
if (incompleteness.length > 0) {
  console.error(`incompleteness reports: ${incompleteness.length}`);
}

if (failures > 0) process.exit(1);
console.log("bundled contracts conform to their exact package releases");
