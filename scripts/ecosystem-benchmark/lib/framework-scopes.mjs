// Reviewed entrypoint scopes for multi-framework packages in the Solid
// ecosystem benchmark. These are exact package-version policies, not name
// heuristics: every runtime export not listed in `selectedEntrypoints` is the
// excluded set, with the recorded reason, and the full exports object is hash
// bound so discovery stops when a publisher changes any subpath, condition, or
// target.

import { createHash } from "node:crypto";

const FRAMEWORK_SCOPES = new Map([
  [
    "@tanstack/charts",
    {
      version: "0.15.0",
      exportMapSha256: "f30015b841aa5d47d369081a605b0665002cd3b2310572d9cb38fa5e1d41b8c1",
      selectedEntrypoints: ["./solid"],
      excludedEntrypoints: {
        definition: "all-runtime-exports-except-selected",
        reason: "solid-adapter-only-benchmark-scope"
      }
    }
  ],
  [
    "@tanstack/devtools-utils",
    {
      version: "0.7.0",
      exportMapSha256: "478d520c2fc017f2e536acf77ef0614118e759f753184d2421e9cafe69567f6a",
      selectedEntrypoints: ["./solid", "./solid/class"],
      excludedEntrypoints: {
        definition: "all-runtime-exports-except-selected",
        reason: "foreign-framework-adapter"
      }
    }
  ],
  [
    "@tanstack/devtools-a11y",
    {
      version: "0.2.2",
      exportMapSha256: "fc440ad6d19cdba105c0aaa1616c1f2e305c2b9f6299290448f5a458f1475f98",
      selectedEntrypoints: ["./core", "./core/production", "./solid", "./solid/production"],
      excludedEntrypoints: {
        definition: "all-runtime-exports-except-selected",
        reason: "foreign-framework-adapter"
      }
    }
  ]
]);

function canonical(value) {
  if (Array.isArray(value)) return value.map(canonical);
  if (!value || typeof value !== "object") return value;
  return Object.fromEntries(Object.keys(value).sort().map(key => [key, canonical(value[key])]));
}

export function exportMapSha256(exportsMap) {
  return createHash("sha256").update(JSON.stringify(canonical(exportsMap))).digest("hex");
}

export function frameworkScopePolicy(packageName, version) {
  const policy = FRAMEWORK_SCOPES.get(packageName);
  if (!policy) return null;
  if (version !== policy.version) {
    throw new Error(
      `framework scope for ${packageName} is reviewed at ${policy.version}, but discovery selected ${version}`
    );
  }
  return policy;
}

export function scopedEntrypoints(packageName, version) {
  const policy = frameworkScopePolicy(packageName, version);
  return policy ? [...policy.selectedEntrypoints] : null;
}

export function auditFrameworkScope(packageName, version, versionManifest) {
  const policy = frameworkScopePolicy(packageName, version);
  if (!policy) return null;
  if (!versionManifest || typeof versionManifest !== "object") {
    throw new Error(`exact version manifest is unavailable for ${packageName}@${version}`);
  }
  if (versionManifest.name !== packageName || versionManifest.version !== version) {
    throw new Error(
      `exact version manifest identity mismatch for ${packageName}@${version}: got ` +
        `${JSON.stringify(versionManifest.name)}@${JSON.stringify(versionManifest.version)}`
    );
  }

  const exportsMap = versionManifest.exports;
  if (!exportsMap || typeof exportsMap !== "object" || Array.isArray(exportsMap)) {
    throw new Error(`${packageName}@${version} has no auditable object exports map`);
  }
  const actualHash = exportMapSha256(exportsMap);
  if (actualHash !== policy.exportMapSha256) {
    throw new Error(
      `framework scope export map drift for ${packageName}@${version}: expected ` +
        `${policy.exportMapSha256}, got ${actualHash}`
    );
  }

  const runtimeEntrypoints = Object.keys(exportsMap)
    .filter(entrypoint => entrypoint !== "./package.json")
    .sort();
  for (const entrypoint of runtimeEntrypoints) {
    if ((entrypoint !== "." && !entrypoint.startsWith("./")) || entrypoint.includes("*")) {
      throw new Error(`${packageName}@${version} has non-enumerable export subpath ${JSON.stringify(entrypoint)}`);
    }
  }
  const available = new Set(runtimeEntrypoints);
  for (const entrypoint of policy.selectedEntrypoints) {
    if (!available.has(entrypoint)) {
      throw new Error(`${packageName}@${version} no longer exports selected entrypoint ${entrypoint}`);
    }
  }
  const selected = new Set(policy.selectedEntrypoints);
  return {
    selectedEntrypoints: [...policy.selectedEntrypoints],
    excludedEntrypoints: runtimeEntrypoints
      .filter(entrypoint => !selected.has(entrypoint))
      .map(entrypoint => ({ entrypoint, reason: policy.excludedEntrypoints.reason }))
  };
}
