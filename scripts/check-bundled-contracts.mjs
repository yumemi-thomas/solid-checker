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
let failures = 0;
const fail = message => {
  failures++;
  console.error(`FAIL ${message}`);
};
const pass = message => console.log(`ok   ${message}`);

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
const request = {
  packages: contracts.map(({ name }) => ({
    name,
    directory: join(install, "node_modules", name),
  })),
};
const execution = spawnSync(
  "node",
  ["--conditions=browser", worker, JSON.stringify(request)],
  { encoding: "utf8" },
);
if (execution.status !== 0) {
  process.stderr.write(execution.stderr);
  process.exit(execution.status ?? 1);
}
const observed = JSON.parse(execution.stdout);
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
                return [name, old?.kind === kind ? old : { kind }];
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

const results = new Map(
  observed.probes.map(result => [
    `${result.pkg}:${result.entrypoint}:${result.name}:${result.claim}`,
    result,
  ]),
);
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
        const result = results.get(key);
        if (!result) fail(`${item.file} ${entrypoint}:${name} ${claim} has no probe`);
        else if (!result.ok) {
          fail(
            `${item.file} ${entrypoint}:${name} ${claim} failed${result.error ? `: ${result.error}` : ""}`,
          );
        } else pass(`${item.name} ${entrypoint}:${name} ${claim}`);
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
