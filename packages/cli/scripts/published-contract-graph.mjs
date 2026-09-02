// Untrusted acquisition adapter for policy-2 published dependency graphs.
//
// This module may locate installed packages and transport exact lock/archive
// inputs. It never creates semantic or receipt authority: Rust independently
// replays every node and edge before witness acquisition.

import { readFileSync } from "node:fs";
import { dirname, relative, resolve, sep } from "node:path";

import {
  ArtifactResolutionError,
  findPackageRoot,
  resolvePackageArtifactClosure
} from "./artifact-resolution.mjs";

const MAX_GRAPH_NODES = 256;
const MAX_GRAPH_DEPTH = 64;

export class PublishedGraphAcquisitionRefusal extends Error {
  constructor(kind, detail) {
    super(`${kind}: ${detail}`);
    this.name = "PublishedGraphAcquisitionRefusal";
    this.kind = kind;
  }
}

function parseJsonLike(source) {
  try {
    return JSON.parse(source);
  } catch {}
  let output = "";
  let inString = false;
  let escaped = false;
  for (let index = 0; index < source.length; index += 1) {
    const character = source[index];
    if (inString) {
      output += character;
      if (escaped) escaped = false;
      else if (character === "\\") escaped = true;
      else if (character === '"') inString = false;
      continue;
    }
    if (character === '"') {
      inString = true;
      output += character;
      continue;
    }
    if (character === "," && /^\s*[}\]]/.test(source.slice(index + 1))) continue;
    output += character;
  }
  return JSON.parse(output);
}

const bunLockSelectionRecords = new WeakMap();

/**
 * Parses and indexes untrusted Bun lock bytes once for exact installed-package
 * selection. The index is acquisition material only; native certification
 * still authenticates the transported lock bytes and every graph edge.
 */
export function createBunLockSelectionIndex(lockfile) {
  const recordsByIdentity = new Map();
  for (const [locator, record] of Object.entries(parseJsonLike(lockfile).packages ?? {})) {
    if (!Array.isArray(record) || typeof record[0] !== "string") continue;
    const indexed = Object.freeze({ locator, integrity: record[3] });
    for (const identity of new Set([locator, record[0]])) {
      const records = recordsByIdentity.get(identity);
      if (records) records.push(indexed);
      else recordsByIdentity.set(identity, [indexed]);
    }
  }
  const index = Object.freeze({});
  bunLockSelectionRecords.set(index, recordsByIdentity);
  return index;
}

function bunLockSelectionIndex(lockfileOrIndex) {
  if (bunLockSelectionRecords.has(lockfileOrIndex)) return lockfileOrIndex;
  if (typeof lockfileOrIndex === "string") {
    return createBunLockSelectionIndex(lockfileOrIndex);
  }
  throw new TypeError("exact Bun selection requires lockfile bytes or a Bun lock index");
}

export function bunLockLocatorForInstalledPackage(bunLockPath, packageRoot) {
  const installed = relative(dirname(resolve(bunLockPath)), resolve(packageRoot));
  const parts = installed.split(sep);
  if (parts[0] === ".." || parts[0] !== "node_modules") {
    throw new PublishedGraphAcquisitionRefusal(
      "installed-lock-layout",
      `${packageRoot} is not under the lockfile's node_modules tree`
    );
  }
  const locator = parts.slice(1).filter(part => part !== "node_modules").join("/");
  if (!locator) {
    throw new PublishedGraphAcquisitionRefusal(
      "installed-lock-layout",
      `${packageRoot} has no exact Bun locator`
    );
  }
  return locator;
}

export function exactBunLockSelection(
  lockfileOrIndex,
  packageName,
  packageVersion,
  installedLocator = null
) {
  const exact = `${packageName}@${packageVersion}`;
  const matches = [];
  const index = bunLockSelectionIndex(lockfileOrIndex);
  for (const record of bunLockSelectionRecords.get(index).get(exact) ?? []) {
    const { locator } = record;
    if (installedLocator && locator !== installedLocator && locator !== exact) continue;
    if (typeof record.integrity !== "string") {
      throw new PublishedGraphAcquisitionRefusal(
        "missing-lock-integrity",
        `${exact} has no exact Bun integrity`
      );
    }
    matches.push({ locator, integrity: record.integrity });
  }
  if (matches.length !== 1) {
    throw new PublishedGraphAcquisitionRefusal(
      matches.length === 0 ? "missing-lock-selection" : "ambiguous-lock-selection",
      `${exact} has ${matches.length} exact Bun selections`
    );
  }
  return Object.freeze(matches[0]);
}

function packageNameFromSpecifier(specifier) {
  if (!specifier || specifier.startsWith(".") || specifier.startsWith("/") || specifier.startsWith("#")) {
    return null;
  }
  const parts = specifier.split("/");
  return specifier.startsWith("@") ? parts.slice(0, 2).join("/") : parts[0];
}

function externalDependency(hazard) {
  if (hazard?.kind !== "unaccepted-external-dependency") return null;
  const separator = hazard.source.indexOf(":");
  if (separator < 0) return null;
  return {
    source: hazard.source.slice(0, separator),
    specifier: hazard.source.slice(separator + 1)
  };
}

export function publishedGraphRequestKey({
  importer,
  specifier,
  packageRoot,
  conditions,
  integrity
}) {
  return JSON.stringify([
    resolve(importer),
    specifier,
    resolve(packageRoot),
    [...new Set([...conditions, "import"])].sort(),
    integrity
  ]);
}

/**
 * Discovers a finite installed graph for one exact root artifact case. The
 * result is dependency-first and order-canonical, but remains untrusted input
 * to native certification.
 */
