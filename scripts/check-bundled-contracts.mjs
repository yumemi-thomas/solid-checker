#!/usr/bin/env node
import { spawnSync } from "node:child_process";
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

const definitions = loadDialectManifests({ requireArtifacts: true })
  .flatMap(manifest => manifest.contracts)
  .filter(contract => contract.probeRuntime)
  .map(contract => ({ file: contract.bundledContract, name: contract.package }));
const write = process.argv.includes("--write");
const probeModes = [
  { name: "client", conditions: ["browser"] },
  { name: "server", conditions: ["node"] },
  { name: "development", conditions: ["browser", "development"] },
  { name: "production", conditions: ["browser", "production"] },
];
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

function writeProbeEvidence(summary, packageName, entrypoint, name) {
  const claimResults = claim =>
    observed.probes.filter(
      result =>
        result.pkg === packageName &&
        result.entrypoint === entrypoint &&
        result.name === name &&
        result.claim === claim,
    );
  const next = { ...summary };
  const exportResults = [
    ...(summary.callbacks ?? []).map(callback =>
      claimResults(`callbacks[${callback.parameter}]=${callback.execution}`),
    ),
    ...(summary.returns ? [claimResults(`returns=${summary.returns.kind}`)] : []),
  ].flat();
  const evidence = probeEvidence(exportResults);
  if (evidence && (!next.evidence || next.evidence.kind === "inferred")) {
    next.evidence = evidence;
  }
  if (summary.callbacks) {
    next.callbacks = summary.callbacks.map(callback => {
      const callbackEvidence = probeEvidence(
        claimResults(`callbacks[${callback.parameter}]=${callback.execution}`),
      );
      return callbackEvidence && (!callback.evidence || callback.evidence.kind === "inferred")
        ? { ...callback, evidence: callbackEvidence }
        : callback;
    });
  }
  if (summary.returns) {
    const returnEvidence = probeEvidence(claimResults(`returns=${summary.returns.kind}`));
    if (returnEvidence && (!summary.returns.evidence || summary.returns.evidence.kind === "inferred")) {
      next.returns = { ...summary.returns, evidence: returnEvidence };
    }
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

const cacheKey = contracts
  .map(({ name, contract }) => `${name}@${contract.package.version}`)
  .join("_")
  .replace(/[^\w.@-]+/g, "-");
const install = join(tmpdir(), `solid-checker-contract-conformance-${cacheKey}`);
mkdirSync(install, { recursive: true });

const installedVersion = name => {
  const path = join(install, "node_modules", name, "package.json");
  return existsSync(path) ? JSON.parse(readFileSync(path, "utf8")).version : null;
};
if (contracts.some(({ name, contract }) => installedVersion(name) !== contract.package.version)) {
  const result = spawnSync(
    "npm",
    [
      "install",
      "--prefix",
      install,
      "--no-audit",
      "--no-fund",
      "--no-save",
      ...contracts.map(({ name, contract }) => `${name}@${contract.package.version}`),
    ],
    { stdio: "inherit" },
  );
  if (result.status !== 0) process.exit(result.status ?? 1);
}

const worker = join(install, "contract-probes.mjs");
copyFileSync(join(root, "scripts/contract-probes.mjs"), worker);
const packages = contracts.map(({ name }) => ({
  name,
  directory: join(install, "node_modules", name),
}));
const observations = [];
for (const mode of probeModes) {
  const request = { mode: mode.name, packages };
  const execution = spawnSync(
    "node",
    [
      ...mode.conditions.flatMap(condition => ["--conditions", condition]),
      worker,
      JSON.stringify(request),
    ],
    { encoding: "utf8" },
  );
  if (execution.status !== 0) {
    process.stderr.write(execution.stderr);
    process.exit(execution.status ?? 1);
  }
  observations.push(JSON.parse(execution.stdout));
}
const observed = {
  packages: Object.fromEntries(
    contracts.map(({ name }) => {
      const surfaces = observations
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
      return [name, { version: surfaces[0]?.version, entrypoints }];
    }),
  ),
  probes: observations.flatMap(observation => observation.probes),
  discoveredClaims: observations.flatMap(observation => observation.discoveredClaims ?? []),
};
const hiddenLockPath = join(install, "node_modules", ".package-lock.json");
const hiddenLock = existsSync(hiddenLockPath)
  ? JSON.parse(readFileSync(hiddenLockPath, "utf8"))
  : null;
// npm's hidden lockfile keys packages by path, but the path's shape varies:
// a plain `node_modules/<name>` on Linux, and a relative traversal that ends
// with `/node_modules/<name>` where the temp directory resolves through a
// symlink (macOS's /var -> /private/var).
const installedIntegrity = name =>
  Object.entries(hiddenLock?.packages ?? {}).find(
    ([path]) =>
      path === `node_modules/${name}` || path.endsWith(`/node_modules/${name}`),
  )?.[1]?.integrity;

const installedManifest = name => {
  const path = join(install, "node_modules", ...name.split("/"), "package.json");
  return existsSync(path) ? JSON.parse(readFileSync(path, "utf8")) : undefined;
};

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
    const actualVersion = installedVersion(expected.name);
    if (actualVersion !== expected.version) {
      fail(
        `${runtimeLockPath} pins ${identifier}, installed ${expected.name}@${actualVersion ?? "missing"}`,
      );
    }
    const manifest = installedManifest(expected.name);
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
        const resolvedVersion = installedVersion(name);
        const resolvedIntegrity = installedIntegrity(name);
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
    const runtime = observed.packages[item.name];
    item.contract.package.version = runtime.version;
    const integrity = installedIntegrity(item.name);
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
                ? writeProbeEvidence(summary, item.name, entrypoint, name)
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
  const runtime = observed.packages[item.name];
  if (runtime.version !== item.contract.package.version) {
    fail(
      `${item.file} pins ${item.contract.package.version}, installed ${runtime.version}`,
    );
  }
  if (!item.contract.package.integrity) {
    fail(`${item.file} does not pin npm integrity`);
  } else if (item.contract.package.integrity !== installedIntegrity(item.name)) {
    fail(`${item.file} npm integrity does not match the installed release`);
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
  const key = `${result.pkg}:${result.entrypoint}:${result.name}:${result.claim}`;
  const modeResults = results.get(key) ?? [];
  modeResults.push(result);
  results.set(key, modeResults);
}
const claimed = new Set();
const declaredByTarget = new Map();
const incompleteness = [];
const targetKey = (packageName, entrypoint, name) =>
  `${packageName}:${entrypoint}:${name}`;
const claimFamily = claim => claim.replace(/=.*/, "");
for (const item of contracts) {
  for (const [entrypoint, entry] of Object.entries(item.contract.entrypoints)) {
    for (const [name, summary] of Object.entries(entry.exports)) {
      const claims = [
        ...(summary.callbacks ?? []).map(
          callback => `callbacks[${callback.parameter}]=${callback.execution}`,
        ),
        ...(summary.returns ? [`returns=${summary.returns.kind}`] : []),
      ];
      declaredByTarget.set(targetKey(item.name, entrypoint, name), claims);
      for (const claim of claims) {
        const key = `${item.name}:${entrypoint}:${name}:${claim}`;
        claimed.add(key);
        const modeResults = results.get(key) ?? [];
        if (modeResults.length === 0) {
          fail(`${item.file} ${entrypoint}:${name} ${claim} has no probe`);
          continue;
        }
        for (const mode of probeModes.filter(candidate => modeApplies(entry, candidate))) {
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
  const key = `${observation.pkg}:${observation.entrypoint}:${observation.name}:${observation.claim}`;
  if (claimed.has(key)) continue;
  const target = targetKey(observation.pkg, observation.entrypoint, observation.name);
  const declared = declaredByTarget.get(target) ?? [];
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
