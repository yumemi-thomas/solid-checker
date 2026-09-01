import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import {
  ArtifactResolutionError,
  closureEntriesAreCurrent,
  findPackageRoot,
  locateExternalDependencyPackageRoot,
  resolvePackageDependencyPlanClosure
} from "../../../packages/cli/scripts/artifact-resolution.mjs";
import {
  createPackageIntegrityIndex,
  PackageIntegrityError,
  packageIntegrityForVersion
} from "../../lib/package-integrity.mjs";
import { splitPackageSpecifier } from "./external-edges.mjs";

const hash = value =>
  `sha256:${createHash("sha256").update(JSON.stringify(value)).digest("hex")}`;

function rootSpecifier(packageName, entrypoint) {
  return entrypoint && entrypoint !== "."
    ? `${packageName}/${entrypoint.replace(/^\.\//, "")}`
    : packageName;
}

function externalHazard(hazard) {
  if (hazard?.kind !== "unaccepted-external-dependency") return null;
  if (typeof hazard.importerPath !== "string" || typeof hazard.specifier !== "string") {
    return null;
  }
  return {
    importerPath: hazard.importerPath,
    specifier: hazard.specifier,
    optionalPeer: hazard.optionalPeer === true,
    dynamicImport: hazard.dynamicImport === true
  };
}

function stableError(error, projectDir) {
  const reason = String(error?.message ?? error).split(resolve(projectDir)).join("<project-root>");
  return {
    code:
      error instanceof ArtifactResolutionError || error instanceof PackageIntegrityError
        ? error.code
        : "planner-error",
    reason
  };
}

function readIdentity(packageRoot) {
  const manifest = JSON.parse(readFileSync(resolve(packageRoot, "package.json"), "utf8"));
  return { package: manifest.name, version: manifest.version };
}

/**
 * Builds the complete exact installed dependency graph for refused artifact
 * cases. This is planning evidence only: terminal graph leaves explicitly
 * remain unauthenticated and cannot be used as accepted dependency contracts.
 */
