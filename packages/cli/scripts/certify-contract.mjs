// Policy-2 certification transaction.
//
// Node owns exact registry acquisition, temporary files, producer process
// lifecycle, and the final publication transaction. Rust owns proposal
// semantics, demand derivation, witness verification, accepted bytes, and the
// catalog bytes. Audit output is diagnostic only and is never an authority
// input to this command.

import { createHash } from "node:crypto";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  renameSync,
  rmSync,
  writeFileSync
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import process from "node:process";

import { runNativeAsync } from "../bin/launcher.mjs";
import {
  ArtifactResolutionError,
  ArtifactResolutionSession,
  locateExternalDependencyPackageRoot,
  resolvePackageArtifactClosure
} from "./artifact-resolution.mjs";
export { locateExternalDependencyPackageRoot } from "./artifact-resolution.mjs";
import {
  CERTIFICATION_INPUTS_FORMAT,
  artifactCaseDisposition,
  finiteArtifactCandidates,
  finiteConditionPartitions,
  finiteEntrypoints,
  generatePackageContract,
  prepareArtifact
} from "./generate-package-contract.mjs";
import {
  bunLockLocatorForInstalledPackage,
  createBunLockSelectionIndex,
  exactBunLockSelection,
  publishedGraphRequestKey
} from "./published-contract-graph.mjs";

export const contractCertifyHelp = `Usage:
  solid-checker contract certify --integrity <SRI> [OPTIONS]

Acquires the exact published package archive, generates an open proposal,
asks Rust for policy-2 proof demands, and attempts authoritative certification.
The catalog is replaced only after Rust has produced accepted bytes and a
configured issuer has produced a receipt. Audit files are non-authoritative.

Options:
  --package-root <DIR>    Installed package root (default: current directory)
  --integrity <SRI>       Package-manager-pinned archive integrity (required)
  --catalog <FILE>        Accepted-contract catalog to publish after success
  --issuer-configuration <FILE>
                          External policy-2 signing configuration
  --trust-configuration-output <FILE>
                          External public trust configuration for discovery
  --registry-origin <URL> Exact HTTPS registry origin (default: npm registry)
  --entrypoint <SUBPATH>  Exact exported subpath (repeatable)
  --conditions <LIST>     Exact runtime conditions, comma-separated
  --proposal-refusal-audit <FILE>
                          Reuse a complete current dependency-refusal census
                          after authenticated artifact acquisition
  --proposal <FILE>       Reuse a proposal that contract generate emitted for this
                          exact package root, integrity, importer, entrypoints
                          and conditions (with its .proposal.json and
                          .certification-inputs.json sidecars) instead of
                          regenerating it; anything that does not match
                          regenerates
  --audit-output <FILE>   Write a non-replayable diagnostic transcript
  -h, --help              Show this help

Environment:
  SOLID_CHECKER_REGISTRY_CACHE <DIR>
                          Content-addressed store for registry bytes already
                          acquired for an exact (origin, package, version,
                          integrity). Unset or empty: every acquisition
                          fetches from the registry.
  SOLID_CHECKER_REGISTRY_CONCURRENCY <N>
                          Parallel registry acquisitions per certification
                          (default 8)
`;

export const contractCertificationStages = Object.freeze([
  "artifactAcquisition",
  "proposalGeneration",
  "demandPlanning",
  "witnessAcquisition",
  "certification",
  "receiptIssuance",
  "catalogPublication"
]);

export function isExactDependencyCompositionRefusal(refusal) {
  return /external export-all|unaccepted external dependency|accepted dependency contract|accepted dependency .* exact .* binding/i.test(
    refusal?.reason ?? ""
  );
}

export class CertificationRefusal extends Error {
  constructor({ stage, owner, reason, demandId = null, family = null, refusals = [] }) {
    const location = demandId ? ` for demand ${demandId}` : "";
    super(`${stage} refused${location}: ${reason}`);
    this.name = "CertificationRefusal";
    this.stage = stage;
    this.owner = owner;
    this.reason = reason;
    this.demandId = demandId;
    this.family = family;
    this.refusals = refusals;
  }
}

function requireFunction(owner, name) {
  const operation = owner?.[name];
  if (typeof operation !== "function") {
    throw new TypeError(`contract certification stage ${name} is not configured`);
  }
  return operation;
}

function requireProduct(value, stage, authority) {
  if (!value || typeof value !== "object" || value.authority !== authority) {
    throw new TypeError(
      `contract certification stage ${stage} must return a ${authority}-owned product`
    );
  }
  return value;
}

/// Runs the publication path as one ordered transaction. In particular,
/// `commit` is unreachable until every witness, certification result, and
/// receipt has been authenticated by its owning adapter.
export async function runContractCertificationPipeline({
  request,
  acquisition,
  proposal,
  rust,
  evidence,
  issuer,
  publication
}) {
  const artifactSnapshot = await requireFunction(acquisition, "acquireArtifacts")(request);
  const openProposal = requireProduct(
    await requireFunction(proposal, "generate")({ request, artifactSnapshot }),
    "proposalGeneration",
    "rust"
  );
  const demandPlan = requireProduct(
    await requireFunction(rust, "planDemands")({ request, artifactSnapshot, openProposal }),
    "demandPlanning",
    "rust"
  );
  const witnesses = await requireFunction(evidence, "obtainWitnesses")({
    request,
    artifactSnapshot,
    openProposal,
    demandPlan
  });
  const accepted = requireProduct(
    await requireFunction(rust, "certify")({
      request,
      artifactSnapshot,
      openProposal,
      demandPlan,
      witnesses
    }),
    "certification",
    "rust"
  );
  const receipt = requireProduct(
    await requireFunction(issuer, "issue")({ request, artifactSnapshot, demandPlan, accepted }),
    "receiptIssuance",
    "configured-issuer"
  );
  return await requireFunction(publication, "commit")({
    request,
    artifactSnapshot,
    accepted,
    receipt
  });
}