export function discoverInstalledPublishedGraph(
  { root, bunLockPath, maxNodes = MAX_GRAPH_NODES, maxDepth = MAX_GRAPH_DEPTH },
  {
    resolveClosure = resolvePackageArtifactClosure,
    locatePackage = findPackageRoot,
    readManifest = packageRoot =>
      JSON.parse(readFileSync(resolve(packageRoot, "package.json"), "utf8")),
    readLock = path => readFileSync(path, "utf8")
  } = {}
) {
  const lockfile = readLock(bunLockPath);
  const lockIndex = createBunLockSelectionIndex(lockfile);
  const planned = new Map();
  const active = [];

  const visit = (request, depth) => {
    if (depth > maxDepth) {
      throw new PublishedGraphAcquisitionRefusal(
        "depth-limit",
        `${request.specifier} exceeds graph depth ${maxDepth}`
      );
    }
    const conditions = [...new Set([...(request.conditions ?? []), "import"])].sort();
    const key = publishedGraphRequestKey({ ...request, conditions });
    const cycleAt = active.indexOf(key);
    if (cycleAt >= 0) {
      throw new PublishedGraphAcquisitionRefusal(
        "cycle",
        [...active.slice(cycleAt), key].join(" -> ")
      );
    }
    if (planned.has(key)) return key;
    if (planned.size >= maxNodes) {
      throw new PublishedGraphAcquisitionRefusal(
        "node-limit",
        `published graph exceeds ${maxNodes} nodes`
      );
    }

    const resolved = resolveClosure({
      importer: request.importer,
      specifier: request.specifier,
      packageRoot: request.packageRoot,
      conditions,
      resolutionKind: "import",
      integrity: request.integrity
    });
    const manifest = readManifest(resolved.packageRoot);
    if (manifest.name !== resolved.packageName || manifest.version !== resolved.packageVersion) {
      throw new PublishedGraphAcquisitionRefusal(
        "installed-identity",
        `${resolved.packageRoot} disagrees with its resolved package identity`
      );
    }
    const lockSelection = exactBunLockSelection(
      lockIndex,
      resolved.packageName,
      resolved.packageVersion,
      bunLockLocatorForInstalledPackage(bunLockPath, resolved.packageRoot)
    );
    if (lockSelection.integrity !== request.integrity) {
      throw new PublishedGraphAcquisitionRefusal(
        "lock-integrity-disagreement",
        `${resolved.packageName}@${resolved.packageVersion} disagrees with acquired integrity`
      );
    }

    active.push(key);
    const dependencies = [];
    for (const hazard of resolved.closure.hazards ?? []) {
      const dependency = externalDependency(hazard);
      if (!dependency) {
        throw new PublishedGraphAcquisitionRefusal(
          "unsupported-closure-hazard",
          `${hazard.kind} at ${hazard.source}`
        );
      }
      const dependencyName = packageNameFromSpecifier(dependency.specifier);
      if (!dependencyName || dependency.specifier.startsWith("node:")) {
        throw new PublishedGraphAcquisitionRefusal(
          "unsupported-external-specifier",
          dependency.specifier
        );
      }
      const dependencyImporter = resolve(resolved.packageRoot, dependency.source);
      const dependencyRoot = locatePackage(dependencyImporter, dependencyName);
      const dependencyManifest = readManifest(dependencyRoot);
      const dependencyLock = exactBunLockSelection(
        lockIndex,
        dependencyManifest.name,
        dependencyManifest.version,
        bunLockLocatorForInstalledPackage(bunLockPath, dependencyRoot)
      );
      let child;
      try {
        child = visit(
          {
            importer: dependencyImporter,
            specifier: dependency.specifier,
            packageRoot: dependencyRoot,
            conditions,
            integrity: dependencyLock.integrity
          },
          depth + 1
        );
      } catch (error) {
        if (
          error instanceof ArtifactResolutionError &&
          ["target-not-found", "declarations-not-found"].includes(error.code) &&
          typeof (dependencyManifest.types ?? dependencyManifest.typings) === "string" &&
          !dependencyManifest.main &&
          !dependencyManifest.module
        ) {
          throw new PublishedGraphAcquisitionRefusal(
            "type-only-declaration-dependency",
            `${dependencyManifest.name}@${dependencyManifest.version} has authenticated declaration bytes but no runtime artifact; policy 2 has no declaration-only graph witness`
          );
        }
        throw error;
      }
      dependencies.push({ specifier: dependency.specifier, node: child });
    }
    active.pop();
    dependencies.sort((left, right) =>
      left.specifier.localeCompare(right.specifier) || left.node.localeCompare(right.node)
    );
    planned.set(key, {
      key,
      importer: resolve(request.importer),
      specifier: request.specifier,
      packageRoot: resolve(resolved.packageRoot),
      packageName: resolved.packageName,
      packageVersion: resolved.packageVersion,
      integrity: request.integrity,
      entrypoint: resolved.requestedEntrypoint,
      conditions,
      lockLocator: lockSelection.locator,
      bunLockPath: resolve(bunLockPath),
      dependencies
    });
    return key;
  };

  const rootKey = visit(root, 0);
  const emitted = new Set();
  const dependencyFirst = [];
  const emit = key => {
    if (emitted.has(key)) return;
    const node = planned.get(key);
    for (const dependency of node.dependencies) emit(dependency.node);
    emitted.add(key);
    dependencyFirst.push(node);
  };
  emit(rootKey);
  return Object.freeze({
    schemaVersion: 1,
    root: rootKey,
    nodes: Object.freeze(dependencyFirst)
  });
}
