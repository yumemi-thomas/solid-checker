import { readFileSync } from "node:fs";
import { join, relative, resolve, sep } from "node:path";

import { bunLockLocatorForInstalledPackage } from "../../packages/cli/scripts/published-contract-graph.mjs";

const ABSENT = Symbol("absent package-integrity selection");

export class PackageIntegrityError extends Error {
  constructor(code, detail) {
    super(`${code}: ${detail}`);
    this.name = "PackageIntegrityError";
    this.code = code;
  }
}

const parseJsonLike = source => {
  try {
    return JSON.parse(source);
  } catch {}

  // Bun's lockfile is JSON with trailing commas. Remove only commas that
  // occur outside strings and directly precede a closing object/array so
  // package metadata containing punctuation remains untouched.
  let normalized = "";
  let inString = false;
  let escaped = false;
  for (let index = 0; index < source.length; index += 1) {
    const character = source[index];
    if (inString) {
      normalized += character;
      if (escaped) escaped = false;
      else if (character === "\\") escaped = true;
      else if (character === '"') inString = false;
      continue;
    }
    if (character === '"') {
      inString = true;
      normalized += character;
      continue;
    }
    if (character === ",") {
      const remainder = source.slice(index + 1);
      if (/^\s*[}\]]/.test(remainder)) continue;
    }
    normalized += character;
  }
  try {
    return JSON.parse(normalized);
  } catch {
    return null;
  }
};

const readJson = path => {
  try {
    return parseJsonLike(readFileSync(path, "utf8"));
  } catch {
    return null;
  }
};

function readLock(path, invalidCode) {
  let source;
  try {
    source = readFileSync(path, "utf8");
  } catch (error) {
    if (error?.code === "ENOENT") return null;
    throw new PackageIntegrityError("lockfile-read-failed", `${path}: ${error?.message ?? error}`);
  }
  const lock = parseJsonLike(source);
  if (!lock || typeof lock !== "object" || Array.isArray(lock)) {
    throw new PackageIntegrityError(invalidCode, `${path} is not a JSON object`);
  }
  if (
    lock.packages !== undefined &&
    (!lock.packages || typeof lock.packages !== "object" || Array.isArray(lock.packages))
  ) {
    throw new PackageIntegrityError(invalidCode, `${path} has an invalid packages index`);
  }
  return lock;
}

const directPackageRoot = (projectDir, name) =>
  join(projectDir, "node_modules", ...name.split("/"));

const installedVersion = packageRoot =>
  readJson(join(packageRoot, "package.json"))?.version ?? null;

function exactIdentity(name, version) {
  if (typeof name !== "string" || name.length === 0 || typeof version !== "string" || version.length === 0) {
    throw new PackageIntegrityError(
      "invalid-package-identity",
      `expected non-empty package name and version, got ${JSON.stringify([name, version])}`
    );
  }
  return `${name}@${version}`;
}

const slash = path => path.split(sep).join("/");

function packageLockPathForInstalledPackage(projectDir, packageRoot, name) {
  const installedPath = slash(relative(resolve(projectDir), resolve(packageRoot)));
  const suffix = `node_modules/${name}`;
  if (
    !installedPath.startsWith("node_modules/") ||
    (installedPath !== suffix && !installedPath.endsWith(`/${suffix}`))
  ) {
    throw new PackageIntegrityError(
      "installed-root-mismatch",
      `${packageRoot} is not an installed root for ${name} under ${projectDir}`
    );
  }
  return installedPath;
}

function bunLocatorForInstalledPackage(bunLockPath, projectDir, packageRoot, name) {
  // Validate the npm path too: unlike the Bun locator, it retains each
  // node_modules segment and therefore proves that the final installed path
  // actually names the requested package.
  packageLockPathForInstalledPackage(projectDir, packageRoot, name);
  try {
    return bunLockLocatorForInstalledPackage(bunLockPath, packageRoot);
  } catch (error) {
    throw new PackageIntegrityError(
      "installed-root-mismatch",
      error?.message ?? String(error)
    );
  }
}