export function parseCertifyArguments(arguments_) {
  const options = {
    packageRoot: process.cwd(),
    integrity: "",
    catalog: "",
    registryOrigin: "https://registry.npmjs.org",
    entrypoints: [],
    conditions: [],
    proposalRefusalAudit: "",
    proposal: "",
    auditOutput: "",
    issuerConfiguration: process.env.SOLID_CHECKER_POLICY2_ISSUER_CONFIG ?? "",
    trustConfigurationOutput: process.env.SOLID_CHECKER_POLICY2_TRUST_CONFIG ?? "",
    help: false
  };
  for (let index = 0; index < arguments_.length; index += 1) {
    const argument = arguments_[index];
    if (["--help", "-h"].includes(argument)) return { ...options, help: true };
    const separator = argument.indexOf("=");
    const key = separator < 0 ? argument : argument.slice(0, separator);
    const value = separator < 0 ? arguments_[++index] : argument.slice(separator + 1);
    if (!key.startsWith("--") || value === undefined || value === "") {
      throw new Error(`${key} needs a value`);
    }
    if (key === "--package-root") options.packageRoot = value;
    else if (key === "--integrity") options.integrity = value;
    else if (key === "--catalog") options.catalog = value;
    else if (key === "--registry-origin") options.registryOrigin = value;
    else if (key === "--entrypoint") options.entrypoints.push(value);
    else if (key === "--conditions") {
      options.conditions.push(...value.split(",").map(item => item.trim()).filter(Boolean));
    } else if (key === "--audit-output") options.auditOutput = value;
    else if (key === "--proposal-refusal-audit") options.proposalRefusalAudit = value;
    else if (key === "--proposal") options.proposal = value;
    else if (key === "--issuer-configuration") options.issuerConfiguration = value;
    else if (key === "--trust-configuration-output") options.trustConfigurationOutput = value;
    else throw new Error(`unknown contract certification argument ${key}`);
  }
  if (!options.integrity) {
    throw new Error(
      "--integrity is required; certification cannot choose package bytes from registry metadata alone"
    );
  }
  if (!/^https:\/\/[^/?#@]+$/.test(options.registryOrigin)) {
    throw new Error("--registry-origin must be a canonical HTTPS origin without a path");
  }
  return options;
}

function exactRegistryPackageUrl(origin, packageName) {
  return `${origin}/${encodeURIComponent(packageName)}`;
}

async function checkedResponse(response, label) {
  if (!response.ok) throw new Error(`${label} returned HTTP ${response.status}`);
  return new Uint8Array(await response.arrayBuffer());
}

/// The exact registry record the acquisition is allowed to use, or a thrown
/// refusal. Both the fresh path and the cache path run this on the metadata
/// bytes they are about to hand Rust, so a cached packument is held to the
/// same checks as a freshly served one.
function selectExactRegistryRecord(metadataBytes, options, manifest) {
  let metadata;
  try {
    metadata = JSON.parse(new TextDecoder().decode(metadataBytes));
  } catch (error) {
    throw new Error(`registry metadata is not JSON: ${error.message}`);
  }
  const selected = metadata?.versions?.[manifest.version];
  if (selected?.name !== manifest.name || selected?.version !== manifest.version) {
    throw new Error(`registry metadata has no exact ${manifest.name}@${manifest.version} record`);
  }
  if (selected.dist?.integrity !== options.integrity) {
    throw new Error(
      `registry integrity for ${manifest.name}@${manifest.version} disagrees with --integrity`
    );
  }
  if (
    typeof selected.dist?.tarball !== "string" ||
    !selected.dist.tarball.startsWith(`${options.registryOrigin}/`) ||
    /[?#]/.test(selected.dist.tarball)
  ) {
    throw new Error("registry tarball URL is outside the exact registry origin");
  }
  return selected;
}

/// `true` when the archive bytes hash to the sha512 SRI the caller pinned,
/// `false` when they do not, and `null` when the integrity is not a sha512
/// SRI at all and so cannot be checked here.
function archiveMatchesIntegrity(archiveBytes, integrity) {
  const match = /^sha512-([A-Za-z0-9+/]+={0,2})$/.exec(integrity ?? "");
  if (!match) return null;
  return createHash("sha512").update(archiveBytes).digest("base64") === match[1];
}

/// Registry acquisitions are content-addressed: an exact (origin, package,
/// version, integrity) names one archive, and the archive is self-authenticating
/// through its sha512 SRI. `SOLID_CHECKER_REGISTRY_CACHE` names a directory
/// where acquisitions already made for that exact identity are kept so the
/// same bytes need not cross the network again — every ecosystem probe that
/// depends on `solid-js` otherwise re-downloads the same packument and tarball.
///
/// What the cache does *not* weaken: an entry is used only when its archive
/// still hashes to the pinned integrity and its packument still carries the
/// exact version record that names that integrity inside the same origin —
/// the identical checks the fresh path performs — and Rust re-derives the
/// snapshot from those bytes and refuses any lock disagreement exactly as
/// before. What it does not preserve is packument freshness for fields
/// outside the exact version record, which no certification input reads.
/// Unset or empty disables it; every read and write is best-effort and falls
/// back to a fresh registry acquisition.
export function registryCacheRoot(env = process.env) {
  const raw = env.SOLID_CHECKER_REGISTRY_CACHE;
  if (raw === undefined || raw === "") return null;
  return resolve(raw);
}

export function registryAcquisitionConcurrency(env = process.env) {
  const raw = env.SOLID_CHECKER_REGISTRY_CONCURRENCY;
  if (raw === undefined || raw === "") return 8;
  const value = Number(raw);
  if (!Number.isInteger(value) || value < 1) {
    throw new Error(
      `SOLID_CHECKER_REGISTRY_CONCURRENCY must be a positive integer, got ${JSON.stringify(raw)}`
    );
  }
  return value;
}

const REGISTRY_CACHE_FORMAT = "v1";

function registryCacheEntry(cacheRoot, options, manifest) {
  const key = createHash("sha256")
    .update(
      JSON.stringify([options.registryOrigin, manifest.name, manifest.version, options.integrity])
    )
    .digest("hex");
  return join(cacheRoot, REGISTRY_CACHE_FORMAT, key.slice(0, 2), key);
}

function readRegistryCacheEntry(entry, options, manifest) {
  const metadataPath = join(entry, "registry-metadata.json");
  const archivePath = join(entry, "package.tgz");
  if (!existsSync(metadataPath) || !existsSync(archivePath)) return null;
  try {
    const metadataBytes = new Uint8Array(readFileSync(metadataPath));
    const archiveBytes = new Uint8Array(readFileSync(archivePath));
    selectExactRegistryRecord(metadataBytes, options, manifest);
    if (archiveMatchesIntegrity(archiveBytes, options.integrity) !== true) {
      throw new Error("cached archive does not hash to the pinned integrity");
    }
    return { metadataPath, archivePath };
  } catch {
    // An entry that fails its own checks is not evidence of anything; drop it
    // so the fresh acquisition below can replace it.
    rmSync(entry, { recursive: true, force: true });
    return null;
  }
}

function writeRegistryCacheEntry(entry, metadataBytes, archiveBytes) {
  let staging = null;
  try {
    mkdirSync(dirname(entry), { recursive: true });
    staging = mkdtempSync(`${entry}.staging-`);
    writeFileSync(join(staging, "registry-metadata.json"), metadataBytes);
    writeFileSync(join(staging, "package.tgz"), archiveBytes);
    // Publish atomically. Concurrent certifications race to fill the same
    // entry with identical content; whichever rename lands first wins and
    // the other staging directory is discarded below.
    renameSync(staging, entry);
    staging = null;
  } catch {
    // Best-effort: a cache that cannot be written only costs the next
    // acquisition a network round trip.
  } finally {
    if (staging) rmSync(staging, { recursive: true, force: true });
  }
}

async function acquirePublishedArtifact({
  options,
  manifest,
  scratch,
  fetch_ = fetch,
  cacheRoot = registryCacheRoot()
}) {
  const cacheEntry = cacheRoot ? registryCacheEntry(cacheRoot, options, manifest) : null;
  const cached = cacheEntry ? readRegistryCacheEntry(cacheEntry, options, manifest) : null;
  let metadataPath;
  let archivePath;
  if (cached) {
    // Rust only reads these paths, and the entry outlives every scratch
    // directory, so the validated bytes are named in place: a certification
    // with dozens of sources otherwise rewrites tens of megabytes it already
    // holds on disk.
    ({ metadataPath, archivePath } = cached);
  } else {
    const metadataResponse = await fetch_(
      exactRegistryPackageUrl(options.registryOrigin, manifest.name),
      // The install-v1 packument preserves the exact version/dist identity Rust
      // authenticates while excluding unrelated readmes and publisher metadata
      // that can exceed the pinned bounded-JSON string limit. The response bytes
      // themselves still cross the native provenance boundary unchanged.
      { headers: { accept: "application/vnd.npm.install-v1+json" } }
    );
    const metadataBytes = await checkedResponse(
      metadataResponse,
      "registry metadata acquisition"
    );
    const selected = selectExactRegistryRecord(metadataBytes, options, manifest);
    const archiveResponse = await fetch_(selected.dist.tarball, {
      headers: { accept: "application/octet-stream" }
    });
    const archiveBytes = await checkedResponse(archiveResponse, "package archive acquisition");
    metadataPath = join(scratch, "registry-metadata.json");
    archivePath = join(scratch, "package.tgz");
    writeFileSync(metadataPath, metadataBytes);
    writeFileSync(archivePath, archiveBytes);
    // Only bytes that already authenticate against the pinned integrity are
    // worth remembering; anything else Rust is about to refuse anyway.
    if (cacheEntry && archiveMatchesIntegrity(archiveBytes, options.integrity) === true) {
      writeRegistryCacheEntry(cacheEntry, metadataBytes, archiveBytes);
    }
  }
  return Object.freeze({
    registryOrigin: options.registryOrigin,
    metadataPath,
    archivePath,
    package: manifest.name,
    version: manifest.version,
    integrity: options.integrity
  });
}

function certificationPlannings(generated, artifactSnapshot) {
  return generated.certificationInputs.map(input => ({
    schemaVersion: 1,
    proposal: generated.output,
    resolution: input.resolution,
    exportConditions: [...new Set([...input.conditions, "import"])].sort(),
    registryOrigin: artifactSnapshot.registryOrigin,
    registryMetadata: artifactSnapshot.metadataPath,
    archive: artifactSnapshot.archivePath
  }));
}

async function planDemands({ options, generated, artifactSnapshot, scratch }) {
  const plans = [];
  for (const [index, planning] of certificationPlannings(generated, artifactSnapshot).entries()) {
    const requestPath = join(scratch, `certification-request-${index}.json`);
    const outputPath = join(scratch, `certification-plan-${index}.json`);
    writeFileSync(requestPath, `${JSON.stringify(planning, null, 2)}\n`);
    const child = await runNativeAsync(
      "solid-checker",
      [
        "--plan-contract-certification",
        requestPath,
        "--certification-plan-output",
        outputPath
      ],
      { cwd: options.packageRoot, env: { SOLID_CHECKER_DAEMON: "0" } }
    );
    if (child.error) throw new Error(`could not start the native checker: ${child.error.message}`);
    if (child.status !== 0) {
      throw new Error(
        child.stderr.trim() || child.stdout.trim() || `native checker exited ${child.status}`
      );
    }
    plans.push(JSON.parse(readFileSync(outputPath, "utf8")));
  }
  return plans;
}

function reviewGraphProposal(generated) {
  const review = JSON.parse(readFileSync(generated.plan, "utf8"));
  const artifactCases = new Set();
  const visit = value => {
    if (Array.isArray(value)) {
      for (const child of value) visit(child);
    } else if (value && typeof value === "object") {
      if (typeof value.artifactCase === "string") artifactCases.add(value.artifactCase);
      for (const child of Object.values(value)) visit(child);
    }
  };
  visit(review);
  if (artifactCases.size !== 1) {
    throw new Error(
      `exact graph proposal plan named ${artifactCases.size} artifact cases; expected one`
    );
  }
  return {
    selectedArtifactCase: artifactCases.values().next().value,
    candidateSemanticDigest: review.semanticDigest,
    demands: review.unresolvedClaims ?? []
  };
}

function findBunLock(packageRoot) {
  let directory = resolve(packageRoot);
  while (true) {
    const candidate = join(directory, "bun.lock");
    if (existsSync(candidate)) return candidate;
    const parent = dirname(directory);
    if (parent === directory) break;
    directory = parent;
  }
  throw new CertificationRefusal({
    stage: "artifact-acquisition",
    owner: "package-manager",
    reason: `no exact Bun text lockfile exists above ${packageRoot}`
  });
}

/// Walks the declaration-only closure a package's typings reach and names the
/// exact published artifact behind each one.
///
/// This is the single traversal both certification paths use: an ordinary root
/// package and every node of a published dependency graph. What it returns is
/// only a *naming* of packages — name, version, exact Bun lock locator and
/// integrity, and the installed root the resolver used. No installed byte
/// becomes evidence: the caller acquires each named package from the registry,
/// and Rust re-derives the snapshot from those archive bytes and refuses any
/// one whose lock selection disagrees.
///
/// A package the lockfile does not select exactly, or whose declarations do not
/// resolve, is simply not named. That is the fail-closed direction: the witness
/// program then cannot resolve the reference, and the demands that needed it
/// stay open exactly as they were.
function createCompilerSourceCollector({
  bunLockPath,
  bunLockIndex,
  scratch,
  resolutionSession,
  scratchPrefix,
  onUnnameable = null
}) {
  const sourceArtifacts = new Map();
  const compilerSourceClosures = new Map();
  let nextSourceIndex = 0;
  const locateExternalFrom = (ownerRoot, dependency) => {
    if (dependency.specifier.startsWith("node:")) return null;
    const dependencyName = packageNameOfSpecifier(dependency.specifier);
    const dependencyImporter = resolve(
      ownerRoot,
      dependency.importerPath ?? dependency.source
    );
    const dependencyRoot = locateExternalDependencyPackageRoot(
      dependencyImporter,
      dependency
    );
    if (!dependencyRoot) return null;
    const dependencyManifest = JSON.parse(
      readFileSync(join(dependencyRoot, "package.json"), "utf8")
    );
    const dependencyLock = exactBunLockSelection(
      bunLockIndex,
      dependencyManifest.name,
      dependencyManifest.version,
      bunLockLocatorForInstalledPackage(bunLockPath, dependencyRoot)
    );
    return {
      ...dependency,
      dependencyImporter,
      dependencyRoot,
      dependencyManifest,
      dependencyLock
    };
  };
  const collectCompilerSources = async (
    located,
    sourceConditions,
    semanticRoots,
    visiting = new Set()
  ) => {
    const installedRoot = resolve(located.dependencyRoot);
    if (semanticRoots.has(installedRoot)) return [];
    const memoKey = JSON.stringify([
      installedRoot,
      located.specifier,
      [...new Set(sourceConditions)].sort(),
      [...semanticRoots].sort()
    ]);
    if (!visiting.has(memoKey)) {
      let memoized = compilerSourceClosures.get(memoKey);
      if (!memoized) {
        memoized = collectCompilerSources(
          located,
          sourceConditions,
          semanticRoots,
          new Set(visiting).add(memoKey)
        );
        // A memo entry is the in-flight promise, so a rejection would otherwise
        // be replayed to every later caller of this key for the rest of the
        // run. Forget a rejected traversal so an independent caller retries it
        // rather than inheriting a failure it never provoked.
        memoized = memoized.catch(error => {
          if (compilerSourceClosures.get(memoKey) === memoized) {
            compilerSourceClosures.delete(memoKey);
          }
          throw error;
        });
        compilerSourceClosures.set(memoKey, memoized);
      }
      return memoized;
    }
    const artifactKey = JSON.stringify([
      installedRoot,
      located.dependencyLock.locator,
      located.dependencyLock.integrity
    ]);
    let source = sourceArtifacts.get(artifactKey);
    if (!source) {
      const sourceScratch = join(scratch, `${scratchPrefix}-source-${nextSourceIndex++}`);
      mkdirSync(sourceScratch);
      source = {
        key: artifactKey,
        manifest: located.dependencyManifest,
        integrity: located.dependencyLock.integrity,
        scratch: sourceScratch,
        packageName: located.dependencyManifest.name,
        packageVersion: located.dependencyManifest.version,
        lockfile: resolve(bunLockPath),
        lockLocator: located.dependencyLock.locator,
        installedPackageRoot: installedRoot
      };
      sourceArtifacts.set(artifactKey, source);
    }
    const traversalKey = JSON.stringify([
      installedRoot,
      located.specifier,
      [...new Set(sourceConditions)].sort()
    ]);
    if (visiting.has(traversalKey)) return [source];
    const nextVisiting = new Set(visiting).add(traversalKey);
    let closure;
    try {
      closure = resolvePackageArtifactClosure({
        importer: located.dependencyImporter,
        specifier: located.specifier,
        packageRoot: located.dependencyRoot,
        conditions: sourceConditions,
        resolutionKind: "import",
        integrity: located.dependencyLock.integrity
      }, resolutionSession);
    } catch (error) {
      if (["target-not-found", "declarations-not-found"].includes(error?.code)) {
        return [source];
      }
      throw error;
    }
    const transitive = [source];
    for (const dependency of closure.externalDependencies.filter(
      dependency => dependency.axis === "declarations"
    )) {
      // With `onUnnameable` (the root path) one unnameable grandchild poisons
      // only its own package name; the rest of the subtree is still collected.
      // Without it (the published-graph path) the failure propagates exactly as
      // before, because a graph node's canonical identity binds its source set.
      if (onUnnameable) {
        try {
          const child = locateExternalFrom(closure.packageRoot, dependency);
          if (!child) continue;
          transitive.push(
            ...await collectCompilerSources(
              child,
              sourceConditions,
              semanticRoots,
              nextVisiting
            )
          );
        } catch {
          onUnnameable(packageNameOfSpecifier(dependency.specifier));
        }
        continue;
      }
      const child = locateExternalFrom(closure.packageRoot, dependency);
      if (!child) continue;
      transitive.push(
        ...await collectCompilerSources(
          child,
          sourceConditions,
          semanticRoots,
          nextVisiting
        )
      );
    }
    return transitive;
  };
  return {
    locateExternalFrom,
    collectCompilerSources,
    sourceArtifacts,
    compilerSourceClosureCount: () => compilerSourceClosures.size
  };
}

/// The bare package name a specifier addresses, which is also the directory
/// name every copy of it occupies under `node_modules` — the exact granularity
/// module resolution decides at.
export function packageNameOfSpecifier(specifier) {
  return specifier.startsWith("@")
    ? specifier.split("/").slice(0, 2).join("/")
    : specifier.split("/")[0];
}

/// Canonical order for a declaration-only source set, deduplicated by the exact
/// coordinate that identifies one installed package.
function canonicalCompilerSources(sources) {
  return [...new Map(
    sources.map(source => [
      JSON.stringify([
        source.packageName,
        source.packageVersion,
        source.lockLocator,
        source.installedPackageRoot
      ]),
      source
    ])
  ).values()].sort((left, right) =>
    left.installedPackageRoot.localeCompare(right.installedPackageRoot) ||
    left.packageName.localeCompare(right.packageName) ||
    left.packageVersion.localeCompare(right.packageVersion)
  );
}

function rootSpecifier(packageName, entrypoint) {
  return entrypoint && entrypoint !== "."
    ? `${packageName}/${entrypoint.replace(/^\.\//, "")}`
    : packageName;
}

function artifactCaseCoordinate(entrypoint, conditions) {
  if (
    typeof entrypoint !== "string" ||
    !Array.isArray(conditions) ||
    conditions.some(condition => typeof condition !== "string" || !condition)
  ) {
    return null;
  }
  const canonicalConditions = [...new Set(conditions)].sort();
  if (JSON.stringify(conditions) !== JSON.stringify(canonicalConditions)) return null;
  return JSON.stringify([entrypoint, canonicalConditions]);
}

/**
 * Accepts an earlier proposal refusal census only when it is a complete,
 * duplicate-free census of the artifact cases selected from the current
 * installed files and every case carries the exact dependency-composition
 * refusal that activates graph preparation. The census remains untrusted:
 * authenticated archive acquisition has already run, and native graph
 * certification still reconstructs every root, closure, edge, and receipt.
 */
export function isReusableDependencyRefusalAudit({
  audit,
  manifest,
  packageRoot,
  integrity,
  certificationImporter,
  entrypoints = [],
  conditions = []
}) {
  if (
    audit?.format !== "solid-checker-contract-proposal-refusals" ||
    audit?.refusalVersion !== 1 ||
    audit?.package?.name !== manifest?.name ||
    audit?.package?.version !== manifest?.version ||
    typeof integrity !== "string" ||
    !integrity ||
    typeof certificationImporter !== "string" ||
    !certificationImporter ||
    !Array.isArray(audit?.refusals) ||
    audit.refusals.length === 0
  ) {
    return false;
  }
  let currentEntrypoints;
  let candidates;
  try {
    currentEntrypoints = finiteEntrypoints(manifest, entrypoints, packageRoot);
    if (
      currentEntrypoints.wildcardRefusals.length > 0 ||
      currentEntrypoints.wildcardBranchRefusals.length > 0 ||
      currentEntrypoints.wildcardResourceRefusals.length > 0
    ) {
      return false;
    }
    candidates = finiteArtifactCandidates(
      manifest,
      currentEntrypoints.entrypoints,
      finiteConditionPartitions(manifest, conditions),
      packageRoot
    ).filter(candidate =>
      // Re-derived here rather than read from the untrusted audit: a case the
      // current census records inapplicable produces neither a proposal case
      // nor a refusal, so requiring a refusal row for it would reject every
      // reusable audit of a package that has one.
      artifactCaseDisposition({
        manifest,
        packageRoot,
        entrypoint: candidate.entrypoint,
        conditions: candidate.conditions
      }) === null
    );
    if (candidates.length === 0) return false;
  } catch {
    return false;
  }
  const expected = new Set(
    candidates.map(candidate => artifactCaseCoordinate(candidate.entrypoint, candidate.conditions))
  );
  const observed = new Set();
  const refusalsByCoordinate = new Map();
  for (const refusal of audit.refusals) {
    const coordinate = artifactCaseCoordinate(refusal?.entrypoint, refusal?.conditions);
    if (
      coordinate === null ||
      observed.has(coordinate) ||
      refusal.stage !== "artifact-case" ||
      refusal.applicability !== "runtime-module" ||
      !isExactDependencyCompositionRefusal(refusal)
    ) {
      return false;
    }
    observed.add(coordinate);
    refusalsByCoordinate.set(coordinate, refusal);
  }
  if (
    observed.size !== expected.size ||
    ![...observed].every(coordinate => expected.has(coordinate))
  ) {
    return false;
  }

  // Coordinates alone would permit a stale audit to change proposal-failure
  // precedence after source bytes moved. Replay the same current standalone
  // resolver preparation for every exact artifact case and require the same
  // normalized dependency refusal. This deliberately stops before native
  // analysis/Type Facts; a case that now prepares successfully or refuses for
  // any other reason falls back to the ordinary proposal path.
  const resolutionSession = new ArtifactResolutionSession();
  for (const candidate of candidates) {
    const coordinate = artifactCaseCoordinate(candidate.entrypoint, candidate.conditions);
    const audited = refusalsByCoordinate.get(coordinate);
    try {
      prepareArtifact({
        packageRoot,
        manifest,
        integrity,
        entrypoint: candidate.entrypoint,
        conditions: candidate.conditions,
        resolutionSession,
        certificationImporter,
        acceptedDependencies: {}
      });
      return false;
    } catch (error) {
      const currentReason = (error?.message ?? String(error)).replaceAll(
        packageRoot,
        "<package-root>"
      );
      if (
        !isExactDependencyCompositionRefusal({ reason: currentReason }) ||
        currentReason !== audited.reason
      ) {
        return false;
      }
    }
  }
  return true;
}

/// The importer file `contract certify` writes beside a package root, named by
/// the package root and catalog it certifies. Exported so a harness that
/// generates a proposal ahead of certification can generate it under the same
/// importer and hand it over with `--proposal`.
export function certificationImporterPathFor({ packageRoot, catalog }) {
  const resolvedRoot = realpathSync(resolve(packageRoot));
  const resolvedCatalog = resolve(
    catalog || join(resolvedRoot, ".solid-checker", "accepted-contracts.json")
  );
  const importerIdentity = createHash("sha256")
    .update("solid-checker:certification-importer:v1\0")
    .update(resolvedRoot)
    .update("\0")
    .update(resolvedCatalog)
    .digest("hex");
  return join(dirname(resolvedRoot), `.solid-checker-certification-${importerIdentity}.mjs`);
}

function samePathIdentity(left, right) {
  const canonical = path => {
    try {
      return join(realpathSync(dirname(path)), path.split(/[\\/]/).pop());
    } catch {
      return null;
    }
  };
  const a = canonical(left);
  const b = canonical(right);
  return a !== null && b !== null && a === b;
}

/// Decides whether a proposal emitted earlier by `contract generate` may stand
/// in for the one this certification would generate now. Everything the
/// generation was parameterized by must match — package identity, integrity,
/// package root, certification importer, entrypoints, conditions — and the
/// document and plan bytes must still carry the digests the sidecar recorded,
/// so an edited or swapped file is not paired with inputs derived from another.
/// Each named artifact case must also be one the current census would emit.
/// Returns the parsed inputs, or null when the caller must regenerate. Nothing
/// this admits becomes authority: Rust still verifies every resolution against
/// the authenticated archive and proves every claim before a receipt exists.
export function reusableProposalInputs({
  inputs,
  documentBytes,
  planBytes,
  manifest,
  packageRoot,
  integrity,
  certificationImporter,
  entrypoints = [],
  conditions = []
}) {
  const digest = bytes => `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
  const sameList = (left, right) =>
    Array.isArray(left) &&
    Array.isArray(right) &&
    left.length === right.length &&
    left.every((value, index) => value === right[index]);
  if (
    inputs?.format !== CERTIFICATION_INPUTS_FORMAT ||
    inputs?.inputsVersion !== 1 ||
    inputs?.package?.name !== manifest?.name ||
    inputs?.package?.version !== manifest?.version ||
    typeof integrity !== "string" ||
    !integrity ||
    inputs?.integrity !== integrity ||
    typeof inputs?.packageRoot !== "string" ||
    !samePathIdentity(inputs.packageRoot, packageRoot) ||
    typeof certificationImporter !== "string" ||
    !certificationImporter ||
    typeof inputs?.certificationImporter !== "string" ||
    !samePathIdentity(inputs.certificationImporter, certificationImporter) ||
    !sameList(inputs?.entrypoints, entrypoints) ||
    !sameList(inputs?.conditions, conditions) ||
    inputs?.document?.sha256 !== digest(documentBytes) ||
    inputs?.plan?.sha256 !== digest(planBytes) ||
    !Array.isArray(inputs?.certificationInputs) ||
    inputs.certificationInputs.length === 0
  ) {
    return null;
  }
  let expected;
  try {
    const current = finiteEntrypoints(manifest, entrypoints, packageRoot);
    expected = new Set(
      finiteArtifactCandidates(
        manifest,
        current.entrypoints,
        finiteConditionPartitions(manifest, conditions),
        packageRoot
      )
        .filter(candidate =>
          artifactCaseDisposition({
            manifest,
            packageRoot,
            entrypoint: candidate.entrypoint,
            conditions: candidate.conditions
          }) === null
        )
        .map(candidate => artifactCaseCoordinate(candidate.entrypoint, candidate.conditions))
    );
  } catch {
    return null;
  }
  for (const input of inputs.certificationInputs) {
    const coordinate = artifactCaseCoordinate(input?.entrypoint, input?.conditions);
    if (
      coordinate === null ||
      !expected.has(coordinate) ||
      typeof input?.resolution?.specifier !== "string" ||
      typeof input?.resolution?.importer !== "string" ||
      !samePathIdentity(input.resolution.importer, certificationImporter) ||
      // Rust binds the receipt to the resolved import root by exact string, so
      // the resolution must have been computed under the very root string this
      // certification uses — a symlinked spelling of the same directory would
      // certify and then fail to bind, a spurious refusal.
      input.resolution.packageRoot !== packageRoot
    ) {
      return null;
    }
  }
  return inputs;
}

export function validatedReusableDependencyRefusalAuditBytes({
  auditBytes,
  ...validation
}) {
  let audit;
  try {
    audit = JSON.parse(auditBytes.toString("utf8"));
  } catch {
    return null;
  }
  return isReusableDependencyRefusalAudit({ audit, ...validation })
    ? auditBytes
    : null;
}

function storeMergedObject(source, target) {
  if (existsSync(target)) {
    if (!readFileSync(source).equals(readFileSync(target))) {
      throw new Error(`dependency catalog object collision at ${target}`);
    }
    return;
  }
  copyFileSync(source, target);
}

function mergeProposalDependencies(dependencies, outputRoot) {
  if (dependencies.length === 0) {
    return { catalog: "", proposalDependencies: {} };
  }
  mkdirSync(join(outputRoot, "objects"), { recursive: true });
  const contracts = [];
  const proposalDependencies = {};
  for (const dependency of dependencies) {
    const resolution = dependency.planning?.resolution;
    if (
      resolution?.importer !== dependency.node.importer ||
      resolution?.specifier !== dependency.viaSpecifier
    ) {
      throw new Error(
        `dependency proposal does not carry the exact import binding for ${dependency.viaSpecifier}`
      );
    }
    if (proposalDependencies[dependency.viaSpecifier]) {
      throw new Error(`dependency graph repeats exact specifier ${dependency.viaSpecifier}`);
    }
    const documentBytes = readFileSync(dependency.planning.proposal);
    const documentDigest = `sha256:${createHash("sha256").update(documentBytes).digest("hex")}`;
    const documentName = `${documentDigest.slice("sha256:".length)}.json`;
    storeMergedObject(
      dependency.planning.proposal,
      join(outputRoot, "objects", documentName)
    );
    contracts.push({
      document: `objects/${documentName}`,
      documentDigest,
      import: resolution
    });
    proposalDependencies[dependency.viaSpecifier] = {
      packageName: dependency.node.packageName,
      artifactCase: dependency.demandPlan.selectedArtifactCase,
      acceptedContractDigest: dependency.demandPlan.candidateSemanticDigest,
      exports: resolution.exports ?? {}
    };
  }
  contracts.sort((left, right) =>
    left.import.importer.localeCompare(right.import.importer) ||
    left.import.specifier.localeCompare(right.import.specifier)
  );
  const catalog = join(outputRoot, "proposal-dependencies.json");
  writeFileSync(catalog, `${JSON.stringify({
    format: "solid-checker-proposal-dependency-catalog",
    catalogVersion: 1,
    contracts
  })}\n`);
  return { catalog, proposalDependencies };
}

function graphNodeExecutionInput(state) {
  return {
    planning: state.planning,
    lockfile: state.node.bunLockPath,
    lockLocator: state.node.lockLocator,
    sourceDependencies: (state.sourceDependencies ?? []).map(source => ({
      packageName: source.packageName,
      packageVersion: source.packageVersion,
      registryOrigin: source.registryOrigin,
      registryMetadata: source.registryMetadata,
      archive: source.archive,
      lockfile: source.lockfile,
      lockLocator: source.lockLocator,
      installedPackageRoot: source.installedPackageRoot
    }))
  };
}

export function buildPublishedGraphExecutionRequest({
  cases,
  typefactsExecutable,
  issuerConfiguration,
  catalogRoot,
  trustConfigurationOutput
}) {
  if (!Array.isArray(cases) || cases.length === 0) {
    throw new TypeError("published graph execution requires at least one root case");
  }
  const graphFor = item => ({
    root: graphNodeExecutionInput(item.root),
    dependencies: item.nodes
      .filter(state => state !== item.root)
      .map(graphNodeExecutionInput)
  });
  if (cases.length > 1) {
    const nodes = new Map();
    for (const item of cases) {
      for (const state of item.nodes) {
        const key = state.node?.key;
        if (typeof key !== "string" || !key) {
          throw new TypeError("published graph case-set node has no full-identity key");
        }
        const input = graphNodeExecutionInput(state);
        const previous = nodes.get(key);
        if (previous && JSON.stringify(previous) !== JSON.stringify(input)) {
          throw new Error(`published graph full identity ${key} has conflicting acquisition input`);
        }
        nodes.set(key, input);
      }
    }
    return {
      schemaVersion: 5,
      graphCaseSet: {
        nodes: [...nodes]
          .sort(([left], [right]) => left.localeCompare(right))
          .map(([key, input]) => ({ key, ...input })),
        cases: cases.map(item => ({
          root: item.root.node.key,
          nodes: [...new Set(item.nodes.map(state => state.node.key))].sort()
        }))
      },
      typefactsExecutable: resolve(typefactsExecutable),
      issuerConfiguration: resolve(issuerConfiguration),
      catalogRoot,
      trustConfigurationOutput: resolve(trustConfigurationOutput)
    };
  }
  return {
    schemaVersion: 3,
    graph: graphFor(cases[0]),
    typefactsExecutable: resolve(typefactsExecutable),
    issuerConfiguration: resolve(issuerConfiguration),
    catalogRoot,
    trustConfigurationOutput: resolve(trustConfigurationOutput)
  };
}

async function executePreparedPublishedGraphs({
  options,
  cases,
  scratch,
  catalogRoot,
  trustConfigurationOutput
}) {
  const typefactsExecutable = process.env.SOLID_TYPEFACTS_BIN;
  if (!typefactsExecutable) {
    throw new CertificationRefusal({
      stage: "witness-acquisition",
      owner: "type-facts",
      reason: "SOLID_TYPEFACTS_BIN does not name the pinned Type Facts producer"
    });
  }
  const execution = buildPublishedGraphExecutionRequest({
    cases,
    typefactsExecutable,
    issuerConfiguration: options.issuerConfiguration,
    catalogRoot,
    trustConfigurationOutput
  });
  const requestPath = join(scratch, `published-graph-execution-${cases[0].root.index}.json`);
  writeFileSync(requestPath, `${JSON.stringify(execution, null, 2)}\n`);
  const child = await runNativeAsync(
    "solid-checker",
    ["--execute-contract-certification", requestPath],
    { cwd: options.packageRoot, env: { SOLID_CHECKER_DAEMON: "0" } }
  );
  if (child.error) throw new Error(`could not start the native checker: ${child.error.message}`);
  if (child.status !== 0) {
    throw new CertificationRefusal({
      stage: "witness-acquisition",
      owner: "certifier",
      reason: child.stderr.trim() || child.stdout.trim() || `native checker exited ${child.status}`
    });
  }
  return { authority: "native-certification-complete", catalogRoot };
}

function reachableGraphStates(root, byKey) {
  const found = new Set();
  const visit = state => {
    if (found.has(state.node.key)) return;
    for (const dependency of state.node.dependencies) visit(byKey.get(dependency.node));
    found.add(state.node.key);
  };
  visit(root);
  return [...byKey.values()].filter(state => found.has(state.node.key));
}

async function mapWithExactConcurrency(items, concurrency, worker) {
  const results = new Array(items.length);
  let cursor = 0;
  await Promise.all(
    Array.from({ length: Math.min(concurrency, items.length) }, async () => {
      while (cursor < items.length) {
        const index = cursor++;
        results[index] = await worker(items[index], index);
      }
    })
  );
  return results;
}

export function publishedGraphPreparationConcurrency(env = process.env) {
  const raw = env.SOLID_CHECKER_GRAPH_CONCURRENCY;
  if (raw === undefined || raw === "") return 8;
  const value = Number(raw);
  if (!Number.isInteger(value) || value < 1) {
    throw new Error(
      `SOLID_CHECKER_GRAPH_CONCURRENCY must be a positive integer, got ${JSON.stringify(raw)}`
    );
  }
  return value;
}

async function preparePublishedGraphFallback({
  options,
  manifest,
  scratch,
  output,
  certificationImporter,
  rootArtifactSnapshot,
  fetch_
}) {
  if (!options.issuerConfiguration) {
    throw new CertificationRefusal({
      stage: "receipt-issuance",
      owner: "trust",
      reason: "no external policy-2 issuer configuration was provided"
    });
  }
  if (!options.trustConfigurationOutput) {
    throw new CertificationRefusal({
      stage: "catalog-publication",
      owner: "trust",
      reason: "no external policy-2 trust-configuration output was provided"
    });
  }
  const refusalPath = `${output}.refusals.json`;
  if (!existsSync(refusalPath)) throw new Error("proposal failure did not emit a refusal census");
  const audit = JSON.parse(readFileSync(refusalPath, "utf8"));
  const dependencyCases = (audit.refusals ?? []).filter(
    isExactDependencyCompositionRefusal
  );
  if (dependencyCases.length === 0) {
    throw new Error("proposal refusal has no exact dependency-composition case");
  }
  const bunLockPath = findBunLock(options.packageRoot);
  const bunLock = readFileSync(bunLockPath, "utf8");
  const bunLockIndex = createBunLockSelectionIndex(bunLock);
  const preparedCases = [];
  const demandPlans = [];
  // The artifact-case set is one acquisition transaction. Reuse a node only
  // when its complete resolver identity is identical; Rust still rebuilds and
  // authenticates each per-root graph before the reused bytes can authorize a
  // receipt.
  const byKey = new Map();
  const pendingByKey = new Map();
  const publishedArtifacts = new Map();
  const graphResolutionSession = new ArtifactResolutionSession();
  let nextNodeIndex = 0;
  const publishedArtifactKey = (nodeManifest, integrity) => JSON.stringify([
    options.registryOrigin,
    nodeManifest.name,
    nodeManifest.version,
    integrity
  ]);
  const acquireSharedPublishedArtifact = (
    nodeManifest,
    integrity,
    artifactScratch
  ) => {
    const key = publishedArtifactKey(nodeManifest, integrity);
    let acquisition = publishedArtifacts.get(key);
    if (!acquisition) {
      acquisition = acquirePublishedArtifact({
        options: { registryOrigin: options.registryOrigin, integrity },
        manifest: nodeManifest,
        scratch: artifactScratch,
        fetch_
      });
      publishedArtifacts.set(key, acquisition);
    }
    return acquisition;
  };
  publishedArtifacts.set(
    publishedArtifactKey(manifest, options.integrity),
    Promise.resolve(rootArtifactSnapshot)
  );

  const {
    locateExternalFrom,
    collectCompilerSources,
    sourceArtifacts,
    compilerSourceClosureCount
  } = createCompilerSourceCollector({
    bunLockPath,
    bunLockIndex,
    scratch,
    resolutionSession: graphResolutionSession,
    scratchPrefix: "graph"
  });

  const prepareArtifactCase = async (artifactCase, caseIndex) => {
    const prepareState = async (request, isRoot, conditions, key, nextAncestry) => {
      const resolved = resolvePackageArtifactClosure({
        importer: request.importer,
        specifier: request.specifier,
        packageRoot: request.packageRoot,
        conditions,
        resolutionKind: "import",
        integrity: request.integrity
      }, graphResolutionSession);
      const nodeManifest = JSON.parse(
        readFileSync(join(resolved.packageRoot, "package.json"), "utf8")
      );
      const lock = exactBunLockSelection(
        bunLockIndex,
        resolved.packageName,
        resolved.packageVersion,
        bunLockLocatorForInstalledPackage(bunLockPath, resolved.packageRoot)
      );
      if (lock.integrity !== request.integrity) {
        throw new Error(
          `Bun lock integrity for ${resolved.packageName}@${resolved.packageVersion} disagrees with acquisition`
        );
      }
      const node = {
        key,
        importer: resolve(request.importer),
        specifier: request.specifier,
        packageRoot: resolve(resolved.packageRoot),
        packageName: resolved.packageName,
        packageVersion: resolved.packageVersion,
        integrity: request.integrity,
        entrypoint: resolved.requestedEntrypoint,
        conditions,
        lockLocator: lock.locator,
        bunLockPath: resolve(bunLockPath),
        dependencies: []
      };
      const nodeIndex = nextNodeIndex++;
      const nodeScratch = join(scratch, `graph-${caseIndex}-node-${nodeIndex}`);
      mkdirSync(nodeScratch);
      const artifactSnapshot =
        isRoot ? rootArtifactSnapshot : null;
      const generatedOutput = join(nodeScratch, "solid-reactivity.json");
      const directDependencies = [];
      const locatedDependencies = new Map();
      const addSemanticDependency = async dependency => {
        if (dependency.specifier.startsWith("node:")) {
          throw new Error(
            `runtime-library-policy-required: ${dependency.specifier} is not a package receipt`
          );
        }
        const located = locateExternalFrom(node.packageRoot, dependency);
        const previous = locatedDependencies.get(dependency.specifier);
        if (previous && previous.dependencyRoot !== located.dependencyRoot) {
          throw new Error(
            `ambiguous dependency identity for ${dependency.specifier}: ${previous.dependencyRoot} and ${located.dependencyRoot}`
          );
        }
        if (previous) return false;
        locatedDependencies.set(dependency.specifier, located);
        const child = await prepareNode(
          {
            importer: located.dependencyImporter,
            specifier: dependency.specifier,
            packageRoot: located.dependencyRoot,
            conditions,
            integrity: located.dependencyLock.integrity
          },
          false,
          nextAncestry
        );
        node.dependencies.push({ specifier: dependency.specifier, node: child.node.key });
        directDependencies.push({ state: child, viaSpecifier: dependency.specifier });
        return true;
      };
      for (const dependency of resolved.externalDependencies.filter(
        dependency => dependency.axis === "runtime" && dependency.kind === "reexport"
      )) {
        await addSemanticDependency(dependency);
      }
      const sortDependencies = () => {
        node.dependencies.sort((left, right) =>
          left.specifier.localeCompare(right.specifier) || left.node.localeCompare(right.node)
        );
        directDependencies.sort((left, right) =>
          left.viaSpecifier.localeCompare(right.viaSpecifier) ||
          left.state.node.key.localeCompare(right.state.node.key)
        );
      };
      sortDependencies();
      const semanticRoots = new Set(
        directDependencies.map(dependency => resolve(dependency.state.node.packageRoot))
      );
      const ownSourceDependencies = [];
      for (const dependency of resolved.externalDependencies) {
        const located = locateExternalFrom(node.packageRoot, dependency);
        if (!located) continue;
        ownSourceDependencies.push(
          ...await collectCompilerSources(located, conditions, semanticRoots)
        );
      }
      const sourceDependencies = canonicalCompilerSources([
        ...ownSourceDependencies,
        ...directDependencies.flatMap(
          dependency => dependency.state.sourceDependencies ?? []
        )
      ]);
      const preparedState = {
        index: `${caseIndex}-${nodeIndex}`,
        node,
        nodeIndex,
        nodeManifest,
        artifactSnapshot,
        generatedOutput,
        directDependencies,
        planning: null,
        demandPlan: null,
        sourceDependencies,
        scratch: nodeScratch
      };
      byKey.set(node.key, preparedState);
      return preparedState;
    };
    const prepareNode = async (request, isRoot = false, ancestry = []) => {
      const conditions = [...new Set([...(request.conditions ?? []), "import"])].sort();
      const key = publishedGraphRequestKey({ ...request, conditions });
      if (ancestry.includes(key)) {
        throw new Error(`published dependency graph cycle: ${[...ancestry, key].join(" -> ")}`);
      }
      let state = byKey.get(key);
      if (!state) {
        let preparation = pendingByKey.get(key);
        if (!preparation) {
          if (byKey.size + pendingByKey.size >= 256 || ancestry.length > 64) {
            throw new Error("published dependency graph exceeds policy-2 node/depth limits");
          }
          const nextAncestry = [...ancestry, key];
          preparation = prepareState(request, isRoot, conditions, key, nextAncestry);
          pendingByKey.set(key, preparation);
        }
        try {
          state = await preparation;
        } finally {
          if (pendingByKey.get(key) === preparation) pendingByKey.delete(key);
        }
      }
      return state;
    };
    const root = await prepareNode(
      {
        importer: certificationImporter,
        specifier: rootSpecifier(manifest.name, artifactCase.entrypoint),
        packageRoot: options.packageRoot,
        conditions: artifactCase.conditions ?? [],
        integrity: options.integrity
      },
      true
    );
    return { root, nodes: reachableGraphStates(root, byKey) };
  };
  const prepared = await mapWithExactConcurrency(
    dependencyCases,
    publishedGraphPreparationConcurrency(),
    prepareArtifactCase
  );
  const sourceByKey = new Map();
  const acquisitionUnits = [
    ...[...byKey.values()]
      .filter(state => !state.artifactSnapshot)
      .map(state => ({ kind: "node", state })),
    ...[...sourceArtifacts.values()].map(source => ({ kind: "source", source }))
  ];
  const graphAcquisitionStartedAt = performance.now();
  await mapWithExactConcurrency(
    acquisitionUnits,
    publishedGraphPreparationConcurrency(),
    async unit => {
      if (unit.kind === "node") {
        unit.state.artifactSnapshot = await acquireSharedPublishedArtifact(
          unit.state.nodeManifest,
          unit.state.node.integrity,
          unit.state.scratch
        );
        return;
      }
      const artifact = await acquireSharedPublishedArtifact(
        unit.source.manifest,
        unit.source.integrity,
        unit.source.scratch
      );
      sourceByKey.set(unit.source.key, {
        packageName: unit.source.packageName,
        packageVersion: unit.source.packageVersion,
        registryOrigin: artifact.registryOrigin,
        registryMetadata: artifact.metadataPath,
        archive: artifact.archivePath,
        lockfile: unit.source.lockfile,
        lockLocator: unit.source.lockLocator,
        installedPackageRoot: unit.source.installedPackageRoot
      });
    }
  );
  for (const state of byKey.values()) {
    state.sourceDependencies = state.sourceDependencies.map(source => {
      const acquired = sourceByKey.get(source.key);
      if (!acquired) {
        throw new Error(`compiler source ${source.packageName}@${source.packageVersion} was not acquired`);
      }
      return acquired;
    });
  }
  const graphAcquisitionDurationMs =
    Math.round((performance.now() - graphAcquisitionStartedAt) * 100) / 100;
  const pendingGeneration = new Set(byKey.values());
  let proposalGenerations = 0;
  const proposalFrontiers = [];
  while (pendingGeneration.size > 0) {
    const ready = [...pendingGeneration]
      .filter(state => state.directDependencies.every(dependency => dependency.state.planning))
      .sort((left, right) => left.node.key.localeCompare(right.node.key));
    if (ready.length === 0) {
      throw new Error("published dependency graph has no dependency-first proposal frontier");
    }
    const frontierStartedAt = performance.now();
    await mapWithExactConcurrency(
      ready,
      publishedGraphPreparationConcurrency(),
      async state => {
        const dependencies = state.directDependencies.map(dependency => ({
          ...dependency.state,
          viaSpecifier: dependency.viaSpecifier
        }));
        const merged = mergeProposalDependencies(
          dependencies,
          join(state.scratch, `dependency-catalog-${dependencies.length}`)
        );
        const generationArguments = [
          "--package-root",
          state.node.packageRoot,
          "--output",
          state.generatedOutput,
          "--integrity",
          state.node.integrity,
          "--entrypoint",
          state.node.entrypoint,
          "--certification-importer",
          state.node.importer
        ];
        const explicitConditions = state.node.conditions.filter(
          condition => condition !== "import"
        );
        if (explicitConditions.length) {
          generationArguments.push("--conditions", explicitConditions.join(","));
        }
        const generated = await generatePackageContract(generationArguments, {
          quiet: true,
          proposalDependencies: merged.proposalDependencies,
          proposalDependencyCatalog: merged.catalog,
          privateGraphPreparation: true,
          exactConditions: explicitConditions
        });
        const plannings = certificationPlannings(generated, state.artifactSnapshot);
        if (plannings.length !== 1) {
          throw new Error(
            `exact graph node ${state.node.packageName}@${state.node.packageVersion} produced ${plannings.length} artifact cases`
          );
        }
        // This plan is diagnostic orchestration material only. The final
        // native case-set transaction independently decodes the proposal and
        // derives every authority-bearing demand.
        const reviewedPlan = reviewGraphProposal(generated);
        state.planning = plannings[0];
        state.demandPlan = reviewedPlan;
        demandPlans[state.nodeIndex] = reviewedPlan;
        proposalGenerations += 1;
      }
    );
    proposalFrontiers.push({
      nodes: ready.length,
      durationMs: Math.round((performance.now() - frontierStartedAt) * 100) / 100
    });
    for (const state of ready) pendingGeneration.delete(state);
  }
  preparedCases.push(...prepared);
  for (const item of preparedCases) {
    item.nodes = reachableGraphStates(item.root, byKey);
  }
  return {
    preparedCases,
    demandPlans: demandPlans.filter(Boolean),
    timing: {
      rootCases: preparedCases.length,
      canonicalNodes: byKey.size,
      acquiredPublishedArtifacts: publishedArtifacts.size,
      acquisitionUnits: acquisitionUnits.length,
      graphAcquisitionDurationMs,
      resolutionSession: graphResolutionSession.statistics(),
      compilerSourceClosureCensus: compilerSourceClosureCount(),
      proposalGenerations,
      proposalFrontiers,
      graphNodeReferences: preparedCases.reduce((total, item) => total + item.nodes.length, 0),
      nativeCertificationTransactions: 1,
      typeFactsCaseSetBatches: 1
    }
  };
}

/// Acquires the declaration-only closure an ordinary root certification needs
/// so its witness program can resolve cross-package type references.
///
/// Contract *generation* resolves `Accessor`, `Component`, `JSX.Element` and
/// every other cross-package reference against the installed tree. Certification
/// deliberately replays in a private project built only from authenticated
/// bytes, so without this the same references resolve to `any` and the producer
/// correctly fail-closes their callability to Unknown. This supplies the missing
/// evidence through the one authenticated channel — registry archives replayed
/// against exact lock selections — and never through the installed tree.
///
/// Failure is a non-event, but it is **name-scoped and all-or-nothing**. A
/// missing lockfile, a copy the lock does not select exactly, a package that is
/// not installed, declarations that do not resolve, a registry that will not
/// serve the archive — any of these poisons the whole *package name*, and every
/// copy of that name is then withheld.
///
/// Dropping one copy while another copy of the same name survives is not
/// evidence removal, it is evidence substitution: `moduleResolution: "bundler"`
/// walks up `node_modules`, so a nested copy that is withheld silently hands the
/// lookup to a hoisted copy at a *different version*, and the census accepts
/// those bytes because they are authentic under their own marker. A verdict can
/// flip that way. Withholding the whole name is the only drop that really does
/// mean "cannot resolve": TypeScript then reports the module as missing and the
/// reference is `any`, exactly as when nothing is supplied.
///
/// The exclusion is deliberately global across certification inputs, because the
/// private project is materialized once from their union — a name one input
/// could not authenticate must not reach any of them.
///
/// Returns one array of acquired sources per certification input, positionally.
export async function acquireRootCompilerSources({ options, generated, scratch, fetch_ }) {
  const empty = generated.certificationInputs.map(() => []);
  let bunLockPath;
  try {
    bunLockPath = findBunLock(options.packageRoot);
  } catch {
    return empty;
  }
  let bunLockIndex;
  try {
    bunLockIndex = createBunLockSelectionIndex(readFileSync(bunLockPath, "utf8"));
  } catch {
    return empty;
  }
  const sourceScratch = join(scratch, "root-sources");
  mkdirSync(sourceScratch, { recursive: true });
  const withheldNames = new Set();
  const collector = createCompilerSourceCollector({
    bunLockPath,
    bunLockIndex,
    scratch: sourceScratch,
    resolutionSession: new ArtifactResolutionSession(),
    scratchPrefix: "root",
    onUnnameable: name => withheldNames.add(name)
  });
  const perInput = [];
  for (const input of generated.certificationInputs) {
    const conditions = [...new Set([...(input.conditions ?? []), "import"])].sort();
    let resolved;
    try {
      resolved = resolvePackageArtifactClosure({
        importer: input.resolution.importer,
        specifier: input.resolution.specifier,
        packageRoot: input.resolution.packageRoot ?? options.packageRoot,
        conditions,
        resolutionKind: "import",
        integrity: options.integrity
      }, null);
    } catch {
      perInput.push([]);
      continue;
    }
    const found = [];
    for (const dependency of resolved.externalDependencies) {
      try {
        const located = collector.locateExternalFrom(resolved.packageRoot, dependency);
        if (!located) continue;
        found.push(...await collector.collectCompilerSources(located, conditions, new Set()));
      } catch {
        withheldNames.add(packageNameOfSpecifier(dependency.specifier));
      }
    }
    perInput.push(canonicalCompilerSources(found));
  }
  const acquiredByKey = new Map();
  // Each source costs two registry round trips (packument, then archive) and
  // a wide-surface root names dozens of them; acquiring them one after another
  // made registry latency, not analysis, the dominant certification cost.
  // Every acquisition is keyed by exact identity, so completion order does
  // not affect which sources are named.
  await mapWithExactConcurrency(
    [...new Set(perInput.flat())],
    registryAcquisitionConcurrency(),
    async source => {
      try {
        const artifact = await acquirePublishedArtifact({
          options: { registryOrigin: options.registryOrigin, integrity: source.integrity },
          manifest: source.manifest,
          scratch: source.scratch,
          fetch_
        });
        acquiredByKey.set(source.key, {
          packageName: source.packageName,
          packageVersion: source.packageVersion,
          registryOrigin: artifact.registryOrigin,
          registryMetadata: artifact.metadataPath,
          archive: artifact.archivePath,
          lockfile: source.lockfile,
          lockLocator: source.lockLocator,
          installedPackageRoot: source.installedPackageRoot
        });
      } catch {
        withheldNames.add(source.packageName);
      }
    }
  );
  return perInput.map(sources =>
    sources
      .map(source => acquiredByKey.get(source.key))
      .filter(acquired => acquired && !withheldNames.has(acquired.packageName))
  );
}

async function executeNativeCertification({
  options,
  generated,
  artifactSnapshot,
  scratch,
  fetch_
}) {
  if (!options.issuerConfiguration) {
    throw new CertificationRefusal({
      stage: "receipt-issuance",
      owner: "trust",
      reason: "no external policy-2 issuer configuration was provided"
    });
  }
  if (!options.trustConfigurationOutput) {
    throw new CertificationRefusal({
      stage: "catalog-publication",
      owner: "trust",
      reason: "no external policy-2 trust-configuration output was provided"
    });
  }
  if (generated.certificationInputs.length === 0) {
    throw new CertificationRefusal({
      stage: "certification",
      owner: "certifier",
      reason: "the value-only transaction has no selected artifact case"
    });
  }
  const typefactsExecutable = process.env.SOLID_TYPEFACTS_BIN;
  if (!typefactsExecutable) {
    throw new CertificationRefusal({
      stage: "witness-acquisition",
      owner: "type-facts",
      reason: "SOLID_TYPEFACTS_BIN does not name the pinned Type Facts producer"
    });
  }
  const catalogRoot = options.catalog.endsWith("accepted-contracts.json")
    ? dirname(options.catalog)
    : options.catalog;
  const requestPath = join(scratch, "certification-execution.json");
  const sourceDependenciesByInput = await acquireRootCompilerSources({
    options,
    generated,
    scratch,
    fetch_
  });
  const plannings = generated.certificationInputs.map((input, index) => ({
      schemaVersion: 1,
      proposal: generated.output,
      resolution: input.resolution,
      exportConditions: [...new Set([...input.conditions, "import"])].sort(),
      registryOrigin: artifactSnapshot.registryOrigin,
      registryMetadata: artifactSnapshot.metadataPath,
      archive: artifactSnapshot.archivePath,
      sourceDependencies: sourceDependenciesByInput[index] ?? []
  }));
  const execution = {
    schemaVersion: plannings.length === 1 ? 1 : 2,
    ...(plannings.length === 1 ? { planning: plannings[0] } : { plannings }),
    typefactsExecutable: resolve(typefactsExecutable),
    issuerConfiguration: resolve(options.issuerConfiguration),
    catalogRoot,
    trustConfigurationOutput: resolve(options.trustConfigurationOutput)
  };
  writeFileSync(requestPath, `${JSON.stringify(execution, null, 2)}\n`);
  const child = await runNativeAsync(
    "solid-checker",
    ["--execute-contract-certification", requestPath],
    { cwd: options.packageRoot, env: { SOLID_CHECKER_DAEMON: "0" } }
  );
  if (child.error) throw new Error(`could not start the native checker: ${child.error.message}`);
  if (child.status !== 0) {
    throw new CertificationRefusal({
      stage: "witness-acquisition",
      owner: "certifier",
      reason: child.stderr.trim() || child.stdout.trim() || `native checker exited ${child.status}`
    });
  }
  return { authority: "native-certification-complete", catalogRoot };
}

async function executeNativeOrGraphCertification({
  options,
  generated,
  graph,
  artifactSnapshot,
  scratch,
  fetch_
}) {
  if (!graph) {
    return executeNativeCertification({
      options,
      generated,
      artifactSnapshot,
      scratch,
      fetch_
    });
  }
  const catalogRoot = options.catalog.endsWith("accepted-contracts.json")
    ? dirname(options.catalog)
    : options.catalog;
  return executePreparedPublishedGraphs({
    options,
    cases: graph.preparedCases,
    scratch,
    catalogRoot,
    trustConfigurationOutput: options.trustConfigurationOutput
  });
}

function unavailableWitnessRefusal(plans) {
  const demands = plans.flatMap(plan => plan.demands ?? []);
  const missing = demands
    .filter(demand => !demand.satisfiedByArtifactSnapshot)
    .map(demand => ({
      demandId: demand.id,
      family: demand.family,
      owner: demand.owner,
      reason:
        demand.owner === "probe-gate"
          ? "the mandatory policy-2 probe harness and runtime-image binding are unavailable"
          : `the automatic ${demand.owner} witness adapter is unavailable`
    }));
  if (missing.length === 0) {
    throw new CertificationRefusal({
      stage: "receipt-issuance",
      owner: "trust",
      reason: "no configured policy-2 receipt issuer is available"
    });
  }
  const first = missing[0];
  throw new CertificationRefusal({
    stage: "witness-acquisition",
    owner: first.owner,
    demandId: first.demandId,
    family: first.family,
    reason: first.reason,
    refusals: missing
  });
}

function writeAudit(
  path,
  manifest,
  refusal,
  demandPlans,
  stageDurationsMs,
  graphPreparation = null
) {
  if (!path) return;
  const output = resolve(path);
  mkdirSync(dirname(output), { recursive: true });
  writeFileSync(
    output,
    `${JSON.stringify(
      {
        format: "solid-checker-contract-certification-audit",
        auditVersion: 1,
        authoritative: false,
        replayable: false,
        status: "refused",
        package: { name: manifest.name, version: manifest.version },
        stage: refusal.stage ?? "orchestration",
        refusal: {
          owner: refusal.owner ?? "orchestration",
          demandId: refusal.demandId ?? null,
          family: refusal.family ?? null,
          reason: refusal.reason ?? refusal.message
        },
        stageDurationsMs,
        graphPreparation,
        refusals: refusal.refusals ?? [],
        demandPlans: demandPlans.map(plan => ({
          policyDigest: plan.policyDigest,
          candidateSemanticDigest: plan.candidateSemanticDigest,
          snapshotRoot: plan.snapshotRoot,
          provenanceRoot: plan.provenanceRoot,
          demandGraphRoot: plan.demandGraphRoot,
          demands: plan.demands
        }))
      },
      null,
      2
    )}\n`
  );
}

function writeSuccessAudit(
  path,
  manifest,
  demandPlans,
  stageDurationsMs,
  graphPreparation = null
) {
  if (!path) return;
  const output = resolve(path);
  mkdirSync(dirname(output), { recursive: true });
  writeFileSync(output, `${JSON.stringify({
    format: "solid-checker-contract-certification-audit",
    auditVersion: 1,
    authoritative: false,
    replayable: false,
    status: "certified",
    package: { name: manifest.name, version: manifest.version },
    stage: "catalog-publication",
    ordinaryAnalysis: { receiptAuthenticated: true, exactCaseSelected: true },
    stageDurationsMs,
    graphPreparation,
    refusals: [],
    demandPlans: demandPlans.map(plan => ({
      policyDigest: plan.policyDigest,
      candidateSemanticDigest: plan.candidateSemanticDigest,
      snapshotRoot: plan.snapshotRoot,
      provenanceRoot: plan.provenanceRoot,
      demandGraphRoot: plan.demandGraphRoot,
      demands: plan.demands
    }))
  }, null, 2)}\n`);
}

/// Reads the sidecars beside `options.proposal` and, when
/// `reusableProposalInputs` admits them, copies document, plan and refusal
/// audit into the certification scratch under the names an in-process
/// generation would have produced. The copies are made from the exact bytes
/// that were validated, never re-read from the reusable path.
function reuseEmittedProposal({ options, manifest, certificationImporter, proposalOutput }) {
  let inputs;
  let documentBytes;
  let planBytes;
  try {
    documentBytes = readFileSync(options.proposal);
    planBytes = readFileSync(`${options.proposal}.proposal.json`);
    inputs = JSON.parse(readFileSync(`${options.proposal}.certification-inputs.json`, "utf8"));
  } catch {
    return null;
  }
  const admitted = reusableProposalInputs({
    inputs,
    documentBytes,
    planBytes,
    manifest,
    packageRoot: options.packageRoot,
    integrity: options.integrity,
    certificationImporter,
    entrypoints: options.entrypoints,
    conditions: options.conditions
  });
  if (!admitted) return null;
  writeFileSync(proposalOutput, documentBytes);
  writeFileSync(`${proposalOutput}.proposal.json`, planBytes);
  try {
    writeFileSync(
      `${proposalOutput}.refusals.json`,
      readFileSync(`${options.proposal}.refusals.json`)
    );
  } catch {
    // The refusal census is a separate, separately validated reuse.
  }
  return {
    package: manifest.name,
    version: manifest.version,
    output: proposalOutput,
    plan: `${proposalOutput}.proposal.json`,
    schemaVersion: 1,
    certificationInputs: admitted.certificationInputs,
    accepted: false
  };
}

export async function certifyContract(arguments_, { fetch_ = fetch } = {}) {
  const options = parseCertifyArguments(arguments_);
  if (options.help) {
    process.stdout.write(contractCertifyHelp);
    return;
  }
  options.packageRoot = realpathSync(resolve(options.packageRoot));
  options.catalog = resolve(
    options.catalog || join(options.packageRoot, ".solid-checker", "accepted-contracts.json")
  );
  if (options.issuerConfiguration) options.issuerConfiguration = resolve(options.issuerConfiguration);
  if (options.trustConfigurationOutput) {
    options.trustConfigurationOutput = resolve(options.trustConfigurationOutput);
  }
  if (options.proposalRefusalAudit) {
    options.proposalRefusalAudit = resolve(options.proposalRefusalAudit);
  }
  if (options.proposal) options.proposal = resolve(options.proposal);
  const manifest = JSON.parse(readFileSync(join(options.packageRoot, "package.json"), "utf8"));
  if (!manifest.name || !manifest.version) {
    throw new Error("package.json must declare an exact package name and version");
  }
  const scratch = mkdtempSync(join(tmpdir(), "solid-checker-certify-"));
  const importerIdentity = createHash("sha256")
    .update("solid-checker:certification-importer:v1\0")
    .update(options.packageRoot)
    .update("\0")
    .update(options.catalog)
    .digest("hex");
  const certificationImporterPath = join(
    dirname(options.packageRoot),
    `.solid-checker-certification-${importerIdentity}.mjs`
  );
  let createdCertificationImporter = false;
  try {
    writeFileSync(certificationImporterPath, "export {};\n", { flag: "wx", mode: 0o600 });
    createdCertificationImporter = true;
  } catch (error) {
    if (
      error?.code !== "EEXIST" ||
      readFileSync(certificationImporterPath, "utf8") !== "export {};\n"
    ) {
      throw error;
    }
  }
  const certificationImporter = realpathSync(certificationImporterPath);
  const demandPlans = [];
  let graphPreparation = null;
  let reusedProposal = false;
  const stageDurationsMs = {};
  let certified = false;
  const measure = async (stage, operation) => {
    const started = process.hrtime.bigint();
    try {
      return await operation();
    } finally {
      stageDurationsMs[stage] = Math.round(
        Number(process.hrtime.bigint() - started) / 10_000
      ) / 100;
    }
  };
  try {
    await runContractCertificationPipeline({
      request: options,
      acquisition: {
        acquireArtifacts: async () =>
          measure("artifactAcquisition", () =>
            acquirePublishedArtifact({ options, manifest, scratch, fetch_ })
          )
      },
      proposal: {
        generate: async ({ artifactSnapshot }) => measure("proposalGeneration", async () => {
          const proposalOutput = join(scratch, "solid-reactivity.json");
          const generationArguments = [
            "--package-root",
            options.packageRoot,
            "--output",
            proposalOutput,
            "--integrity",
            options.integrity,
            "--certification-importer",
            certificationImporter,
            ...options.entrypoints.flatMap(entrypoint => ["--entrypoint", entrypoint])
          ];
          if (options.conditions.length) {
            generationArguments.push("--conditions", options.conditions.join(","));
          }
          if (options.proposalRefusalAudit) {
            // Retain the exact bytes that were parsed and validated. Re-reading
            // the path for the scratch copy would let a concurrent replacement
            // substitute a different, incomplete root census after validation.
            const existingAuditBytes = validatedReusableDependencyRefusalAuditBytes({
              auditBytes: readFileSync(options.proposalRefusalAudit),
              manifest,
              packageRoot: options.packageRoot,
              integrity: options.integrity,
              certificationImporter,
              entrypoints: options.entrypoints,
              conditions: options.conditions
            });
            if (existingAuditBytes) {
              // The authenticated archive has already been acquired by the
              // previous pipeline stage. This copy is only a complete root
              // census for untrusted graph preparation; the final native
              // transaction still reconstructs every proposal and graph.
              writeFileSync(`${proposalOutput}.refusals.json`, existingAuditBytes);
              const graph = await preparePublishedGraphFallback({
                options,
                manifest,
                scratch,
                output: proposalOutput,
                certificationImporter,
                rootArtifactSnapshot: artifactSnapshot,
                fetch_
              });
              graphPreparation = {
                ...graph.timing,
                reusedProposalRefusalAudit: true
              };
              return { authority: "rust", generated: null, graph };
            }
          }
          const reused = options.proposal
            ? reuseEmittedProposal({ options, manifest, certificationImporter, proposalOutput })
            : null;
          if (reused) {
            reusedProposal = true;
            return { authority: "rust", generated: reused, graph: null };
          }
          try {
            const generated = await generatePackageContract(generationArguments, { quiet: true });
            return { authority: "rust", generated, graph: null };
          } catch (rootFailure) {
            try {
              const graph = await preparePublishedGraphFallback({
                options,
                manifest,
                scratch,
                output: proposalOutput,
                certificationImporter,
                rootArtifactSnapshot: artifactSnapshot,
                fetch_
              });
              graphPreparation = graph.timing;
              return { authority: "rust", generated: null, graph };
            } catch (graphFailure) {
              if (
                graphFailure instanceof Error &&
                graphFailure.message === "proposal refusal has no exact dependency-composition case"
              ) {
                throw rootFailure;
              }
              throw graphFailure;
            }
          }
        })
      },
      rust: {
        planDemands: async ({ artifactSnapshot, openProposal }) =>
          measure("demandPlanning", async () => {
          const plans = openProposal.graph
            ? openProposal.graph.demandPlans
            : await planDemands({
                options,
                generated: openProposal.generated,
                artifactSnapshot,
                scratch
              });
          demandPlans.push(...plans);
          return { authority: "rust", plans };
        }),
        certify: async ({ witnesses }) => measure("certification", async () => {
          requireProduct(witnesses, "certification", "native-certification-complete");
          return { authority: "rust", witnesses };
        })
      },
      evidence: {
        obtainWitnesses: async ({ artifactSnapshot, openProposal }) =>
          measure("witnessAcquisition", () => executeNativeOrGraphCertification({
            options,
            generated: openProposal.generated,
            graph: openProposal.graph,
            artifactSnapshot,
            scratch,
            fetch_
          }))
      },
      issuer: {
        issue: async ({ accepted }) => measure("receiptIssuance", async () => ({
          authority: "configured-issuer",
          accepted
        }))
      },
      publication: {
        commit: async ({ receipt }) => measure("catalogPublication", async () => receipt)
      }
    });
    writeSuccessAudit(
      options.auditOutput,
      manifest,
      demandPlans,
      stageDurationsMs,
      reusedProposal ? { ...(graphPreparation ?? {}), reusedProposal: true } : graphPreparation
    );
    certified = true;
  } catch (error) {
    const refusal =
      error instanceof CertificationRefusal
        ? error
        : new CertificationRefusal({
            stage: demandPlans.length ? "witness-acquisition" : "artifact-or-demand-planning",
            owner: demandPlans.length ? "certifier" : "artifact-provenance",
            reason: error instanceof Error ? error.message : String(error)
          });
    writeAudit(
      options.auditOutput,
      manifest,
      refusal,
      demandPlans,
      stageDurationsMs,
      graphPreparation
    );
    throw refusal;
  } finally {
    rmSync(scratch, { recursive: true, force: true });
    if (!certified && createdCertificationImporter) {
      rmSync(certificationImporterPath, { force: true });
    }
  }
}