export function planRecursiveDependencies({
  projectDir,
  rootPackageRoot,
  rootPackage,
  rootVersion,
  rootIntegrity,
  artifactCases,
  maxDepth = 32,
  maxNodes = 512,
  resolveClosure = resolvePackageDependencyPlanClosure,
  locatePackage = findPackageRoot,
  pathExists,
  integrityForVersion = packageIntegrityForVersion
}) {
  const nodes = new Map();
  const edges = [];
  const leaves = new Map();
  const conditionalDependencies = new Map();
  const cycles = [];
  const resolvedClosures = new Map();
  const integrities = new Map();
  const lockIndex = integrityForVersion === packageIntegrityForVersion
    ? createPackageIntegrityIndex(projectDir)
    : null;
  let budgetExceeded = false;

  const exactIntegrity = (packageRoot, name, version) => {
    const key = JSON.stringify([resolve(packageRoot), name, version]);
    if (!integrities.has(key)) {
      integrities.set(
        key,
        lockIndex
          ? lockIndex.integrityForInstalledPackage(packageRoot, name, version)
          : integrityForVersion(projectDir, name, version)
      );
    }
    return integrities.get(key);
  };
  const exactClosure = ({ importer, specifier, packageRoot, integrity, conditions }) => {
    // This is untrusted planning evidence, not receipt authority. Reuse is
    // nevertheless restricted to the complete resolution input so unequal
    // conditions, roots, archives, resolution modes, or source programs never
    // merge. `resolvePackageDependencyPlanClosure` uses `importer` only to
    // locate a package root when none is supplied; every visit here supplies
    // the exact installed root, so a different parent importer cannot change
    // this acquisition.
    const key = JSON.stringify([
      specifier,
      resolve(packageRoot),
      integrity,
      conditions,
      "import"
    ]);
    const cached = resolvedClosures.get(key);
    if (
      cached &&
      closureEntriesAreCurrent(cached.closure.entries, cached.packageRoot)
    ) {
      return cached;
    }
    resolvedClosures.delete(key);
    // A failed acquisition has no complete observed source closure to
    // revalidate. Retry it rather than allowing a transient or source-bound
    // refusal to become transaction-wide evidence.
    const resolved = resolveClosure({
      importer,
      specifier,
      packageRoot,
      conditions,
      integrity
    });
    resolvedClosures.set(key, resolved);
    return resolved;
  };

  const addLeaf = leaf => {
    const id = hash(leaf);
    leaves.set(id, { id, ...leaf });
  };
  const addConditionalDependency = dependency => {
    const id = hash(dependency);
    conditionalDependencies.set(id, { id, ...dependency });
  };
  const visit = ({ importer, specifier, packageRoot, integrity, conditions, depth, ancestry }) => {
    if (depth > maxDepth) {
      budgetExceeded = true;
      addLeaf({ kind: "depth-budget", specifier, conditions, limit: maxDepth });
      return null;
    }
    let resolved;
    try {
      resolved = exactClosure({
        importer,
        specifier,
        packageRoot,
        conditions,
        integrity
      });
    } catch (error) {
      addLeaf({
        kind: "artifact-resolution",
        specifier,
        conditions,
        ...stableError(error, projectDir)
      });
      return null;
    }

    const identity = {
      package: resolved.packageName,
      version: resolved.packageVersion,
      integrity: resolved.packageIntegrity,
      entrypoint: resolved.requestedEntrypoint,
      conditions: [...conditions],
      runtime: { path: resolved.runtime.path, digest: resolved.runtime.digest },
      declarations: { path: resolved.declarations.path, digest: resolved.declarations.digest },
      closureDigest: resolved.closure.digest
    };
    const nodeId = hash(identity);
    if (ancestry.includes(nodeId)) {
      const cycle = [...ancestry.slice(ancestry.indexOf(nodeId)), nodeId];
      if (!cycles.some(value => JSON.stringify(value.nodes) === JSON.stringify(cycle))) {
        cycles.push({ id: hash(cycle), nodes: cycle });
      }
      return nodeId;
    }
    if (nodes.has(nodeId)) return nodeId;
    if (nodes.size >= maxNodes) {
      budgetExceeded = true;
      addLeaf({ kind: "node-budget", specifier, conditions, limit: maxNodes });
      return null;
    }

    const node = { id: nodeId, ...identity };
    nodes.set(nodeId, node);
    for (const frontier of resolved.closure.frontiers ?? []) {
      addLeaf({
        kind: "module-loading-frontier",
        node: nodeId,
        frontier: frontier.kind,
        source: frontier.source,
        specifier: frontier.specifier ?? null
      });
    }
    const dependencyHazards = [];
    for (const hazard of resolved.closure.hazards) {
      const dependency = externalHazard(hazard);
      if (dependency) dependencyHazards.push(dependency);
      else {
        addLeaf({
          kind: "closure-hazard",
          node: nodeId,
          hazard: hazard.kind,
          source: hazard.source
        });
      }
    }

    if (dependencyHazards.length === 0) {
      addLeaf({
        kind: "authenticated-receipt-unavailable",
        node: nodeId,
        package: node.package,
        version: node.version,
        integrity: node.integrity,
        entrypoint: node.entrypoint,
        conditions: node.conditions
      });
    }

    for (const dependency of dependencyHazards) {
      const split = splitPackageSpecifier(dependency.specifier);
      if (!split || dependency.specifier.startsWith("node:")) {
        addLeaf({
          kind: "unsupported-external-specifier",
          node: nodeId,
          specifier: dependency.specifier
        });
        edges.push({ from: nodeId, specifier: dependency.specifier, to: null });
        continue;
      }
      const dependencyImporter = resolve(resolved.packageRoot, dependency.importerPath);
      let dependencyRoot;
      let dependencyIdentity;
      let dependencyIntegrity;
      try {
        // Package layout and manifests are live transaction inputs. They are
        // cheap relative to closure parsing, so reread both for every edge
        // rather than letting a changed nearest install or identity inherit a
        // prior edge's planning evidence.
        dependencyRoot = locateExternalDependencyPackageRoot(
          dependencyImporter,
          {
            kind: dependency.dynamicImport ? "dynamic" : "import",
            ...dependency
          },
          {
            locatePackage,
            ...(pathExists ? { pathExists } : {})
          }
        );
        if (!dependencyRoot) {
          addConditionalDependency({
            kind: "absent-optional-peer",
            node: nodeId,
            specifier: dependency.specifier
          });
          edges.push({
            from: nodeId,
            specifier: dependency.specifier,
            to: null,
            conditional: "absent-optional-peer"
          });
          continue;
        }
        dependencyIdentity = readIdentity(dependencyRoot);
        dependencyIntegrity = exactIntegrity(
          dependencyRoot,
          dependencyIdentity.package,
          dependencyIdentity.version
        );
        if (!dependencyIntegrity) {
          throw new ArtifactResolutionError(
            "integrity-not-found",
            `lockfile has no integrity for ${dependencyIdentity.package}@${dependencyIdentity.version}`
          );
        }
      } catch (error) {
        addLeaf({
          kind: "dependency-identity",
          node: nodeId,
          specifier: dependency.specifier,
          ...stableError(error, projectDir)
        });
        edges.push({ from: nodeId, specifier: dependency.specifier, to: null });
        continue;
      }
      const child = visit({
        importer: dependencyImporter,
        specifier: dependency.specifier,
        packageRoot: dependencyRoot,
        integrity: dependencyIntegrity,
        conditions,
        depth: depth + 1,
        ancestry: [...ancestry, nodeId]
      });
      edges.push({ from: nodeId, specifier: dependency.specifier, to: child });
    }
    return nodeId;
  };

  const roots = [];
  for (const artifactCase of artifactCases) {
    const conditions = Array.isArray(artifactCase.conditions) ? artifactCase.conditions : [];
    const specifier = rootSpecifier(rootPackage, artifactCase.entrypoint);
    const root = visit({
      importer: resolve(projectDir, "package.json"),
      specifier,
      packageRoot: rootPackageRoot,
      integrity: rootIntegrity,
      conditions,
      depth: 0,
      ancestry: []
    });
    roots.push({
      entrypoint: artifactCase.entrypoint,
      conditions,
      node: root
    });
  }

  const sortedNodes = [...nodes.values()].sort((left, right) => left.id.localeCompare(right.id));
  edges.sort((left, right) =>
    left.from.localeCompare(right.from) ||
    left.specifier.localeCompare(right.specifier) ||
    String(left.to).localeCompare(String(right.to))
  );
  const sortedLeaves = [...leaves.values()].sort((left, right) => left.id.localeCompare(right.id));
  const sortedConditionalDependencies = [...conditionalDependencies.values()]
    .sort((left, right) => left.id.localeCompare(right.id));
  cycles.sort((left, right) => left.id.localeCompare(right.id));
  return {
    schemaVersion: 1,
    rootIdentity: { package: rootPackage, version: rootVersion, integrity: rootIntegrity },
    status: budgetExceeded
      ? "resource-refusal"
      : sortedLeaves.length > 0
        ? "exact-leaf-refusal"
        : cycles.length > 0
          ? "cycle-refusal"
          : sortedConditionalDependencies.length > 0
            ? "conditional-only"
            : "complete",
    complete: !budgetExceeded,
    limits: { maxDepth, maxNodes },
    roots,
    nodes: sortedNodes,
    edges,
    cycles,
    leaves: sortedLeaves,
    conditionalDependencies: sortedConditionalDependencies,
    graphDigest: hash({
      roots,
      nodes: sortedNodes,
      edges,
      cycles,
      leaves: sortedLeaves,
      ...(sortedConditionalDependencies.length > 0
        ? { conditionalDependencies: sortedConditionalDependencies }
        : {})
    })
  };
}
