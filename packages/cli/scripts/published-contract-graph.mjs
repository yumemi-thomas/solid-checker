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
  nodeBuiltinSpecifier,
  resolvePackageArtifactClosure
} from "./artifact-resolution.mjs";

// Matches POLICY_2_GRAPH_NODE_LIMIT in the native planner: a node is one
// (artifact, importing module) pair, and importer variants share their work.
const MAX_GRAPH_NODES = 1024;
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

// The lockfiles this adapter can read, by the exact file name a package manager
// writes. The name is the whole format decision on both sides: Rust receives the
// lockfile *path* and dispatches on the same table, so no format tag travels the
// wire and no reader guesses from content. A file named anything else is refused
// rather than sniffed.
export const SUPPORTED_LOCKFILES = Object.freeze([
  Object.freeze({ fileName: "bun.lock", packageManager: "bun" }),
  Object.freeze({ fileName: "pnpm-lock.yaml", packageManager: "pnpm" }),
  Object.freeze({ fileName: "package-lock.json", packageManager: "npm" })
]);

// Lockfiles a package manager writes that this adapter deliberately does not
// read. They are named only so a refusal can say what it found: a tree installed
// by one of these has no exact lock selection here, and certification states no
// dependency environment rather than an empty one.
export const UNSUPPORTED_LOCKFILES = Object.freeze([
  "npm-shrinkwrap.json",
  "yarn.lock",
  "bun.lockb"
]);

// Only npm `packages`-map lockfiles are read: `lockfileVersion` 2 and 3. Version
// 1 has only the name-keyed `dependencies` tree, which cannot say *which*
// installed copy an entry describes under hoisting.
const NPM_LOCKFILE_VERSIONS = new Set([2, 3]);

// The `bun.lock` `lockfileVersion`s read, exactly admission's set
// (`BUN_LOCKFILE_VERSIONS` in the Rust authority): the two whose npm package
// records were checked against Bun's parser. Certifying from any other version
// would issue a receipt that admission then refuses.
const BUN_LOCKFILE_VERSIONS = new Set([1, 2]);

/** Refuses a Bun lockfile whose version is outside `BUN_LOCKFILE_VERSIONS`,
 * with the Rust twin's reason wording. */
function requireBunLockfileVersion(document) {
  const version = document?.lockfileVersion;
  if (version === undefined) {
    throw new PublishedGraphAcquisitionRefusal(
      "unsupported-lock-version",
      "Bun lockfile does not declare a lockfileVersion"
    );
  }
  if (!BUN_LOCKFILE_VERSIONS.has(version)) {
    throw new PublishedGraphAcquisitionRefusal(
      "unsupported-lock-version",
      `Bun lockfileVersion ${JSON.stringify(version)} is not 1 or 2; only those versions' ` +
        "package records are read"
    );
  }
}

// Only pnpm lockfile major 9 is read, and the restriction is load-bearing rather
// than conservative packaging. The selection argument below rests on `packages:`
// keys being exactly `name@version` and unique; major 6 wrote peer suffixes into
// those same keys (`foo@1.0.0(bar@2.0.0)`), so one `name@version` could appear
// under several keys and the uniqueness this reader relies on would not hold.
const PNPM_LOCKFILE_MAJOR = 9;

const PNPM_INTEGRITY = /^(?:sha512|sha384|sha256|sha1)-[A-Za-z0-9+/]+={0,2}$/;

/** Rejects the YAML features this reader does not implement, rather than
 * ignoring them. An anchor, alias or merge key can move a value from one entry
 * to another; a reader that skipped what it did not understand would answer
 * from the wrong bytes. Document markers, which can redefine `packages:`
 * wholesale, are `pnpmDocuments`'s to judge. */
