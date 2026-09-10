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
  statSync,
  writeFileSync
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import process from "node:process";
import { inspectRetainedCertificationFloor } from "./retained-certification-floor.mjs";
import { fileURLToPath } from "node:url";
import {
  RECOVERY_GRAPH_CASE_BUDGET,
  recoveryGraphBudgetRefusal,
  selectRecoveryPreparation,
  retainedProposalGraphCases,
  certifyRetainedProposalSelection
} from "./retained-proposal-graphs.mjs";
export { RECOVERY_GRAPH_CASE_BUDGET, recoveryGraphBudgetRefusal };

/// The repository/package root the runtime-probe harness source manifest is
/// recomputed against. This file lives at `<root>/packages/cli/scripts/`, and
/// the manifest names its members relative to `<root>`.
const harnessRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");

import { runNativeAsync } from "../bin/launcher.mjs";

// The native checker's `SOLID_CHECKER_TIMINGS` lines go to its stderr, which
// the launcher pipes and a successful exit would otherwise drop. Forward them
// when timings were asked for, so a slow certification is attributable from
// the CLI the same way it is from the binary.
function forwardNativeTimings(child) {
  if (!process.env.SOLID_CHECKER_TIMINGS || !child.stderr) return;
  process.stderr.write(child.stderr);
}
import {
  ArtifactResolutionError,
  ArtifactResolutionSession,
  locateExternalDependencyPackageRoot,
  resolvePackageArtifactClosure
} from "./artifact-resolution.mjs";
export { locateExternalDependencyPackageRoot } from "./artifact-resolution.mjs";
import {
  CERTIFICATION_INPUTS_FORMAT,
  REFUSAL_CLASSES,
  artifactCaseDisposition,
  declaredApplicabilityClaims,
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
  --dependency-graph-lane When the root proposal generates only partially and
                          its own refusal census names exact
                          dependency-composition cases, certify those cases
                          through the published-dependency-graph lane instead of
                          publishing the partial proposal without them. The two
                          lanes describe different case sets -- the graph lane
                          covers exactly the refused cases, the partial proposal
                          exactly the others -- so this is a choice, not a
                          strict improvement, and it is off by default. Also
                          retries a single generated case after an exact native
                          callback-flow refusal, only in a fresh catalog
  --recover-entrypoints  Re-certify generated cases together with exact refused
                          dependency-composition cases. For a new value-only
                          publication, isolate proof-refused artifact cases and
                          re-certify the successful subset (at most 1024 cases).
                          Never shrinks an existing publication or reuses trial
                          receipts. Unproved cases retain explicit refusals
  --probe-recipe-corpus <DIR>
                          Hand-authored, claim-addressed runtime-probe recipes
                          (a directory holding recipes.json plus its modules).
                          Required only when a proposal closes a claim domain:
                          such a claim schedules a mandatory contradiction veto
                          that has to be executed, and without a corpus that
                          gate refuses instead of certifying an unvetoed
                          closure. Claim ids are content digests, so the way to
                          learn one is to certify once and read the refusal,
                          which names the exact claim that has no recipe. The
                          corpus may not live inside the analyzed package
  --audit-output <FILE>   Write a non-replayable diagnostic transcript
  -h, --help              Show this help

Environment:
  SOLID_CHECKER_PROBE_NODE <FILE>
                          Real path of the Node executable the probe harness is
                          launched with. It must be the path this verifier
                          build pinned (make PROBE_NODE=...), and it must be
                          a real path: the adapter refuses a symlink, because a
                          symlink is a name that can be repointed at other
                          bytes after the pin was taken. Defaults to the
                          "node" on PATH
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

/// Whether this census row says "this case needs an accepted contract for a
/// dependency that was not in scope".
///
/// Decided from the structured class the generator records at the moment it
/// builds the row (`artifactRefusalClass` in generate-package-contract.mjs),
/// which is derived from the error's own code or the native emitter's
/// machine-readable marker line -- never from the sentence a human reads.
///
/// The regex is a **legacy fallback only**, for an audit sidecar written before
/// the `class` field existed (a checked-in benchmark artifact, or a proposal
/// generated by an older CLI and handed over with `--proposal-refusal-audit`).
/// A row that carries a class is answered by the class alone: a reason that
/// happens to quote one of these phrases must not override a row the generator
/// classified as a fact about the publisher's own bytes.
export function isExactDependencyCompositionRefusal(refusal) {
  if (typeof refusal?.class === "string" && refusal.class) {
    return refusal.class === REFUSAL_CLASSES.DependencyComposition;
  }
  return /external export-all|unaccepted external dependency|accepted dependency contract|accepted dependency .* exact .* binding/i.test(
    refusal?.reason ?? ""
  );
}

/// Whether a proposal that generated *partially* has a frontier the
/// published-dependency-graph lane is the answer for.
///
/// The distinction the routing rests on: a dependency-composition refusal says
/// "this case needs an accepted contract for a dependency that was not in
/// scope", which the graph lane supplies by acquiring, generating and
/// certifying that dependency first. Every other refusal class -- a `.cjs`
/// entrypoint with no declaration target, a `.d.ts` naming itself as its own
/// fact source, an entry file whose runtime/declaration export sets do not
/// intersect -- is a fact about the *publisher's own bytes*, and no dependency
/// catalog changes it. Reusing the emitted proposal is the right answer for
/// those, and re-deciding the lane for them would only lose the cases that did
/// generate.
///
/// Takes the refusal rows, so the caller may read them from a census sidecar
/// (the CLI) or from an already-measured result (a harness). Each row is
/// classified by `isExactDependencyCompositionRefusal`, which reads the
/// structured class the generator recorded and falls back to prose only for a
/// census written before that field existed.
export function partialProposalHasDependencyFrontier(refusals) {
  return (
    Array.isArray(refusals) &&
    refusals.length > 0 &&
    refusals.some(isExactDependencyCompositionRefusal)
  );
}

/// The demand and family a native certifier refusal names, so the audit
/// sidecar attributes it instead of recording `demandId: null, family: null`
/// for every semantic refusal alike.
///
/// This reads only the two shapes the native certifier's own error types
/// produce -- `Type Facts demand <id> is unsupported: ... (family=<f>)` and
/// `Type Facts demand <id> is locally open: <f> (<artifact-case>:<export>):
/// ...`. Anything else stays unattributed: a reason this cannot parse is
/// reported exactly as it was, never guessed at, and the sidecar's nulls are
/// the honest answer for it.
export function nativeRefusalAttribution(reason) {
  const text = typeof reason === "string" ? reason : "";
  const demand = /Type Facts demand (sha256:[0-9a-f]{64})\b/.exec(text);
  const declared = /\(family=([a-z][a-z0-9-]*)\)/.exec(text);
  const open = / is locally open: ([a-z][a-z0-9-]*) \(/.exec(text);
  return {
    demandId: demand?.[1] ?? null,
    family: declared?.[1] ?? open?.[1] ?? null
  };
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

/// Refuses an artifact case whose **own runtime module graph** imports a
/// subpath that the installed dependency's `exports` map provably excludes.
///
/// Returns nothing and throws a `CertificationRefusal` when it fires. Three
/// premises, all of which must hold, and each of which one measured row taught:
///
///  1. **The edge is the case's own runtime import.** `axis === "runtime"` on
///     an edge of the case's own closure. The declaration-axis walk is a
///     *type* graph that no runtime resolves — a `.d.ts` naming
///     `jiti/lib/types` is erased — and a specifier belonging to a transitive
///     source package is that package's import, not this case's. Firing on
///     those attributed a grandchild's specifier to the root's artifact case.
///  2. **The dependency declares an `exports` map.** A package with no
///     `exports` does not restrict its subpaths at all: Node applies
///     PACKAGE_EXPORTS_RESOLVE only when the field is present, and otherwise
///     resolves `pkg/sub` as a legacy path. `dayjs@1.11.23`,
///     `picomatch@2.3.2` and `fetch-blob@3.2.0` ship no `exports` and all
///     three publish the requested file. (`artifact-resolution.mjs` no longer
///     answers `not-exported` for them either; this is the second gate on the
///     same mistake, because the claim is about the map and must not be
///     inferable from its absence.)
///  3. **The map, replayed under this run's conditions, answers
///     `not-exported`.** Not "the file is missing" (`target-not-found`) and
///     not "no condition matched" (`conditions-unmatched`, which stays the
///     unresolved-dependency frontier). A `"./*"` or other pattern key that
///     matches the request is a match, so a wildcard map never answers this.
///     The legacy `browser` field cannot rescue it: Node ignores `browser`
///     whenever `exports` is present, and it only substitutes one file for
///     another rather than adding a subpath. `imports` (`#specifier`) is a
///     different resolution that no package specifier reaches.
///
/// What is left when all three hold is a broken import: the package is
/// present, its manifest is the published one, an exact Bun lock selection
/// names that copy, and its own export map publishes nothing for the module
/// the case imports at runtime. `@solid-primitives/favicon`,
/// `@solid-primitives/drag-drop` and `@tanstack/solid-query-devtools` all
/// import `solid-js/web`, which Solid 2 retired.
function refuseCaseImportingUnexportedTarget({
  entrypoint,
  conditions,
  dependency,
  located,
  resolutionSession
}) {
  if (dependency.axis !== "runtime") return;
  if (located.dependencyManifest.exports === undefined) return;
  try {
    resolvePackageArtifactClosure(
      {
        importer: located.dependencyImporter,
        specifier: located.specifier,
        packageRoot: located.dependencyRoot,
        conditions,
        resolutionKind: "import",
        integrity: located.dependencyLock.integrity
      },
      resolutionSession
    );
  } catch (error) {
    if (!(error instanceof ArtifactResolutionError) || error.code !== "not-exported") return;
    throw new CertificationRefusal({
      stage: "artifact-case",
      owner: "certifier",
      reason:
        `artifact case ${entrypoint} [${conditions.join(", ")}] imports ` +
        `dependency-target-not-exported: ${located.specifier} is not exported by ` +
        `${located.dependencyManifest.name}@${located.dependencyManifest.version} ` +
        `under conditions [${conditions.join(", ")}]`
    });
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
    dependencyGraphLane: false,
    recoverEntrypoints: false,
    probeRecipeCorpus: "",
    auditOutput: "",
    issuerConfiguration: process.env.SOLID_CHECKER_POLICY2_ISSUER_CONFIG ?? "",
    trustConfigurationOutput: process.env.SOLID_CHECKER_POLICY2_TRUST_CONFIG ?? "",
    help: false
  };
  for (let index = 0; index < arguments_.length; index += 1) {
    const argument = arguments_[index];
    if (["--help", "-h"].includes(argument)) return { ...options, help: true };
    // The one valueless option, so it is read before the value extraction the
    // rest of the parser requires.
    if (argument === "--dependency-graph-lane") {
      options.dependencyGraphLane = true;
      continue;
    }
    if (argument === "--recover-entrypoints") {
      options.recoverEntrypoints = true;
      continue;
    }
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
    else if (key === "--probe-recipe-corpus") options.probeRecipeCorpus = value;
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

function certificationPlannings(generated, artifactSnapshot, options = null) {
  return generated.certificationInputs.map(input => ({
    schemaVersion: 1,
    proposal: generated.output,
    resolution: input.resolution,
    exportConditions: [...new Set([...input.conditions, "import"])].sort(),
    registryOrigin: artifactSnapshot.registryOrigin,
    registryMetadata: artifactSnapshot.metadataPath,
    archive: artifactSnapshot.archivePath,
    // Every planning carries the whole declared applicability census, not a
    // share of it: each one is an independent native transaction, and a case
    // the proposal omitted must be re-proved by whichever transaction accepts
    // that proposal.
    inapplicableCases: generated.inapplicableCases ?? [],
    // The recipe corpus the execution will be handed, so the reviewed plan is
    // the recipe-gated plan the execution certifies against: a `creates`
    // closure candidate with no recipe is withheld by name at planning, and
    // the plan names neither its demand nor its gate.
    ...(options?.probeRecipeCorpus
      ? { probeRecipeCorpus: resolve(options.probeRecipeCorpus) }
      : {})
  }));
}

/// The closure candidates the native transaction withheld by name, read off
/// its stdout. One `solid-checker:withheld-closure=<json>` line per candidate
/// (`main.rs`'s `report_withheld_closures`); anything else on stdout is not a
/// record and is left alone. Audit material only: the accepted contract itself
/// already says the withheld domains are open.
export const WITHHELD_CLOSURE_MARKER = "solid-checker:withheld-closure=";

export function withheldClosuresFromNativeOutput(stdout) {
  const records = [];
  for (const line of String(stdout ?? "").split("\n")) {
    const trimmed = line.trim();
    if (!trimmed.startsWith(WITHHELD_CLOSURE_MARKER)) continue;
    try {
      const record = JSON.parse(trimmed.slice(WITHHELD_CLOSURE_MARKER.length));
      if (
        record &&
        typeof record === "object" &&
        typeof record.export === "string" &&
        typeof record.domain === "string" &&
        typeof record.reason === "string"
      ) {
        records.push(record);
      }
    } catch {
      // A malformed line is not a record; the honest answer is to omit it
      // rather than to invent fields for it.
    }
  }
  return records;
}

async function planDemands({ options, generated, artifactSnapshot, scratch }) {
  const plans = [];
  for (const [index, planning] of certificationPlannings(
    generated,
    artifactSnapshot,
    options
  ).entries()) {
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
  forwardNativeTimings(child);
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
  if (artifactCases.size === 0) {
    // An inert module has no semantic demands, so its native proposal plan has
    // no demand-bearing artifactCase field to review. The dependency catalog
    // still needs the exact case identity for the later graph replay. Derive
    // it from the one inert wire case with the same length-prefixed inputs as
    // contract_document::artifact_case_id; any other empty plan remains a
    // refusal rather than an inferred dependency identity.
    const proposal = JSON.parse(readFileSync(generated.output, "utf8"));
    const cases = Object.entries(proposal.entrypoints ?? {}).flatMap(
      ([entrypoint, value]) => (value?.cases ?? []).map(case_ => ({ entrypoint, case_ }))
    );
    if (
      cases.length === 1 &&
      cases[0].case_?.initialization === "inert" &&
      Object.keys(cases[0].case_?.exports ?? {}).length === 0
    ) {
      const { entrypoint, case_: case_ } = cases[0];
      const hash = createHash("sha256");
      const text = value => {
        const bytes = Buffer.from(value, "utf8");
        const length = Buffer.alloc(8);
        length.writeBigUInt64BE(BigInt(bytes.length));
        hash.update(length);
        hash.update(bytes);
      };
      const digest = value => `sha256:${String(value).replace(/^sha256:/i, "").toLowerCase()}`;
      const artifact = value => {
        text(value.path);
        text(digest(value.sha256));
      };
      hash.update("solid-checker:artifact-case:v1");
      text(entrypoint);
      if (!case_.resolution || typeof case_.resolution.runtimeBranch !== "string" ||
          typeof case_.resolution.typesBranch !== "string") {
        throw new Error("inert graph proposal is missing its exact resolution trace");
      }
      text("runtime");
      text(case_.resolution.runtimeBranch);
      text("types");
      text(case_.resolution.typesBranch);
      artifact(case_.artifact);
      artifact(case_.declarations);
      text(digest(case_.artifact.closureSha256));
      hash.update(Buffer.from([0]));
      artifactCases.add(`artifact-case:${hash.digest("hex")}`);
    }
  }
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
      // A specifier the located package does not resolve is a fact about that
      // *specifier*, not about the package's bytes. The package's own
      // published declarations are exactly what they were, so they are still
      // supplied and only the closure *behind* the failed specifier is
      // dropped: the private project then reports that one module missing,
      // which is what it is.
      //
      // Letting the failure escape instead unnames the whole package, because
      // both callers answer a throw by withholding
      // `packageNameOfSpecifier(specifier)` — and the name a subpath
      // specifier carries is the name of a package that authenticated
      // perfectly. `@solid-primitives/favicon`'s compiled output imports
      // `solid-js/web`, which Solid 2 no longer exports; withholding
      // `solid-js` for it deleted every Solid declaration from the witness
      // program, collapsed `Component<Props>` and every other imported alias
      // to `any`, and manufactured an `openType` root the published typings
      // do not have. Substitution — the risk withholding exists to prevent —
      // is unaffected: each source is authenticated against its own lock
      // selection in Rust, which still withholds a name whose copy disagrees.
      if (error instanceof ArtifactResolutionError) {
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
    // Exposed so the artifact-case policy replays a dependency edge through
    // the same memoized resolver the collector uses, instead of a second one
    // that could disagree with it.
    resolutionSession,
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
  let expectedClaims;
  try {
    const current = finiteEntrypoints(manifest, entrypoints, packageRoot);
    const candidates = finiteArtifactCandidates(
      manifest,
      current.entrypoints,
      finiteConditionPartitions(manifest, conditions),
      packageRoot
    ).map(candidate => ({
      ...candidate,
      disposition: artifactCaseDisposition({
        manifest,
        packageRoot,
        entrypoint: candidate.entrypoint,
        conditions: candidate.conditions
      })
    }));
    expected = new Set(
      candidates
        .filter(candidate => candidate.disposition === null)
        .map(candidate => artifactCaseCoordinate(candidate.entrypoint, candidate.conditions))
    );
    // A reused proposal omitted these cases on the strength of a content
    // premise. Recompute the census rather than trusting the sidecar's copy of
    // it: an omitted claim would reach certification unproved, and an invented
    // one would refuse a proposal for a case that was never omitted.
    expectedClaims = new Set(
      declaredApplicabilityClaims(
        candidates
          .filter(candidate => candidate.disposition !== null)
          .map(candidate => ({
            entrypoint: candidate.entrypoint,
            conditions: candidate.conditions,
            class: candidate.disposition.class,
            reason: candidate.disposition.reason
          }))
      ).map(claim => artifactCaseCoordinate(claim.entrypoint, claim.conditions))
    );
  } catch {
    return null;
  }
  // A sidecar written before this field existed declares nothing, which is
  // admissible only for a package whose census claims nothing: the equality
  // below refuses reuse the moment a claim would have to travel unproved.
  const declaredClaims = Array.isArray(inputs.inapplicableCases)
    ? inputs.inapplicableCases
    : [];
  if (declaredClaims.length !== expectedClaims.size) return null;
  for (const claim of declaredClaims) {
    const coordinate = artifactCaseCoordinate(claim?.entrypoint, claim?.conditions);
    if (coordinate === null || !expectedClaims.has(coordinate)) return null;
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

/// Every module of a package that re-exports a given specifier, keyed by that
/// specifier -- not only the first one in canonical order.
///
/// One graph node still stands for one specifier: `prepareState` proves every
/// occurrence locates the same installed copy before it plans anything. But
/// the private generation catalog is keyed by `(importer, specifier)`, and
/// emission asks it about the artifact case's *entry module* and about every
/// relative module the export chain walks through. Naming one arbitrary
/// occurrence answers whichever query happens to match it and silently
/// withholds the accepted contract from the rest, which is then
/// indistinguishable from having no dependency at all: `motion-solidjs@0.6.0`
/// re-exports `motion-dom` from four modules, `dist/v1/core/render-style.mjs`
/// sorted first, and its entry module's `addScaleCorrector` bridge therefore
/// resolved against nothing.
///
/// `edges` are the caller's already-filtered static runtime dependency edges, so the
/// census cannot disagree with the set of nodes that gets planned.
export function staticRuntimeDependencies(resolved) {
  return resolved.externalDependencies.filter(
    dependency => dependency.axis === "runtime" &&
      (dependency.kind === "reexport" || dependency.kind === "import")
  );
}

// A refusal is a discovery hint, never authority. Only request an additional
// contract for a declaration re-export present in this exact module census.
// The native graph must still certify that dependency and bind its receipt to
// the declaration importer; source-only archives do not supply these bindings.
export function staticBindingDependencies(resolved, artifactCase, refusals = []) {
  const conditionsKey = value => JSON.stringify([...new Set([...(value ?? []), "import"])].sort());
  const requested = new Set();
  for (const refusal of refusals) {
    if (refusal.entrypoint !== artifactCase.entrypoint ||
        conditionsKey(refusal.conditions) !== conditionsKey(artifactCase.conditions)) continue;
    const match = /^accepted dependency (.+) has no exact declarations binding for export [^\n]+$/.exec(refusal.reason ?? "");
    if (match) requested.add(match[1]);
  }
  return [
    ...staticRuntimeDependencies(resolved),
    ...resolved.externalDependencies.filter(edge => edge.axis === "declarations" &&
      edge.kind === "reexport" && requested.has(edge.specifier))
  ];
}

export function reexportImporterCensus(packageRoot, edges) {
  const census = new Map();
  for (const edge of edges) {
    const importers = census.get(edge.specifier) ?? new Set();
    importers.add(resolve(packageRoot, edge.importerPath ?? edge.source));
    census.set(edge.specifier, importers);
  }
  return census;
}

export function mergeProposalDependencies(dependencies, outputRoot) {
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
    // One entry per module of the consuming package that imports or re-exports this
    // specifier. `prepareState` located every one of them and refused the node
    // outright unless they all resolved to the same installed copy, so the
    // only field that differs between these entries is the importer -- exactly
    // the field the generation-time index is keyed by. The entries share one
    // document object; emission reads whichever occurrence its export chain
    // arrives at.
    const importers = dependency.reexportImporters?.length
      ? dependency.reexportImporters
      : [resolution.importer];
    if (!importers.includes(resolution.importer)) {
      throw new Error(
        `dependency ${dependency.viaSpecifier} names an importer outside its occurrence census`
      );
    }
    for (const importer of importers) {
      contracts.push({
        document: `objects/${documentName}`,
        documentDigest,
        import: importer === resolution.importer ? resolution : { ...resolution, importer }
      });
    }
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
  trustConfigurationOutput,
  probeHarnessRoot,
  probeNodeExecutable,
  probeRecipeCorpus
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
  const probes = probeRecipeCorpus
    ? { probeHarnessRoot, probeNodeExecutable, probeRecipeCorpus }
    : {};
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
      ...probes,
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
    ...probes,
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
    trustConfigurationOutput,
    ...probeHarnessRequest(options)
  });
  const requestPath = join(scratch, `published-graph-execution-${cases[0].root.index}.json`);
  writeFileSync(requestPath, `${JSON.stringify(execution, null, 2)}\n`);
  const child = await runNativeAsync(
    "solid-checker",
    ["--execute-contract-certification", requestPath],
    { cwd: options.packageRoot, env: { SOLID_CHECKER_DAEMON: "0" } }
  );
  if (child.error) throw new Error(`could not start the native checker: ${child.error.message}`);
  forwardNativeTimings(child);
  if (child.status !== 0) {
    const reason =
      child.stderr.trim() || child.stdout.trim() || `native checker exited ${child.status}`;
    throw new CertificationRefusal({
      stage: "witness-acquisition",
      owner: "certifier",
      reason,
      ...nativeRefusalAttribution(reason)
    });
  }
  return {
    authority: "native-certification-complete",
    catalogRoot,
    withheldClosures: withheldClosuresFromNativeOutput(child.stdout)
  };
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

/// A graph node that could not be prepared, acquired or generated is a fact
/// about that node, not about the transaction. Its dependents' proposals are
/// generated *against* its contract, so the refusal reaches every node above
/// it -- and only those. Nothing weaker would be sound: a dependent generated
/// without the dependency it names is exactly the dependency-blind proposal
/// this lane exists to replace.
export function cascadeGraphNodeRefusals(states, nodeRefusals) {
  let changed = true;
  while (changed) {
    changed = false;
    for (const state of states) {
      if (nodeRefusals.has(state.node.key)) continue;
      const blocked = state.directDependencies.find(
        dependency => nodeRefusals.has(dependency.state.node.key)
      );
      if (!blocked) continue;
      const cause = nodeRefusals.get(blocked.state.node.key);
      nodeRefusals.set(state.node.key, {
        stage: cause.stage,
        reason: `dependency ${blocked.viaSpecifier} refused: ${cause.reason}`
      });
      changed = true;
    }
  }
  return nodeRefusals;
}

/// Splits prepared artifact cases into the ones whose whole graph is intact
/// and the ones a refused node reaches, naming the exact node that refused.
/// A case is never kept with a node missing from its graph, and a refused
/// case is never dropped silently -- it leaves an explicit coordinate and
/// reason for the audit.
export function graphCasesWithoutRefusedNodes({
  prepared,
  byKey,
  nodeRefusals,
  reachable = reachableGraphStates
}) {
  const cases = [];
  const refusals = [];
  for (const item of prepared) {
    const nodes = reachable(item.root, byKey);
    const refused = nodes.find(state => nodeRefusals.has(state.node.key));
    if (!refused) {
      cases.push({ ...item, nodes });
      continue;
    }
    const cause = nodeRefusals.get(refused.node.key);
    refusals.push({
      entrypoint: item.artifactCase.entrypoint,
      conditions: [...item.artifactCase.conditions],
      stage: cause.stage,
      reason:
        `graph node ${refused.node.packageName}@${refused.node.packageVersion} ` +
        `${refused.node.entrypoint} [${refused.node.conditions.join(",")}] refused: ${cause.reason}`
    });
  }
  return { cases, refusals };
}

/// Why a graph that dropped a retained case must be abandoned, as a message,
/// or `null` when every retained case survived preparation.
///
/// The retained cases are the ones the proposal lane already generated and
/// would publish. Here -- unlike at certification time, where every case is
/// still unproved -- the comparison is certain rather than a wager: a retained
/// case this graph cannot even prepare is one this lane certainly will not
/// publish. Taking the lane anyway would trade a covered case for a chance at
/// a frontier one.
export function retainedCaseFloorRefusal(retainedCases, survived, caseRefusals) {
  const lost = retainedCases.filter(value => !survived.has(value));
  if (lost.length === 0) return null;
  const [first] = lost;
  const refusal = caseRefusals.find(
    value => value.entrypoint === first.entrypoint &&
      JSON.stringify(value.conditions) === JSON.stringify(first.conditions)
  );
  return (
    `published dependency graph would drop ${lost.length} retained artifact case(s) the ` +
    `proposal already covers, starting at ${first.entrypoint} ` +
    `[${first.conditions.join(",")}]: ${refusal?.reason ?? "preparation refused it"}`
  );
}

export async function mapWithExactConcurrency(items, concurrency, worker) {
  const results = new Array(items.length);
  let cursor = 0;
  const settled = await Promise.allSettled(
    Array.from({ length: Math.min(concurrency, items.length) }, async () => {
      while (cursor < items.length) {
        const index = cursor++;
        results[index] = await worker(items[index], index);
      }
    })
  );
  // Recovery may start a fresh preparation after a failed one. Do not let
  // workers from that failed transaction keep writing into its scratch tree
  // after its caller observes the failure or removes the tree.
  const failed = settled.find(result => result.status === "rejected");
  if (failed) throw failed.reason;
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

/// The published-dependency-graph preparation for a proposal that generated
/// *partially*, as `{ graph, trace }`: `graph` is the prepared lane, or `null`
/// when that lane does not apply or cannot be prepared.
///
/// `trace` is what the audit must record about a lane that was asked for and
/// not taken. It is `null` when there was no frontier to prepare -- the row
/// simply does not want this lane, which the reused/generated proposal already
/// says -- and a `{ partialProposalFrontier: "unprepared", reason }` row when
/// preparation was attempted and failed. Falling back silently would leave a
/// requested lane that never happened indistinguishable from one never
/// requested, and the reason a graph could not be prepared is exactly the fact
/// a reader of the audit is looking for.
///
/// Same preparation as the failed-generation path -- same acquisition, same
/// per-node generation, same `prepareState` identity proofs, same native
/// reconstruction of every root, closure, edge and receipt. Nothing about the
/// evidence is relaxed for having arrived here from a success: the refusal
/// census is untrusted input either way, `preparePublishedGraphFallback` reads
/// only the case coordinates out of it, and every node is authenticated against
/// its own Bun lock selection before it can authorize anything.
///
/// It reports no graph rather than throwing in two cases, because the caller
/// has a valid alternative in hand that a throw would discard:
///
///   * the census names no exact dependency-composition case, so there is no
///     frontier for this lane (the same condition
///     `preparePublishedGraphFallback` reports as
///     "proposal refusal has no exact dependency-composition case");
///   * preparation itself failed *for every artifact case*. Preparation
///     isolates a broken node to the exact cases whose graph reaches it, so
///     arriving here means no case survived. The partial proposal is then
///     certified exactly as it is without the flag, so an unpreparable graph
///     costs the run nothing it had before. A *refusal* raised during
///     preparation -- a missing issuer or trust configuration, which are
///     request errors and not graph facts -- still propagates.
///
/// `prepare` is injectable so the decision can be tested without a registry, a
/// native process, or an authenticated archive; the default is the one
/// preparation this lane has.
export class RetainedCasePreparationRefusal extends Error {}

export async function preparedGraphForPartialProposal(
  { output, ...preparation },
  { prepare = preparePublishedGraphFallback } = {}
) {
  const refusalPath = `${output}.refusals.json`;
  if (!existsSync(refusalPath)) return { graph: null, trace: null };
  let audit;
  try {
    audit = JSON.parse(readFileSync(refusalPath, "utf8"));
  } catch {
    return { graph: null, trace: null };
  }
  if (!partialProposalHasDependencyFrontier(audit?.refusals)) {
    return { graph: null, trace: null };
  }
  try {
    return { graph: await prepare({ output, ...preparation }), trace: null };
  } catch (error) {
    if (error instanceof CertificationRefusal) throw error;
    if (error instanceof RetainedCasePreparationRefusal &&
        preparation.options?.recoverEntrypoints && preparation.generated) {
      try {
        const scratch = mkdtempSync(join(preparation.scratch, "retained-preparation-"));
        const graph = await prepare({ output, ...preparation, scratch, retainGeneratedCases: true });
        graph.timing.retainedPreparationFallback = { reason: error.message };
        return { graph, trace: null };
      } catch (retryError) {
        if (retryError instanceof CertificationRefusal) throw retryError;
        return { graph: null, trace: { partialProposalFrontier: "unprepared",
          reason: error.message, retainedPreparationRefusal: retryError?.message ?? String(retryError) } };
      }
    }
    return {
      graph: null,
      trace: {
        partialProposalFrontier: "unprepared",
        reason: error?.message ?? String(error)
      }
    };
  }
}

/// Coordinates are acquisition requests, never receipt authority. The native
/// transaction replays all roots and dependency receipts, then verifies the
/// published set through ordinary discovery. Never union catalog directories.
export function recoveryGraphCases(generated, refusals) {
  const retained = generated?.certificationInputs;
  if (!Array.isArray(retained) || retained.length === 0) {
    throw new Error("entrypoint recovery has no generated artifact cases to preserve");
  }
  const frontier = refusals.filter(isExactDependencyCompositionRefusal);
  const seen = new Set();
  const cases = [...retained, ...frontier].map(input => {
    if (typeof input.entrypoint !== "string" ||
        !Array.isArray(input.conditions) ||
        input.conditions.some(condition => typeof condition !== "string")) {
      throw new Error("entrypoint recovery requires exact entrypoint and condition coordinates");
    }
    // Import is implicit in native graph acquisition. Different spellings of
    // the same selection must not turn a conflicting duplicate into two cases.
    const conditions = [...new Set([...input.conditions, "import"])].sort();
    const key = JSON.stringify([input.entrypoint, conditions]);
    if (seen.has(key)) throw new Error(`entrypoint recovery has conflicting duplicate case ${key}`);
    seen.add(key);
    return { entrypoint: input.entrypoint, conditions };
  });
  return {
    cases,
    retainedCases: cases.slice(0, retained.length),
    remainingRefusals: refusals.filter(refusal => !isExactDependencyCompositionRefusal(refusal))
  };
}

async function preparePublishedGraphFallback({
  options,
  manifest,
  scratch,
  output,
  certificationImporter,
  rootArtifactSnapshot,
  fetch_,
  generated = null,
  retainGeneratedCases = false
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
  const recovery = options.recoverEntrypoints && generated
    ? recoveryGraphCases(generated, audit.refusals ?? [])
    : null;
  return preparePublishedGraphCases({
    options, manifest, scratch, certificationImporter, rootArtifactSnapshot,
    fetch_, generated, dependencyCases, recovery, retainGeneratedCases
  });
}

// Exact case coordinates are acquisition requests, not claims that generation
// refused them. Keep graph preparation separate from the refusal-driven lane
// selector so a bounded investigation can request a case without fabricating
// a refusal. Every graph still crosses native archive/lock/receipt verification.
export async function preparePublishedGraphCases({
  options, manifest, scratch, certificationImporter, rootArtifactSnapshot,
  fetch_, generated = null, dependencyCases, recovery = null, retainGeneratedCases = false
}) {
  if (!Array.isArray(dependencyCases) || dependencyCases.length === 0) {
    throw new Error("published graph preparation requires explicit artifact cases");
  }
  mkdirSync(scratch, { recursive: true });
  if (!rootArtifactSnapshot) {
    rootArtifactSnapshot = await acquirePublishedArtifact({ options, manifest, scratch, fetch_ });
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
      const canonicalPackageRoot = realpathSync(resolve(resolved.packageRoot));
      let canonicalImporter;
      try {
        canonicalImporter = realpathSync(resolve(request.importer));
      } catch {
        // The generated root importer is normally materialized already, but a
        // caller may provide an exact synthetic importer for a bounded graph
        // preparation. Preserve its resolved identity until native replay.
        canonicalImporter = resolve(request.importer);
      }
      const node = {
        key,
        importer: canonicalImporter,
        specifier: request.specifier,
        packageRoot: canonicalPackageRoot,
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
            integrity: located.dependencyLock.integrity,
            declarationOnly: dependency.axis === "declarations"
          },
          false,
          nextAncestry
        );
        node.dependencies.push({ specifier: dependency.specifier, node: child.node.key });
        directDependencies.push({ state: child, viaSpecifier: dependency.specifier });
        return true;
      };
      // A declaration-only dependency first uses the independent proposal
      // lane. Its runtime imports remain authenticated compiler sources, not
      // inferred behavioral contracts. Native verification still rejects any
      // claim that actually requires a dependency receipt.
      const semanticEdges = request.declarationOnly ? [] : isRoot
        ? staticBindingDependencies(resolved, artifactCase, dependencyCases)
        : staticRuntimeDependencies(resolved);
      const reexportImporters = reexportImporterCensus(node.packageRoot, semanticEdges);
      for (const dependency of semanticEdges) {
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
        reexportImporters,
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
      const key = publishedGraphRequestKey({ ...request, conditions }) +
        (request.declarationOnly ? ":declaration-proposal" : "");
      if (ancestry.includes(key)) {
        throw new Error(`published dependency graph cycle: ${[...ancestry, key].join(" -> ")}`);
      }
      let state = byKey.get(key);
      if (!state) {
        let preparation = pendingByKey.get(key);
        if (!preparation) {
          // 1024 nodes, matching POLICY_2_GRAPH_NODE_LIMIT: a node is one
          // (artifact, importing module) pair, and importer variants share
          // their generation, so the bound is on discovery, not on work.
          if (byKey.size + pendingByKey.size >= 1024 || ancestry.length > 64) {
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
  // A node that cannot be resolved, located or acquired is a fact about that
  // node, and the artifact cases whose graph reaches it are the exact set it
  // refuses. Preparing them together in one transaction is what makes the
  // dependency evidence shared; it must not also make one broken node refuse
  // every case. Cases are therefore prepared independently, refused nodes are
  // recorded by name, and the lane is abandoned only when nothing survives --
  // which is when the caller's proposal fallback is the better answer.
  // Preparing semantic graphs for all 118 retained Kobalte cases (59
  // entrypoints) exceeded the process-tree budget. Large sets keep those
  // proposals and their ordinary compiler sources as independent roots,
  // preparing semantic dependencies only for the bounded missing frontier.
  const { graphCases: requestedCases, retainedProposalCases } =
    selectRecoveryPreparation(recovery, dependencyCases, { retainGeneratedCases });
  const caseRefusals = [];
  const nodeRefusals = new Map();
  const prepared = (await mapWithExactConcurrency(
    requestedCases,
    publishedGraphPreparationConcurrency(),
    async (artifactCase, caseIndex) => {
      try {
        const preparedCase = await prepareArtifactCase(artifactCase, caseIndex);
        return { ...preparedCase, artifactCase };
      } catch (error) {
        if (error instanceof CertificationRefusal) throw error;
        caseRefusals.push({
          entrypoint: artifactCase.entrypoint,
          conditions: [...(artifactCase.conditions ?? [])],
          stage: "graph-preparation",
          reason: error?.message ?? String(error)
        });
        return null;
      }
    }
  )).filter(Boolean);
  if (prepared.length === 0) {
    throw new Error(
      `published dependency graph prepared no artifact case: ${
        caseRefusals[0]?.reason ?? "no artifact case was requested"
      }`
    );
  }
  // Nodes no surviving root reaches were prepared for a refused case alone.
  // Acquiring and generating them would spend this transaction's budget on
  // evidence nothing can use, and let their own failures refuse a graph they
  // are not part of.
  const reachableFromPrepared = new Set(
    prepared.flatMap(item =>
      reachableGraphStates(item.root, byKey).map(state => state.node.key)
    )
  );
  for (const key of [...byKey.keys()]) {
    if (!reachableFromPrepared.has(key)) byKey.delete(key);
  }
  const sourceByKey = new Map();
  const sourceRefusals = new Map();
  const retainedSourceKeys = new Set(
    [...byKey.values()].flatMap(state => state.sourceDependencies.map(source => source.key))
  );
  const acquisitionUnits = [
    ...[...byKey.values()]
      .filter(state => !state.artifactSnapshot)
      .map(state => ({ kind: "node", state })),
    ...[...sourceArtifacts.values()]
      .filter(source => retainedSourceKeys.has(source.key))
      .map(source => ({ kind: "source", source }))
  ];
  const graphAcquisitionStartedAt = performance.now();
  await mapWithExactConcurrency(
    acquisitionUnits,
    publishedGraphPreparationConcurrency(),
    async unit => {
      if (unit.kind === "node") {
        try {
          unit.state.artifactSnapshot = await acquireSharedPublishedArtifact(
            unit.state.nodeManifest,
            unit.state.node.integrity,
            unit.state.scratch
          );
        } catch (error) {
          if (error instanceof CertificationRefusal) throw error;
          nodeRefusals.set(unit.state.node.key, {
            stage: "graph-acquisition",
            reason: error?.message ?? String(error)
          });
        }
        return;
      }
      let artifact;
      try {
        artifact = await acquireSharedPublishedArtifact(
          unit.source.manifest,
          unit.source.integrity,
          unit.source.scratch
        );
      } catch (error) {
        if (error instanceof CertificationRefusal) throw error;
        sourceRefusals.set(unit.source.key, error?.message ?? String(error));
        return;
      }
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
    if (nodeRefusals.has(state.node.key)) continue;
    const acquiredSources = [];
    let unacquired = null;
    for (const source of state.sourceDependencies) {
      const acquired = sourceByKey.get(source.key);
      if (!acquired) {
        unacquired = `compiler source ${source.packageName}@${source.packageVersion} was not acquired${
          sourceRefusals.has(source.key) ? `: ${sourceRefusals.get(source.key)}` : ""
        }`;
        break;
      }
      acquiredSources.push(acquired);
    }
    if (unacquired) {
      nodeRefusals.set(state.node.key, { stage: "graph-acquisition", reason: unacquired });
      continue;
    }
    state.sourceDependencies = acquiredSources;
  }
  cascadeGraphNodeRefusals([...byKey.values()], nodeRefusals);
  const graphAcquisitionDurationMs =
    Math.round((performance.now() - graphAcquisitionStartedAt) * 100) / 100;
  const pendingGeneration = new Set(
    [...byKey.values()].filter(state => !nodeRefusals.has(state.node.key))
  );
  let proposalGenerations = 0;
  let proposalGenerationsShared = 0;
  // Importer variants of one artifact generate once. A node is keyed by the
  // module that imported it, so one package's many entrypoints put the same
  // `solid-js` into the graph once per importing module; every such variant is
  // the same package root, integrity, entrypoint and conditions over the same
  // dependency variants, and the generator's `--certification-importer` changes
  // nothing in the emitted document -- only `certificationInputs[].resolution.
  // importer` names it. The first variant generates; the others take its
  // document with their own importer written back into the resolution, and
  // native certification still replays each variant's resolution from its own
  // importer before anything is trusted. Keyed as a promise so two variants in
  // one frontier share a single in-flight generation.
  const generationByVariantKey = new Map();
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
        // A node whose proposal cannot be generated refuses the artifact
        // cases that reach it, not the transaction. Its dependents cannot
        // generate against a contract it never produced, so the cascade
        // below removes them from this frontier rather than leaving the
        // loop with nothing ready.
        const generateNodeProposal = async () => {
          const dependencies = state.directDependencies.map(dependency => ({
            ...dependency.state,
            viaSpecifier: dependency.viaSpecifier,
            reexportImporters: [
              ...(state.reexportImporters.get(dependency.viaSpecifier) ?? [])
            ].sort()
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
          const variantKey = JSON.stringify([
            state.node.packageRoot,
            state.node.integrity,
            state.node.entrypoint,
            state.node.conditions,
            dependencies
              .map(dependency => [dependency.viaSpecifier, dependency.variantKey ?? dependency.node.key])
              .sort((left, right) => left[0].localeCompare(right[0]) || left[1].localeCompare(right[1]))
          ]);
          state.variantKey = variantKey;
          let generated;
          const shared = generationByVariantKey.get(variantKey);
          if (shared) {
            let representative;
            try {
              representative = await shared;
            } catch (error) {
              throw new Error(
                `published dependency graph node ${state.node.packageName}@${state.node.packageVersion} ` +
                `${state.node.entrypoint} [${state.node.conditions.join(",")}] refused: ${error.message}`,
                { cause: error }
              );
            }
            proposalGenerationsShared += 1;
            generated = {
              ...representative,
              certificationInputs: representative.certificationInputs.map(input => ({
                ...input,
                resolution: { ...input.resolution, importer: state.node.importer }
              }))
            };
          } else {
            const generation = generatePackageContract(generationArguments, {
              quiet: true,
              proposalDependencies: merged.proposalDependencies,
              proposalDependencyCatalog: merged.catalog,
              privateGraphPreparation: true,
              exactConditions: explicitConditions
            });
            generationByVariantKey.set(variantKey, generation);
            try {
              generated = await generation;
            } catch (error) {
              throw new Error(
                `published dependency graph node ${state.node.packageName}@${state.node.packageVersion} ` +
                `${state.node.entrypoint} [${state.node.conditions.join(",")}] refused: ${error.message}`,
                { cause: error }
              );
            }
          }
          const plannings = certificationPlannings(generated, state.artifactSnapshot, options);
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
        };
        try {
          await generateNodeProposal();
        } catch (error) {
          if (error instanceof CertificationRefusal) throw error;
          nodeRefusals.set(state.node.key, {
            stage: "graph-generation",
            reason: error?.message ?? String(error)
          });
        }
      }
    );
    proposalFrontiers.push({
      nodes: ready.length,
      durationMs: Math.round((performance.now() - frontierStartedAt) * 100) / 100
    });
    for (const state of ready) pendingGeneration.delete(state);
    cascadeGraphNodeRefusals([...pendingGeneration], nodeRefusals);
    for (const state of [...pendingGeneration]) {
      if (nodeRefusals.has(state.node.key)) pendingGeneration.delete(state);
    }
  }
  const surviving = graphCasesWithoutRefusedNodes({ prepared, byKey, nodeRefusals });
  caseRefusals.push(...surviving.refusals);
  preparedCases.push(...surviving.cases);
  if (preparedCases.length === 0) {
    throw new Error(
      `published dependency graph prepared no artifact case: ${
        caseRefusals[0]?.reason ?? "no artifact case survived preparation"
      }`
    );
  }
  let retainedRoots = [];
  if (retainedProposalCases.length) {
    const lock = exactBunLockSelection(bunLockIndex, manifest.name, manifest.version,
      bunLockLocatorForInstalledPackage(bunLockPath, options.packageRoot));
    if (lock.integrity !== options.integrity) {
      throw new Error("retained proposal lock integrity disagrees with its archive");
    }
    const sourceDependenciesByInput = await acquireRootCompilerSources({
      options, generated, scratch, fetch_
    });
    retainedRoots = retainedProposalGraphCases({
      plannings: certificationPlannings(generated, rootArtifactSnapshot, options),
      sourceDependenciesByInput, coordinates: retainedProposalCases,
      lockfile: resolve(bunLockPath), lockLocator: lock.locator
    });
    demandPlans.push(...await planDemands({ options, generated,
      artifactSnapshot: rootArtifactSnapshot, scratch }));
    preparedCases.unshift(...retainedRoots);
    recovery.retainedProposalRoots = true;
  }
  // Recovery's selection strategy reads these coordinates and requires them to
  // describe exactly the cases the graph carries, retained ones first. A case
  // that never prepared is not a case this transaction can publish, so it
  // leaves the set and stays visible as an explicit preparation refusal.
  //
  // The retained cases are the floor, and here -- unlike at certification time
  // -- the comparison is certain rather than a wager. A retained case is one
  // the proposal lane already generated and would publish; a retained case
  // this graph cannot even prepare is one this lane certainly will not. Taking
  // the lane anyway would trade a proved case for a chance at a frontier one.
  // So a dropped retained case abandons the graph, and the caller publishes
  // the proposal exactly as it would have without the lane.
  if (recovery) {
    const survived = new Set(preparedCases.map(item => item.artifactCase));
    const floor = retainedCaseFloorRefusal(recovery.retainedCases, survived, caseRefusals);
    if (floor) throw new RetainedCasePreparationRefusal(floor);
    recovery.expectedCases = recovery.cases;
    recovery.preparationRefusals = caseRefusals;
    recovery.cases = recovery.cases.filter(value => survived.has(value));
  }
  return {
    preparedCases,
    demandPlans: demandPlans.filter(Boolean),
    timing: {
      ...(recovery ? { entrypointRecovery: recovery } : {}),
      ...(caseRefusals.length ? { preparationRefusals: caseRefusals } : {}),
      requestedCases: requestedCases.length + retainedRoots.length,
      retainedProposalCases: retainedRoots.length,
      preparedDependencyGraphCases: surviving.cases.length,
      rootCases: preparedCases.length,
      canonicalNodes: byKey.size + retainedRoots.length,
      acquiredPublishedArtifacts: publishedArtifacts.size,
      acquisitionUnits: acquisitionUnits.length,
      graphAcquisitionDurationMs,
      resolutionSession: graphResolutionSession.statistics(),
      compilerSourceClosureCensus: compilerSourceClosureCount(),
      proposalGenerations,
      // Nodes that took an importer variant's document instead of generating;
      // `proposalGenerations - proposalGenerationsShared` generators ran.
      proposalGenerationsShared,
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
  // Recovery invokes this repeatedly in one transaction. Each collector starts
  // its source index at zero, so sharing a directory turns EEXIST into a false
  // unavailable-source disposition and can remove authenticated dependencies.
  const sourceScratch = mkdtempSync(join(scratch, "root-sources-"));
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
        refuseCaseImportingUnexportedTarget({
          entrypoint: input.entrypoint ?? ".",
          conditions,
          dependency,
          located,
          resolutionSession: collector.resolutionSession
        });
        found.push(...await collector.collectCompilerSources(located, conditions, new Set()));
      } catch (error) {
        // A refused case is not an unnameable package: the dependency is
        // authenticated and its declarations must still reach every case that
        // does resolve, which is the repair this loop's own catch exists for.
        if (error instanceof CertificationRefusal) throw error;
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

/// The probe-harness paths the native transaction needs, or nothing.
///
/// All three or none: a partially configured harness is a refusal in Rust
/// rather than a silently narrower binding. Note what is *not* here — the
/// request cannot declare a sandbox kind or an isolation policy. The verifier
/// computes both from the scheme it actually runs, so no caller can assert an
/// isolation property the transaction does not have.
///
/// The harness root is this repository/package root, because the harness
/// source manifest the verifier recomputes covers `packages/cli/...` paths
/// relative to it.
function probeHarnessRequest(options) {
  if (!options.probeRecipeCorpus) return {};
  const node = options.probeNodeExecutable || process.env.SOLID_CHECKER_PROBE_NODE || "";
  const resolvedNode = node ? realpathSync(resolve(node)) : probeNodeOnPath();
  if (!resolvedNode) {
    throw new CertificationRefusal({
      stage: "witness-acquisition",
      owner: "probe-gate",
      reason:
        "a probe recipe corpus was configured but no Node executable could be resolved; set SOLID_CHECKER_PROBE_NODE to the real path this build pinned"
    });
  }
  return {
    probeHarnessRoot: harnessRoot,
    probeNodeExecutable: resolvedNode,
    probeRecipeCorpus: resolve(options.probeRecipeCorpus)
  };
}

/// The `node` on PATH, by real path, or "" when there is none. The adapter
/// refuses a symlink, so the resolution has to end at real bytes.
function probeNodeOnPath() {
  for (const directory of (process.env.PATH ?? "").split(":")) {
    if (!directory) continue;
    const candidate = join(directory, "node");
    try {
      const real = realpathSync(candidate);
      if (statSync(real).isFile()) return real;
    } catch {
      continue;
    }
  }
  return "";
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
      sourceDependencies: sourceDependenciesByInput[index] ?? [],
      inapplicableCases: generated.inapplicableCases ?? []
  }));
  const execution = {
    schemaVersion: plannings.length === 1 ? 1 : 2,
    ...(plannings.length === 1 ? { planning: plannings[0] } : { plannings }),
    typefactsExecutable: resolve(typefactsExecutable),
    issuerConfiguration: resolve(options.issuerConfiguration),
    catalogRoot,
    trustConfigurationOutput: resolve(options.trustConfigurationOutput),
    ...probeHarnessRequest(options)
  };
  writeFileSync(requestPath, `${JSON.stringify(execution, null, 2)}\n`);
  const child = await runNativeAsync(
    "solid-checker",
    ["--execute-contract-certification", requestPath],
    { cwd: options.packageRoot, env: { SOLID_CHECKER_DAEMON: "0" } }
  );
  if (child.error) throw new Error(`could not start the native checker: ${child.error.message}`);
  forwardNativeTimings(child);
  if (child.status !== 0) {
    const reason =
      child.stderr.trim() || child.stdout.trim() || `native checker exited ${child.status}`;
    throw new CertificationRefusal({
      stage: "witness-acquisition",
      owner: "certifier",
      reason,
      ...nativeRefusalAttribution(reason)
    });
  }
  return {
    authority: "native-certification-complete",
    catalogRoot,
    withheldClosures: withheldClosuresFromNativeOutput(child.stdout)
  };
}

export async function executeNativeOrGraphCertification({
  options,
  generated,
  graph,
  artifactSnapshot,
  scratch,
  fetch_,
  caseRecovery = null,
  prepareGeneratedGraph = null
}, { executeNative = executeNativeCertification, executeGraph = executePreparedPublishedGraphs } = {}) {
  const catalogRoot = options.catalog.endsWith("accepted-contracts.json")
    ? dirname(options.catalog) : options.catalog;
  // A refused native attempt can itself create this directory. Capture the
  // protection before either lane runs, never reinterpret that later directory
  // as a pre-existing publication or discard a publication that was here.
  const existingPublication = existsSync(catalogRoot);
  const certifyGenerated = async (recovery, destination = options, publicationExists = existingPublication, trialPrefix = "independent-case-trial") => {
    if (recovery) {
      let round = 0;
      return certifyIndependentCaseSelection({
        cases: generated.certificationInputs,
        recovery,
        existingPublication: publicationExists,
        certify: async (inputs, publish) => {
          const trial = join(scratch, `${trialPrefix}-${++round}`);
          recovery.nativeCertificationTransactions = round;
          let output = generated.output;
          if (inputs.length !== generated.certificationInputs.length) {
            mkdirSync(trial, { recursive: true });
            output = join(trial, "proposal.json");
            writeFileSync(output, `${JSON.stringify(projectProposalCases(
              JSON.parse(readFileSync(generated.output, "utf8")), inputs
            ))}\n`);
          }
          return executeNative({
            options: publish ? destination : { ...destination,
              catalog: join(trial, "catalog"), trustConfigurationOutput: join(trial, "trust.json") },
            generated: { ...generated, output, certificationInputs: inputs },
            artifactSnapshot, scratch, fetch_
          });
        }
      });
    }
    return executeNative({
      options: destination,
      generated,
      artifactSnapshot,
      scratch,
      fetch_
    });
  };
  if (!graph) {
    try {
      return await certifyGenerated(caseRecovery);
    } catch (error) {
      // No accepted case can be traded for this retry: the ordinary single
      // case just refused, and the destination had no publication beforehand.
      // Infrastructure failures and multi-case proposals retain their refusal.
      if (!prepareGeneratedGraph || existingPublication ||
          generated?.certificationInputs?.length !== 1 ||
          !(error instanceof CertificationRefusal) ||
          error.owner !== "certifier" || error.stage !== "witness-acquisition" ||
          !error.demandId || !["argument-binding", "callable-path"].includes(error.family)) throw error;
      const prepared = await prepareGeneratedGraph(error);
      return executeGraph({ options, cases: prepared.preparedCases, scratch,
        catalogRoot, trustConfigurationOutput: options.trustConfigurationOutput });
    }
  }
  const execute = (cases, catalogRoot, trustConfigurationOutput) => executeGraph({
    options,
    cases,
    scratch,
    catalogRoot,
    trustConfigurationOutput
  });
  const recovery = graph.timing?.entrypointRecovery;
  if (!options.recoverEntrypoints || !recovery) {
    return execute(graph.preparedCases, catalogRoot, options.trustConfigurationOutput);
  }
  let round = 0;
  const establishFloor = generated && recovery.retainedProposalRoots && existingPublication === false ? async () => {
    const selection = {}, privateRoot = join(scratch, "verified-retained-floor");
    const destination = { ...options, catalog: join(privateRoot, "catalog"), trustConfigurationOutput: join(privateRoot, "trust.json") };
    const result = await certifyGenerated(selection, destination, false, "retained-floor-trial");
    if (result.authority !== "native-certification-complete" || result.catalogRoot !== destination.catalog) throw new Error("retained floor requires a completed native publication in its private destination");
    const key = item => JSON.stringify([item.entrypoint, [...new Set([...item.conditions, "import"])].sort()]);
    const wanted = new Set(selection.publishedCases.map(key));
    const proposal = JSON.parse(readFileSync(generated.output, "utf8"));
    const expected = generated.certificationInputs.filter(input => wanted.has(key(input))).map(input => {
      const projected = projectProposalCases(proposal, [input]);
      const { exports: _exports, ...artifact } = projected.entrypoints[input.entrypoint].cases[0];
      return { coordinate: selection.publishedCases.find(item => key(item) === key(input)),
        package: projected.package, selection: artifact, resolution: input.resolution };
    });
    const publication = inspectRetainedCertificationFloor({ catalogRoot: result.catalogRoot, expected });
    return { acceptedCases: selection.publishedCases, caseRefusals: selection.caseRefusals ?? [], publication,
      nativeCertificationTransactions: selection.nativeCertificationTransactions };
  } : null;
  return certifyRecoverableCaseSelection({
    cases: graph.preparedCases,
    recovery,
    existingPublication,
    establishFloor,
    fallback: generated ? async error => {
      recovery.graphRefusal = { stage: error.stage, owner: error.owner,
        demandId: error.demandId, family: error.family, reason: error.reason };
      const independent = graph.timing.independentCaseRecovery = {};
      const result = await certifyGenerated(independent);
      recovery.publishedCases = [...independent.publishedCases];
      recovery.caseRefusals = [...(recovery.caseRefusals ?? []), ...(independent.caseRefusals ?? [])];
      const key = item => JSON.stringify([item.entrypoint, [...item.conditions].sort()]);
      const published = new Set(recovery.publishedCases.map(key));
      recovery.unpublishedCases = recovery.cases.filter(item => !published.has(key(item)));
      graph.timing.retainedProposalFallback = true;
      graph.timing.reusedProposal = graph.timing.reusedProposalForRecovery === true;
      return result;
    } : null,
    certify: async (cases, publish) => {
      graph.timing.nativeCertificationTransactions = ++round;
      graph.timing.typeFactsCaseSetBatches = round;
      const trial = join(scratch, `recovery-trial-${round}`);
      return execute(cases, publish ? catalogRoot : join(trial, "catalog"),
        publish ? options.trustConfigurationOutput : join(trial, "trust.json"));
    }
  });
}

// Select whole unaccepted artifact cases, never individual claims. Native
// planning independently authenticates each coordinate and requires its case
// census to equal this proposal's census before any receipt is published.
export function projectProposalCases(document, inputs) {
  const entrypoints = {};
  for (const input of inputs) {
    const resolution = input.resolution;
    if (!resolution || document.package.name !== resolution.packageName ||
        document.package.version !== resolution.packageVersion ||
        document.package.integrity !== resolution.packageIntegrity ||
        input.entrypoint !== resolution.requestedEntrypoint) {
      throw new Error("case projection requires exact package and entrypoint identities");
    }
    const matches = (document.entrypoints[input.entrypoint]?.cases ?? []).filter(item =>
      resolve(resolution.packageRoot, item.artifact.path) === resolution.runtime.path &&
      `sha256:${item.artifact.sha256}` === resolution.runtime.digest &&
      resolve(resolution.packageRoot, item.declarations.path) === resolution.declarations.path &&
      `sha256:${item.declarations.sha256}` === resolution.declarations.digest &&
      item.resolution.runtimeBranch === resolution.runtimeTrace.branch &&
      item.resolution.typesBranch === resolution.declarationTrace.branch
    );
    if (matches.length !== 1) throw new Error("case projection requires one exact artifact selection");
    const selected = entrypoints[input.entrypoint] ??= { cases: [] };
    if (selected.cases.includes(matches[0])) throw new Error("case projection contains a duplicate artifact selection");
    selected.cases.push(matches[0]);
  }
  if (!inputs.length) throw new Error("case projection cannot publish an empty census");
  const summaries = {};
  for (const entrypoint of Object.values(entrypoints)) {
    for (const item of entrypoint.cases) {
      for (const reference of Object.values(item.exports)) {
        const id = typeof reference === "string" ? reference : reference.summary;
        if (!Object.hasOwn(document.summaries, id)) throw new Error("case projection references a missing summary");
        summaries[id] = document.summaries[id];
      }
    }
  }
  return { ...document, entrypoints, summaries };
}

// The proposal is unaccepted, so no case is assumed proved. Every selected
// subset gets a fresh native transaction, including final publication. An
// existing publication cannot be replaced with a smaller set by this lane.
export async function certifyIndependentCaseSelection({ cases, recovery, existingPublication, certify }) {
  if (typeof existingPublication !== "boolean") throw new Error("independent recovery requires an explicit publication-state check");
  const coordinates = recoveryGraphCases({ certificationInputs: cases }, []).cases;
  recovery.expectedCases = coordinates;
  const isProofRefusal = error => error instanceof CertificationRefusal &&
    error.owner === "certifier" && error.stage === "witness-acquisition";
  let combinedFailure;
  try {
    const result = await certify(cases, true);
    recovery.publishedCases = coordinates;
    return result;
  } catch (error) {
    if (!isProofRefusal(error) || existingPublication || cases.length < 2 || cases.length > 1024) throw error;
    combinedFailure = error;
    recovery.combinedRefusal = error.reason ?? error.message;
  }
  const selected = [], accepted = [];
  recovery.caseRefusals = [];
  const refuse = (index, error) => {
    recovery.caseRefusals.push({ ...coordinates[index], stage: error.stage,
        owner: error.owner, demandId: error.demandId ?? null,
        family: error.family ?? null, reason: error.reason ?? error.message });
  };
  if (cases.length > 32) {
    // The document format bounds the complete census to 1024 cases. A binary
    // subdivision makes at most 2*N-2 private transactions, instead of repeatedly
    // verifying a growing prefix. Successful batches provide selection hints
    // only: the complete union is independently verified before publication.
    recovery.strategy = "binary-subdivision";
    const visit = async (start, end) => {
      try {
        await certify(cases.slice(start, end), false);
        selected.push(...cases.slice(start, end));
        accepted.push(...coordinates.slice(start, end));
      } catch (error) {
        if (!isProofRefusal(error)) throw error;
        if (end - start === 1) return refuse(start, error);
        const middle = start + Math.floor((end - start) / 2);
        await visit(start, middle);
        await visit(middle, end);
      }
    };
    const middle = Math.floor(cases.length / 2);
    await visit(0, middle);
    await visit(middle, cases.length);
  } else {
    for (let index = 0; index < cases.length; index++) {
      try {
        await certify([...selected, cases[index]], false);
        selected.push(cases[index]);
        accepted.push(coordinates[index]);
      } catch (error) {
        if (!isProofRefusal(error)) throw error;
        refuse(index, error);
      }
    }
  }
  if (!selected.length) throw combinedFailure;
  const result = await certify(selected, true);
  recovery.publishedCases = accepted;
  return result;
}

// Selection is orchestration only. Every trial and final publication asks the
// native verifier to rebuild all proofs, dependencies, trust and case bindings.
// Trial receipts never become inputs to the final transaction.
export async function certifyRecoverableCaseSelection({
  cases,
  recovery,
  certify,
  fallback = null,
  establishFloor = null,
  // ADR 0070. `false` asserts that nothing was published before this attempt,
  // which is what allows a retained-baseline failure to publish a subset of
  // the prepared set instead of abandoning the graph. `null` is "not asked",
  // and keeps the original behaviour: a caller that cannot state the
  // publication state must not reduce one.
  existingPublication = null
}) {
  if (fallback) {
    try {
      return await certifyRecoverableCaseSelection({ cases, recovery, certify, existingPublication, establishFloor });
    } catch (error) {
      if (!(error instanceof CertificationRefusal) || error.owner !== "certifier" || error.stage !== "witness-acquisition") throw error;
      return fallback(error);
    }
  }
  if (recovery.retainedProposalRoots) {
    return certifyRetainedProposalSelection({ cases, recovery, certify,
      establishFloor: existingPublication === false ? establishFloor : null,
      isProofRefusal: error => error instanceof CertificationRefusal &&
        error.owner === "certifier" && error.stage === "witness-acquisition"
    });
  }
  const retainedCount = recovery.retainedCases.length;
  if (!retainedCount || retainedCount >= cases.length || cases.length > 32 ||
      recovery.cases.length !== cases.length ||
      JSON.stringify(recovery.cases.slice(0, retainedCount)) !== JSON.stringify(recovery.retainedCases)) {
    return certify(cases, true);
  }
  const isProofRefusal = error => error instanceof CertificationRefusal &&
    error.owner === "certifier" && error.stage === "witness-acquisition";
  try {
    const result = await certify(cases, true);
    recovery.publishedCases = [...recovery.cases];
    return result;
  } catch (error) {
    if (!isProofRefusal(error)) throw error;
    recovery.combinedRefusal = error.reason ?? error.message;
  }
  // A retained case is mandatory for *this* strategy: it is the baseline every
  // trial extends, so a baseline failure ends it rather than silently dropping
  // coverage the original proposal selected.
  //
  // ADR 0070: ending the strategy is not the same as abandoning the graph. The
  // prepared set is the retained cases *plus* the dependency-composition
  // frontier, and the proposal this otherwise falls back to never contained
  // that frontier — those cases refused at generation for want of a dependency
  // binding. Losing all of them because one retained case cannot be proved
  // throws away the only lane that ever covered them. So select independently
  // across the whole prepared set instead, retained cases included, on the one
  // condition that makes publishing a subset safe: nothing was published here
  // before. A caller that cannot state that keeps the original behaviour.
  const selected = cases.slice(0, retainedCount);
  const accepted = recovery.cases.slice(0, retainedCount);
  try {
    await certify(selected, false);
  } catch (error) {
    if (!isProofRefusal(error) || existingPublication !== false) throw error;
    recovery.retainedBaselineRefusal = error.reason ?? error.message;
    recovery.strategy = "independent-prepared-selection";
    selected.length = 0;
    accepted.length = 0;
    recovery.caseRefusals = [];
    // Subdivision, not a growing prefix. Every trial here re-certifies the
    // whole prepared graph -- for `@kobalte/utils` that is 113 nodes -- so a
    // transaction per case is the difference between finishing and hitting the
    // certification budget. A batch that proves is a selection *hint* only; the
    // complete union is certified again before anything is published.
    const visit = async (start, end) => {
      try {
        await certify([...selected, ...cases.slice(start, end)], false);
        selected.push(...cases.slice(start, end));
        accepted.push(...recovery.cases.slice(start, end));
      } catch (failure) {
        if (!isProofRefusal(failure)) throw failure;
        if (end - start === 1) {
          recovery.caseRefusals.push({
            ...recovery.cases[start],
            stage: failure.stage, owner: failure.owner,
            demandId: failure.demandId ?? null, family: failure.family ?? null,
            reason: failure.reason ?? failure.message
          });
          return;
        }
        const middle = start + Math.floor((end - start) / 2);
        await visit(start, middle);
        await visit(middle, end);
      }
    };
    await visit(0, cases.length);
    // Nothing survived, so this lane has no answer at all. Rethrow the baseline
    // refusal and let the proposal fallback have its turn.
    if (!selected.length) throw error;
    const result = await certify(selected, true);
    recovery.publishedCases = accepted;
    return result;
  }
  recovery.caseRefusals = [];
  for (let index = retainedCount; index < cases.length; index++) {
    try {
      await certify([...selected, cases[index]], false);
      selected.push(cases[index]);
      accepted.push(recovery.cases[index]);
    } catch (error) {
      if (!isProofRefusal(error)) throw error;
      recovery.caseRefusals.push({
        ...recovery.cases[index],
        stage: error.stage, owner: error.owner,
        demandId: error.demandId ?? null, family: error.family ?? null,
        reason: error.reason ?? error.message
      });
    }
  }
  // Native duplicate/conflict checks and ordinary consumer verification still
  // gate the complete final selection, even when every private trial passed.
  const result = await certify(selected, true);
  recovery.publishedCases = accepted;
  return result;
}

function writeAudit(
  path,
  manifest,
  refusal,
  demandPlans,
  stageDurationsMs,
  graphPreparation = null,
  withheldClosures = []
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
        withheldClosures,
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
  graphPreparation = null,
  withheldClosures = []
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
    // Closure candidates recipe-gated planning withheld by name: each names
    // the export, the domain, the exact semantic claim id a recipe would have
    // had to carry, and the reason. The certified contract leaves those
    // domains open; this is the record of why.
    withheldClosures,
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
    inapplicableCases: admitted.inapplicableCases ?? [],
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
  let withheldClosures = [];
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
          if (options.proposalRefusalAudit && !options.recoverEntrypoints) {
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
          if (reused && !options.recoverEntrypoints) {
            reusedProposal = true;
            return { authority: "rust", generated: reused, graph: null };
          }
          try {
            const generated = reused ?? await generatePackageContract(generationArguments, { quiet: true });
            reusedProposal = Boolean(reused);
            // Generation succeeded, so `generated` is a real answer for the
            // cases that produced one. When it is only a *partial* answer and
            // the missing cases refused on dependency composition, the caller
            // may prefer the lane that certifies exactly those cases with an
            // authenticated dependency catalog behind them. The graph lane is
            // not a superset of the partial proposal -- it covers the refused
            // cases and not the others -- which is why it is opt-in.
            if (options.dependencyGraphLane || options.recoverEntrypoints) {
              const { graph, trace } = await preparedGraphForPartialProposal({
                options,
                manifest,
                scratch,
                output: proposalOutput,
                certificationImporter,
                rootArtifactSnapshot: artifactSnapshot,
                fetch_,
                generated
              });
              if (graph) {
                reusedProposal = false;
                graphPreparation = Object.assign(graph.timing, {
                  partialProposalFrontier: true,
                  ...(options.recoverEntrypoints ? { reusedProposalForRecovery: Boolean(reused) } : {})
                });
                return { authority: "rust", generated: options.recoverEntrypoints ? generated : null, graph };
              }
              // A lane that was requested, attempted and could not be prepared
              // leaves the partial proposal as the answer -- but it must leave
              // a trace, or the audit reads exactly like a row that never
              // wanted the lane.
              if (trace) graphPreparation = { ...(graphPreparation ?? {}), ...trace };
            }
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
          withheldClosures = Array.isArray(witnesses?.withheldClosures)
            ? witnesses.withheldClosures
            : [];
          return { authority: "rust", witnesses };
        })
      },
      evidence: {
        obtainWitnesses: async ({ artifactSnapshot, openProposal }) =>
          measure("witnessAcquisition", () => {
            const caseRecovery = options.recoverEntrypoints && !openProposal.graph &&
              openProposal.generated?.certificationInputs.length > 1 ? {} : null;
            if (caseRecovery) graphPreparation = { ...(graphPreparation ?? {}), independentCaseRecovery: caseRecovery };
            return executeNativeOrGraphCertification({
              options,
              generated: openProposal.generated,
              graph: openProposal.graph,
              artifactSnapshot,
              scratch,
              fetch_,
              caseRecovery,
              prepareGeneratedGraph: !openProposal.graph &&
                (options.dependencyGraphLane || options.recoverEntrypoints) ? async originalRefusal => {
                const trace = {
                  originalRefusal: originalRefusal.reason ?? originalRefusal.message,
                  demandId: originalRefusal.demandId,
                  family: originalRefusal.family,
                  originalProposalDigest: `sha256:${createHash("sha256").update(readFileSync(openProposal.generated.output)).digest("hex")}`,
                  cases: openProposal.generated.certificationInputs.map(({ entrypoint, conditions }) => ({ entrypoint, conditions }))
                };
                graphPreparation = { ...(graphPreparation ?? {}), generatedProposalProofFallback: trace };
                try {
                  const graph = await preparePublishedGraphCases({
                    options, manifest, scratch: join(scratch, "generated-proof-fallback"),
                    certificationImporter, rootArtifactSnapshot: artifactSnapshot, fetch_,
                    dependencyCases: trace.cases
                  });
                  graphPreparation = { ...graph.timing, generatedProposalProofFallback: trace };
                  reusedProposal = false;
                  demandPlans.splice(0, demandPlans.length, ...graph.demandPlans);
                  return graph;
                } catch (error) {
                  trace.preparationRefusal = error?.message ?? String(error);
                  throw originalRefusal;
                }
              } : null
            });
          })
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
      reusedProposal ? { ...(graphPreparation ?? {}), reusedProposal: true } : graphPreparation,
      withheldClosures
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
      // Same merge as the success audit: which lane produced the proposal is a
      // fact about the attempt, and a refused attempt needs it attributed just
      // as much as a certified one.
      reusedProposal ? { ...(graphPreparation ?? {}), reusedProposal: true } : graphPreparation,
      withheldClosures
    );
    throw refusal;
  } finally {
    rmSync(scratch, { recursive: true, force: true });
    if (!certified && createdCertificationImporter) {
      rmSync(certificationImporterPath, { force: true });
    }
  }
}