function packageNameFromLockPath(path) {
  const parts = path.split("/");
  const marker = parts.lastIndexOf("node_modules");
  if (marker < 0 || marker + 1 >= parts.length) return null;
  const tail = parts.slice(marker + 1);
  if (tail[0].startsWith("@")) {
    return tail.length === 2 ? `${tail[0]}/${tail[1]}` : null;
  }
  return tail.length === 1 ? tail[0] : null;
}

function append(map, key, record) {
  const records = map.get(key);
  if (records) records.push(record);
  else map.set(key, [record]);
}

function requireIntegrity(record, manager, identity) {
  if (typeof record.integrity !== "string" || record.integrity.length === 0) {
    throw new PackageIntegrityError(
      "missing-lock-integrity",
      `${manager} lock selection for ${identity} has no integrity`
    );
  }
  return record.integrity;
}

function selectIdentity(records, manager, identity) {
  if (records.length === 0) return ABSENT;
  if (records.length !== 1) {
    throw new PackageIntegrityError(
      "ambiguous-lock-selection",
      `${manager} lock has ${records.length} installed records for ${identity}`
    );
  }
  return requireIntegrity(records[0], manager, identity);
}

/**
 * Reads the integrity recorded for the package installed directly under the
 * project's node_modules tree. This legacy API deliberately keeps resolving
 * that direct installed root rather than selecting by package identity alone.
 */
export function packageIntegrity(projectDir, name) {
  const packageRoot = directPackageRoot(projectDir, name);
  const version = installedVersion(packageRoot);
  if (!version) return null;
  return createPackageIntegrityIndex(projectDir).integrityForInstalledPackage(
    packageRoot,
    name,
    version
  );
}

/**
 * Snapshots Bun and npm lock inputs for one planning transaction.
 *
 * Bun is authoritative when its exact installed locator is present. npm may
 * supply an exact-path fallback only when Bun has no record for that identity;
 * contradictory Bun identity or locator evidence is a refusal, never a reason
 * to accept a stale package-lock record.
 */
