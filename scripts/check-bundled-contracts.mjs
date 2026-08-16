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

function modeApplies(entrypoint, mode) {
  const conditions = new Set(entrypoint.conditions ?? []);
  const environment = new Set(["browser", "node", "client", "server", "development", "production"]);
  const selected = [...environment].filter(condition => conditions.has(condition));
  if (selected.length === 0) return true;
  if (conditions.has("development")) return mode.name === "development";
  if (conditions.has("production")) return mode.name === "production";
  if (conditions.has("server") || conditions.has("node")) return mode.name === "server";
  if (conditions.has("client") || conditions.has("browser")) {
    return ["client", "development", "production"].includes(mode.name);
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
      for (const surface of surfaces) {
        for (const [entrypoint, value] of Object.entries(surface.entrypoints)) {
          const current = entrypoints[entrypoint] ?? {
            exports: {},
            conditions: [],
          };
          Object.assign(current.exports, value.exports);
          current.conditions = [...new Set([...current.conditions, ...value.conditions])].sort();
          entrypoints[entrypoint] = current;
        }
      }
      return [name, { version: surfaces[0]?.version, entrypoints }];
    }),
  ),
  probes: observations.flatMap(observation => observation.probes),
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

if (write) {
  for (const item of contracts) {
    const runtime = observed.packages[item.name];
    const previous = item.contract.entrypoints ?? {};
    item.contract.package.version = runtime.version;
    const integrity = installedIntegrity(item.name);
    if (integrity) item.contract.package.integrity = integrity;
    item.contract.entrypoints = Object.fromEntries(
      Object.entries(runtime.entrypoints)
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([entrypoint, surface]) => {
          const oldExports = previous[entrypoint]?.exports ?? {};
          const exports = Object.fromEntries(
            Object.entries(surface.exports)
              .sort(([left], [right]) => left.localeCompare(right))
              .map(([name, kind]) => {
                const old = oldExports[name];
                const summary = old?.kind === kind ? old : { kind };
                return [name, writeProbeEvidence(summary, item.name, entrypoint, name)];
              }),
          );
          return [entrypoint, { exports, conditions: surface.conditions }];
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
for (const item of contracts) {
  for (const [entrypoint, entry] of Object.entries(item.contract.entrypoints)) {
    for (const [name, summary] of Object.entries(entry.exports)) {
      const claims = [
        ...(summary.callbacks ?? []).map(
          callback => `callbacks[${callback.parameter}]=${callback.execution}`,
        ),
        ...(summary.returns ? [`returns=${summary.returns.kind}`] : []),
      ];
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
for (const result of observed.probes) {
  const key = `${result.pkg}:${result.entrypoint}:${result.name}:${result.claim}`;
  if (!claimed.has(key)) fail(`probe has no matching contract claim: ${key}`);
}

if (failures > 0) process.exit(1);
console.log("bundled contracts conform to their exact package releases");