function refusePnpmYamlBeyondSubset(source) {
  if (source.includes("\t")) {
    throw new PublishedGraphAcquisitionRefusal(
      "unsupported-lock-syntax",
      "pnpm lockfile contains a tab, which YAML does not permit for indentation"
    );
  }
  for (const [index, line] of source.split("\n").entries()) {
    const at = `line ${index + 1}`;
    // No trailing-character requirement: a bare `*` is an alias too, and the
    // Rust twin refuses one. A subset either side reads and the other refuses
    // turns an unsound lockfile into a confusing late refusal instead of an
    // early, accurate one.
    if (/(?:^|[\s[{,])[&*]/.test(line.replace(/'[^']*'|"(?:[^"\\]|\\.)*"/g, ""))) {
      throw new PublishedGraphAcquisitionRefusal(
        "unsupported-lock-syntax",
        `pnpm lockfile uses a YAML anchor or alias at ${at}`
      );
    }
    if (/^\s*<<\s*:/.test(line)) {
      throw new PublishedGraphAcquisitionRefusal(
        "unsupported-lock-syntax",
        `pnpm lockfile uses a YAML merge key at ${at}`
      );
    }
  }
}

/** Reads one YAML scalar used as a mapping key: plain, single- or
 * double-quoted. pnpm quotes any key containing `@`, and a formatter may have
 * rewritten single quotes to double, so both styles are ordinary input. */
function pnpmScalar(text) {
  const trimmed = text.trim();
  if (trimmed.startsWith("'")) {
    if (!trimmed.endsWith("'") || trimmed.length < 2) return null;
    return trimmed.slice(1, -1).replace(/''/g, "'");
  }
  if (trimmed.startsWith('"')) {
    if (!trimmed.endsWith('"') || trimmed.length < 2) return null;
    try {
      return JSON.parse(trimmed);
    } catch {
      return null;
    }
  }
  return trimmed.length ? trimmed : null;
}

/** Parses one flat YAML flow mapping (`{ a: b, c: d }`, trailing comma allowed)
 * into entries. Nested flow collections are refused rather than flattened: the
 * only mapping this reader consumes is `resolution`, whose values pnpm writes as
 * scalars. */
function pnpmFlowMapping(text, at) {
  const body = text.trim();
  if (!body.startsWith("{") || !body.endsWith("}")) {
    throw new PublishedGraphAcquisitionRefusal(
      "unsupported-lock-syntax",
      `pnpm lockfile has a non-flow ${at}`
    );
  }
  const entries = new Map();
  const inner = body.slice(1, -1);
  let depth = 0;
  let quote = null;
  let current = "";
  const parts = [];
  for (let index = 0; index < inner.length; index += 1) {
    const character = inner[index];
    if (quote) {
      current += character;
      if (character === "\\" && quote === '"') {
        current += inner[++index] ?? "";
      } else if (character === quote) quote = null;
      continue;
    }
    if (character === "'" || character === '"') {
      quote = character;
      current += character;
      continue;
    }
    if (character === "{" || character === "[") depth += 1;
    if (character === "}" || character === "]") depth -= 1;
    if (depth < 0) {
      throw new PublishedGraphAcquisitionRefusal(
        "unsupported-lock-syntax",
        `pnpm lockfile has an unbalanced ${at}`
      );
    }
    if (character === "," && depth === 0) {
      parts.push(current);
      current = "";
      continue;
    }
    current += character;
  }
  if (quote || depth !== 0) {
    throw new PublishedGraphAcquisitionRefusal(
      "unsupported-lock-syntax",
      `pnpm lockfile has an unterminated ${at}`
    );
  }
  parts.push(current);
  for (const part of parts) {
    if (!part.trim()) continue;
    if (/[{[]/.test(part)) {
      throw new PublishedGraphAcquisitionRefusal(
        "unsupported-lock-syntax",
        `pnpm lockfile nests a collection inside ${at}`
      );
    }
    const separator = part.indexOf(":");
    if (separator < 0) {
      throw new PublishedGraphAcquisitionRefusal(
        "unsupported-lock-syntax",
        `pnpm lockfile has a non-mapping item in ${at}`
      );
    }
    const key = pnpmScalar(part.slice(0, separator));
    const value = pnpmScalar(part.slice(separator + 1));
    if (key === null) {
      throw new PublishedGraphAcquisitionRefusal(
        "unsupported-lock-syntax",
        `pnpm lockfile has an unreadable key in ${at}`
      );
    }
    if (entries.has(key)) {
      throw new PublishedGraphAcquisitionRefusal(
        "ambiguous-lock-selection",
        `pnpm lockfile repeats ${key} in ${at}`
      );
    }
    entries.set(key, value);
  }
  return entries;
}

const PNPM_ENV_TOP_LEVEL_KEYS = new Set(["lockfileVersion", "importers", "packages", "snapshots"]);
const PNPM_ENV_IMPORTER_KEYS = new Set(["configDependencies", "packageManagerDependencies"]);

/** A line that opens (`---`) or closes (`...`) a YAML document. */
const isPnpmDocumentMarker = line => /^(?:---|\.\.\.)(?: |$)/.test(line);

/** The key of one block-mapping line after its indentation, or null. */
function pnpmMappingKey(rest) {
  const line = rest.trimEnd();
  const match = /^('[^']*'|"(?:[^"\\]|\\.)*"|[^'"].*?)(?::(?: .*)?)$/.exec(line);
  return match ? pnpmScalar(match[1]) : null;
}

/**
 * Splits a pnpm lockfile into its env and project documents; the twin of
 * `pnpm_documents` in Rust.
 *
 * pnpm 11+ leads `pnpm-lock.yaml` with an env document (the lockfile of the
 * project's `configDependencies` and `packageManagerDependencies`, installed
 * outside its `node_modules`) in exactly one shape, `---\n<env>\n---\n<main>`
 * (pnpm's `lockfile/fs/src/yamlDocuments.ts`), and reads the project lockfile as
 * everything after the first separator. Every other arrangement of document
 * markers is refused, and so is an env document that is not pnpm's
 * `EnvLockfile` -- one that names a project importer would make the document
 * that bears the installed integrity a guess.
 */
function pnpmDocuments(text) {
  const lines = text.split("\n");
  const markers = lines.flatMap((line, index) => (isPnpmDocumentMarker(line) ? [index] : []));
  const unexpected = markers.findIndex(
    (index, position) => lines[index] !== "---" || position > 1 || (position === 0 && index !== 0)
  );
  if (unexpected >= 0) {
    const index = markers[unexpected];
    throw new PublishedGraphAcquisitionRefusal(
      "unsupported-lock-syntax",
      `pnpm lockfile has a document marker ${JSON.stringify(lines[index])} at line ${index + 1}; ` +
        "only a single document, or pnpm's env document followed by one project document, is read"
    );
  }
  if (markers.length === 0) return { env: null, main: text };
  const separator = markers.length === 2 ? markers[1] : lines.length;
  const main = lines.slice(separator + 1).join("\n");
  if (!main.trim()) {
    throw new PublishedGraphAcquisitionRefusal(
      "missing-lock-selection",
      "pnpm lockfile holds only an env document; no project lockfile follows it"
    );
  }
  const env = lines.slice(1, separator).join("\n");
  requirePnpmEnvDocument(env);
  return { env, main };
}

function requirePnpmEnvDocument(env) {
  const refuse = detail => {
    throw new PublishedGraphAcquisitionRefusal(
      "unsupported-lock-syntax",
      `pnpm lockfile's leading document is not pnpm's env document: ${detail}`
    );
  };
  const top = new Set();
  let inImporters = false;
  for (const line of env.split("\n")) {
    if (!line.trim() || line.trimStart().startsWith("#")) continue;
    if (!line.startsWith(" ")) {
      const key = pnpmMappingKey(line);
      if (key === null) refuse(`unreadable top-level line ${JSON.stringify(line)}`);
      if (!PNPM_ENV_TOP_LEVEL_KEYS.has(key)) refuse(`it has a top-level ${JSON.stringify(key)}`);
      if (top.has(key)) refuse(`it repeats ${JSON.stringify(key)}`);
      inImporters = key === "importers";
      if (inImporters && !["importers:", "importers: {}"].includes(line.trimEnd())) {
        refuse("its importers are not a block mapping");
      }
      top.add(key);
      continue;
    }
    if (!inImporters) continue;
    const indent = line.length - line.replace(/^ +/, "").length;
    const key = () => {
      const parsed = pnpmMappingKey(line.slice(indent));
      if (parsed === null) refuse(`unreadable importer line ${JSON.stringify(line)}`);
      return parsed;
    };
    if (indent === 2 && key() !== ".") refuse(`it has importer ${JSON.stringify(key())}`);
    else if (indent === 4 && !PNPM_ENV_IMPORTER_KEYS.has(key())) {
      refuse(`its importer records ${JSON.stringify(key())}`);
    } else if (indent !== 2 && indent !== 4 && indent < 6) {
      refuse(`unexpected indentation in ${JSON.stringify(line)}`);
    }
  }
  requirePnpmLockfileMajor(env.split("\n"));
}

function requirePnpmLockfileMajor(lines) {
  const version = lines
    .find(line => /^lockfileVersion\s*:/.test(line))
    ?.split(":")
    .slice(1)
    .join(":");
  const declared = version === undefined ? null : pnpmScalar(version);
  if (declared === null) {
    throw new PublishedGraphAcquisitionRefusal(
      "unsupported-lock-version",
      "pnpm lockfile does not declare a lockfileVersion"
    );
  }
  if (Number.parseInt(declared, 10) !== PNPM_LOCKFILE_MAJOR) {
    throw new PublishedGraphAcquisitionRefusal(
      "unsupported-lock-version",
      `pnpm lockfile major ${declared} is not ${PNPM_LOCKFILE_MAJOR}; ` +
        "earlier majors write peer suffixes into packages keys, so one name@version " +
        "can appear under several keys and exact selection is not decidable here"
    );
  }
}

/**
 * One document's `packages:` block as `key -> integrity | null` (null: the key
 * is present with no registry integrity), or null when the document has no
 * `packages:` block. An inline `packages: {}` is an empty block.
 */
function pnpmPackagesBlock(lines) {
  if (lines.some(line => line.trimEnd() === "packages: {}")) return new Map();
  const start = lines.findIndex(line => line === "packages:");
  if (start < 0) return null;
  const entries = new Map();
  let key = null;
  for (let index = start + 1; index < lines.length; index += 1) {
    const line = lines[index];
    if (!line.trim()) continue;
    if (!line.startsWith(" ")) break;
    const entry = /^ {2}(\S.*?):\s*$/.exec(line);
    if (entry) {
      key = pnpmScalar(entry[1]);
      if (key === null) {
        throw new PublishedGraphAcquisitionRefusal(
          "unsupported-lock-syntax",
          `pnpm lockfile has an unreadable packages key on line ${index + 1}`
        );
      }
      if (entries.has(key)) {
        throw new PublishedGraphAcquisitionRefusal(
          "ambiguous-lock-selection",
          `pnpm lockfile repeats packages key ${key}`
        );
      }
      entries.set(key, null);
      continue;
    }
    const resolution = /^ {4}resolution:\s*(.*)$/.exec(line);
    if (!resolution || key === null) continue;
    let body = resolution[1];
    // pnpm writes this inline; a formatter may wrap it across lines. Gather
    // until the braces balance rather than assuming either layout.
    while (!body.trim().endsWith("}") && index + 1 < lines.length) {
      index += 1;
      if (!lines[index].startsWith("    ")) break;
      body += lines[index].trim();
    }
    const integrity = pnpmFlowMapping(body, `resolution of ${key}`).get("integrity");
    // A package resolved from a tarball, git or link has no registry
    // integrity. Leaving it unselected is the fail-closed direction: the
    // caller then cannot name it and the graph refuses, rather than the
    // adapter inventing a locator for bytes it cannot authenticate.
    if (typeof integrity === "string" && PNPM_INTEGRITY.test(integrity)) entries.set(key, integrity);
  }
  return entries;
}

/**
 * Parses the `packages:` block of an untrusted pnpm lockfile into exact
 * `name@version` -> integrity selections.
 *
 * Deliberately a reader for one block of one lockfile major rather than a YAML
 * parser. It is acquisition material only -- Rust re-reads the same bytes before
 * any receipt -- but it decides which published artifact is fetched, so every
 * shape it does not implement is a refusal. What it accepts is what pnpm writes
 * and what a formatter may have rewritten: either quote style, and `resolution`
 * as an inline or multi-line flow mapping with an optional trailing comma.
 *
 * A pnpm 11+ lockfile is read from its project document (`pnpmDocuments`). The
 * env document's packages select nothing, and a key it shares with the project
 * document must carry the same resolution there, or it is refused.
 */
export function parsePnpmLockPackages(source) {
  const text = source.replace(/\r\n/g, "\n");
  refusePnpmYamlBeyondSubset(text);
  const { env, main } = pnpmDocuments(text);
  const lines = main.split("\n");
  requirePnpmLockfileMajor(lines);
  const block = pnpmPackagesBlock(lines);
  if (block === null) {
    throw new PublishedGraphAcquisitionRefusal(
      "missing-lock-selection",
      "pnpm lockfile has no packages block"
    );
  }
  const shared = env === null ? new Map() : (pnpmPackagesBlock(env.split("\n")) ?? new Map());
  const selections = new Map();
  for (const [key, integrity] of block) {
    if (shared.has(key) && shared.get(key) !== integrity) {
      throw new PublishedGraphAcquisitionRefusal(
        "ambiguous-lock-selection",
        `pnpm lockfile's env document records ${key} with a different resolution than its project document`
      );
    }
    if (integrity !== null) selections.set(key, integrity);
  }
  if (selections.size === 0) {
    throw new PublishedGraphAcquisitionRefusal(
      "missing-lock-selection",
      "pnpm lockfile names no package with a registry integrity"
    );
  }
  return selections;
}

/**
 * Indexes untrusted pnpm lock bytes into the same exact-selection shape the Bun
 * index produces, so `exactLockSelection` is one function for both.
 *
 * pnpm needs no installed-path locator, and that is a property of its store
 * rather than a simplification. Bun's tree can hold one `name@version` at two
 * paths with *different* integrity, which is why its locator disambiguates;
 * pnpm's store is content-addressed, its `packages:` keys are exactly
 * `name@version`, and each carries one integrity. So the locator here is the key
 * itself, and a same-version duplicate is a refusal above rather than a
 * selection to disambiguate.
 */
export function createPnpmLockSelectionIndex(lockfile) {
  const recordsByIdentity = new Map();
  for (const [identity, integrity] of parsePnpmLockPackages(lockfile)) {
    recordsByIdentity.set(identity, [Object.freeze({ locator: identity, integrity })]);
  }
  const index = Object.freeze({});
  bunLockSelectionRecords.set(index, recordsByIdentity);
  return index;
}

/**
 * Indexes untrusted npm lock bytes (`lockfileVersion` 2 or 3) into the same
 * exact-selection shape the Bun index produces.
 *
 * npm's `packages` map is keyed by install path relative to the lockfile
 * (`node_modules/a/node_modules/b`), exactly Bun's disambiguation problem, so
 * the locator is that key and `lockLocatorForInstalledPackage` derives it from
 * the installed root the same way. An entry is selected only when it names a
 * registry tarball: a link, a workspace member, a `file:` or git dependency has
 * no registry integrity, and an alias (`"name"` differing from the path) would
 * put one package's bytes under another's name. Each is left unselected, which
 * is the fail-closed direction -- the caller cannot name it.
 */
export function createNpmLockSelectionIndex(lockfile) {
  let document;
  try {
    document = JSON.parse(lockfile);
  } catch (error) {
    throw new PublishedGraphAcquisitionRefusal(
      "unsupported-lock-syntax",
      `npm lockfile is not JSON: ${error.message}`
    );
  }
  if (!NPM_LOCKFILE_VERSIONS.has(document?.lockfileVersion)) {
    throw new PublishedGraphAcquisitionRefusal(
      "unsupported-lock-version",
      `npm lockfileVersion ${JSON.stringify(document?.lockfileVersion)} is not 2 or 3; ` +
        "only the path-keyed packages map identifies an installed copy exactly"
    );
  }
  if (!document.packages || typeof document.packages !== "object") {
    throw new PublishedGraphAcquisitionRefusal(
      "missing-lock-selection",
      "npm lockfile has no packages map"
    );
  }
  const recordsByIdentity = new Map();
  for (const [locator, record] of Object.entries(document.packages)) {
    const name = npmLockPathName(locator);
    if (!name || !record || typeof record !== "object" || record.link === true) continue;
    if (typeof record.version !== "string") continue;
    if (record.name !== undefined && record.name !== name) continue;
    if (typeof record.resolved !== "string" || !/^https:\/\//.test(record.resolved)) continue;
    const indexed = Object.freeze({
      locator,
      integrity: typeof record.integrity === "string" ? record.integrity : undefined
    });
    const identity = `${name}@${record.version}`;
    const records = recordsByIdentity.get(identity);
    if (records) records.push(indexed);
    else recordsByIdentity.set(identity, [indexed]);
  }
  const index = Object.freeze({});
  bunLockSelectionRecords.set(index, recordsByIdentity);
  return index;
}

/** The package name an npm `packages` key installs, or null for the root and
 * for any key that does not end in a `node_modules/<name>` segment. */
export function npmLockPathName(locator) {
  const marker = "node_modules/";
  const at = locator.lastIndexOf(marker);
  if (at < 0 || (at > 0 && locator[at - 1] !== "/")) return null;
  const name = locator.slice(at + marker.length);
  const parts = name.split("/");
  if (name.startsWith("@") ? parts.length !== 2 : parts.length !== 1) return null;
  if (parts.some(part => !part || part === "." || part === "..")) return null;
  return name;
}

/** Indexes lock bytes for the named package manager. */
export function createLockSelectionIndex(lockfile, packageManager) {
  if (packageManager === "bun") return createBunLockSelectionIndex(lockfile);
  if (packageManager === "pnpm") return createPnpmLockSelectionIndex(lockfile);
  if (packageManager === "npm") return createNpmLockSelectionIndex(lockfile);
  throw new PublishedGraphAcquisitionRefusal(
    "unsupported-package-manager",
    `no exact lock reader for ${packageManager}`
  );
}

/**
 * Parses and indexes untrusted Bun lock bytes once for exact installed-package
 * selection. The index is acquisition material only; native certification
 * still authenticates the transported lock bytes and every graph edge.
 */
export function createBunLockSelectionIndex(lockfile) {
  const document = parseJsonLike(lockfile);
  requireBunLockfileVersion(document);
  const recordsByIdentity = new Map();
  for (const [locator, record] of Object.entries(document.packages ?? {})) {
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

/** The npm `packages` key for one installed package: its path relative to the
 * lockfile's directory, which must descend through `node_modules`. */
export function npmLockLocatorForInstalledPackage(npmLockPath, packageRoot) {
  const installed = relative(dirname(resolve(npmLockPath)), resolve(packageRoot));
  const locator = installed.split(sep).join("/");
  if (!locator || locator.startsWith("../") || npmLockPathName(locator) === null) {
    throw new PublishedGraphAcquisitionRefusal(
      "installed-lock-layout",
      `${packageRoot} is not under the npm lockfile's node_modules tree`
    );
  }
  return locator;
}

/**
 * The exact lock key for one installed package, per package manager.
 *
 * Bun derives it from the install path because its tree is where the identity
 * lives; pnpm's key *is* `name@version`, so the installed path carries no
 * additional selection and deriving one from `.pnpm/<name>@<version>_<peers>/`
 * would only invent a string the lockfile never wrote.
 */
export function lockLocatorForInstalledPackage({
  lockfilePath,
  packageManager,
  packageRoot,
  packageName,
  packageVersion
}) {
  if (packageManager === "bun") {
    return bunLockLocatorForInstalledPackage(lockfilePath, packageRoot);
  }
  if (packageManager === "pnpm") {
    return `${packageName}@${packageVersion}`;
  }
  if (packageManager === "npm") {
    return npmLockLocatorForInstalledPackage(lockfilePath, packageRoot);
  }
  throw new PublishedGraphAcquisitionRefusal(
    "unsupported-package-manager",
    `no exact lock locator for ${packageManager}`
  );
}

/** Selects one installed package's exact lock record, for either manager. */
export function exactLockSelection({
  index,
  packageManager,
  lockfilePath,
  packageRoot,
  packageName,
  packageVersion
}) {
  return exactBunLockSelection(
    index,
    packageName,
    packageVersion,
    lockLocatorForInstalledPackage({
      lockfilePath,
      packageManager,
      packageRoot,
      packageName,
      packageVersion
    })
  );
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
      if (!dependencyName || nodeBuiltinSpecifier(dependency.specifier)) {
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