export function createPackageIntegrityIndex(projectDir) {
  const bunLockPath = join(projectDir, "bun.lock");
  const bunLock = readLock(bunLockPath, "invalid-bun-lock");
  const packageLock = readLock(join(projectDir, "package-lock.json"), "invalid-package-lock");

  const bunByLocator = new Map();
  const bunByIdentity = new Map();
  for (const [locator, value] of Object.entries(bunLock?.packages ?? {})) {
    const record = Object.freeze({
      locator,
      identity: Array.isArray(value) ? value[0] : null,
      integrity: Array.isArray(value) ? value[3] : null
    });
    bunByLocator.set(locator, record);
    if (typeof record.identity === "string") append(bunByIdentity, record.identity, record);
  }

  const npmByPath = new Map();
  const npmByIdentity = new Map();
  for (const [path, value] of Object.entries(packageLock?.packages ?? {})) {
    if (!path.includes("node_modules/")) continue;
    const record = Object.freeze({
      path,
      name: packageNameFromLockPath(path),
      version: value && typeof value === "object" ? value.version : null,
      integrity: value && typeof value === "object" ? value.integrity : null
    });
    npmByPath.set(path, record);
    if (typeof record.name === "string" && typeof record.version === "string") {
      append(npmByIdentity, JSON.stringify([record.name, record.version]), record);
    }
  }

  const bunIntegrityForInstalledPackage = (packageRoot, name, version) => {
    if (!bunLock) return ABSENT;
    const identity = exactIdentity(name, version);
    const locator = bunLocatorForInstalledPackage(bunLockPath, projectDir, packageRoot, name);
    const installedRecord = bunByLocator.get(locator);
    if (installedRecord && installedRecord.identity !== identity) {
      throw new PackageIntegrityError(
        "lock-identity-mismatch",
        `Bun locator ${locator} identifies ${JSON.stringify(installedRecord.identity)}, not ${identity}`
      );
    }
    const identityRecords = bunByIdentity.get(identity) ?? [];
    // Bun may use the exact package identity as the record locator even when
    // the installed path is the ordinary top-level package name. Preserve the
    // published-graph selector's exact-identity fallback, while still refusing
    // a contradictory record at the actual installed locator above.
    const matchingRecords = [];
    for (const candidate of identityRecords) {
      if (candidate.locator !== locator && candidate.locator !== identity) continue;
      // Match the published selector's refusal precedence: a selected record
      // with no integrity refuses before cardinality is considered.
      requireIntegrity(candidate, "Bun", identity);
      matchingRecords.push(candidate);
    }
    if (matchingRecords.length > 1) {
      throw new PackageIntegrityError(
        "ambiguous-lock-selection",
        `Bun lock has ${matchingRecords.length} exact installed selections for ${identity}`
      );
    }
    if (matchingRecords.length === 1) {
      return matchingRecords[0].integrity;
    }
    if (identityRecords.length > 0) {
      throw new PackageIntegrityError(
        "lock-locator-mismatch",
        `Bun records ${identity} at ${identityRecords.map(item => item.locator).join(", ")}, not ${locator}`
      );
    }
    return ABSENT;
  };

  const npmIntegrityForInstalledPackage = (packageRoot, name, version) => {
    if (!packageLock) return ABSENT;
    const identity = exactIdentity(name, version);
    const path = packageLockPathForInstalledPackage(projectDir, packageRoot, name);
    const record = npmByPath.get(path);
    if (record) {
      if (record.name !== name || record.version !== version) {
        throw new PackageIntegrityError(
          "lock-identity-mismatch",
          `npm path ${path} identifies ${JSON.stringify([record.name, record.version])}, not ${identity}`
        );
      }
      return requireIntegrity(record, "npm", identity);
    }
    const identityRecords = npmByIdentity.get(JSON.stringify([name, version])) ?? [];
    if (identityRecords.length > 0) {
      throw new PackageIntegrityError(
        "lock-locator-mismatch",
        `npm records ${identity} at ${identityRecords.map(item => item.path).join(", ")}, not ${path}`
      );
    }
    return ABSENT;
  };

  return Object.freeze({
    /** Selects integrity for one exact installed root, identity, and version. */
    integrityForInstalledPackage(packageRoot, name, version) {
      exactIdentity(name, version);
      const bunIntegrity = bunIntegrityForInstalledPackage(packageRoot, name, version);
      if (bunIntegrity !== ABSENT) return bunIntegrity;
      const npmIntegrity = npmIntegrityForInstalledPackage(packageRoot, name, version);
      return npmIntegrity === ABSENT ? null : npmIntegrity;
    },

    /**
     * Compatibility lookup without an installed root. It is intentionally
     * conservative: duplicate copies of the same identity are ambiguous.
     */
    integrityForVersion(name, version) {
      const identity = exactIdentity(name, version);
      const bunIntegrity = selectIdentity(bunByIdentity.get(identity) ?? [], "Bun", identity);
      if (bunIntegrity !== ABSENT) return bunIntegrity;
      const npmIntegrity = selectIdentity(
        npmByIdentity.get(JSON.stringify([name, version])) ?? [],
        "npm",
        identity
      );
      return npmIntegrity === ABSENT ? null : npmIntegrity;
    }
  });
}

/** Reads lock integrity for one exact installed package root. */
export function packageIntegrityForInstalledPackage(projectDir, packageRoot, name, version) {
  return createPackageIntegrityIndex(projectDir).integrityForInstalledPackage(
    packageRoot,
    name,
    version
  );
}

/** Compatibility identity-only lookup; duplicate installed copies refuse. */
export function packageIntegrityForVersion(projectDir, name, version) {
  if (!version) return null;
  return createPackageIntegrityIndex(projectDir).integrityForVersion(name, version);
}
