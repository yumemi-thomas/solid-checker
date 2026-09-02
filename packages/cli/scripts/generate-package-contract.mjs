// Stable package-contract proposal producer.
//
// Node owns exact artifact acquisition and process/file lifecycle. Rust owns
// semantic inference, normalization, proposal closure weakening, compact
// encoding, and multi-artifact merging. This file never reads a summary.

import { createHash, randomUUID } from "node:crypto";
import {
  copyFileSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync
} from "node:fs";
import { availableParallelism, tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import process from "node:process";

import { runNativeAsync } from "../bin/launcher.mjs";
import {
  ArtifactResolutionError,
  ArtifactResolutionSession,
  MUTUALLY_EXCLUSIVE_CONDITION_AXES,
  RESOLVER_STANDARD_CONDITIONS,
  isPrivateNamespacedCondition,
  nonModuleTargetExtension,
  resolvePackageExport,
  selectPackageExportTarget
} from "./artifact-resolution.mjs";

export const ARTIFACT_APPLICABILITY = Object.freeze({
  RuntimeModule: "runtime-module",
  TypeOnlyExport: "verifier-proved-type-only",
  MissingPublishedTarget: "unavailable-published-target",
  UnsupportedConditionSet: "unsupported-condition-environment",
  UnsupportedArtifactShape: "unsupported-artifact-shape"
});

/**
 * An artifact case that asserts nothing about certifiable behavior. It is not a
 * refusal: a refusal says "a consumer can reach this and we could not prove it",
 * while these two classes say "no consumer reaches a certifiable module here at
 * all". An inapplicable case is recorded with its class and reason, never
 * certified, never counted as a refusal, and never suppresses a sibling case or
 * the proposal.
 */
export const ARTIFACT_DISPOSITION = Object.freeze({
  UnpublishedConditionalTarget: "unpublished-conditional-target",
  NonModuleTarget: "non-module-target"
});

/**
 * The disposition of one exact artifact case, decided from the export-map
 * selection alone and before any analysis. Returns `null` for every case that
 * keeps ordinary certify-or-refuse semantics — including a selection that
 * refuses for its own reason (`blocked`, `conditions-unmatched`, `not-exported`,
 * invalid target syntax, traversal), which is a property of the package and
 * still refuses.
 *
 * `unpublished-conditional-target`: the runtime target is absent from the
 * artifact and the selection traversed at least one *namespaced* custom
 * condition (`@scope/name` or `vendor/name`). The artifact itself proves the
 * target unpublished (`files` excluded it) and, by the namespacing convention,
 * no consumer reaches it without naming that private condition in its own
 * resolver configuration, so there is no behavior to certify. A target reached
 * through standard conditions — or through a *bare-name* custom condition such
 * as `bun`, `workerd`, `edge-light`, `react-native`, or `electron`, each of
 * which its ecosystem activates unconditionally — stays a refusal: real
 * consumers do fail there, and that is a defective publish worth reporting.
 *
 * `non-module-target`: the selected runtime target's filename is one of the
 * genuinely non-executable resource extensions (`nonModuleTargetExtension`) —
 * a sourcemap, stylesheet, JSON, image, font, or document. `.node`/`.wasm` and
 * unknown extensions are deliberately *not* in that set; an entrypoint of that
 * kind is a native-code/opaque-wasm hazard rather than "nothing to assert", and
 * still refuses. Assets remain ordinary closure members; this rule only says an
 * *entrypoint* must be a module.
 */
export function artifactCaseDisposition({
  manifest,
  packageRoot,
  entrypoint,
  conditions,
  resolutionKind = "import"
}) {
  let selected;
  try {
    selected = selectPackageExportTarget({
      packageRoot,
      manifest,
      entrypoint,
      conditions,
      axis: "runtime",
      resolutionKind
    });
  } catch {
    return null;
  }
  const extension = nonModuleTargetExtension(selected.path);
  if (extension) {
    return {
      class: ARTIFACT_DISPOSITION.NonModuleTarget,
      reason: `runtime target extension ${JSON.stringify(extension)} is not an executable module`
    };
  }
  if (selected.exists) return null;
  const namespaced = selected.conditions.filter(condition =>
    isPrivateNamespacedCondition(condition)
  );
  if (namespaced.length === 0) return null;
  return {
    class: ARTIFACT_DISPOSITION.UnpublishedConditionalTarget,
    reason:
      `runtime target is unpublished behind private namespaced export condition(s) ` +
      `${namespaced.map(condition => JSON.stringify(condition)).join(", ")}`
  };
}

export function artifactApplicabilityForRefusal(error) {
  if (!(error instanceof ArtifactResolutionError)) {
    return ARTIFACT_APPLICABILITY.RuntimeModule;
  }
  if (error.code === "target-not-found") {
    return ARTIFACT_APPLICABILITY.MissingPublishedTarget;
  }
  if (["blocked", "conditions-unmatched", "not-exported"].includes(error.code)) {
    return ARTIFACT_APPLICABILITY.UnsupportedConditionSet;
  }
  if (error.code === "module-not-found") {
    return error.message.includes("node_modules/")
      ? ARTIFACT_APPLICABILITY.UnsupportedArtifactShape
      : ARTIFACT_APPLICABILITY.MissingPublishedTarget;
  }
  if (
    [
      "declarations-not-found",
      "invalid-specifier",
      "invalid-target",
      "package-imports-unsupported",
      "package-not-found",
      "unmaterialized-transform"
    ].includes(error.code)
  ) {
    return ARTIFACT_APPLICABILITY.UnsupportedArtifactShape;
  }
  return ARTIFACT_APPLICABILITY.RuntimeModule;
}

export const packageContractHelp = `Usage:
  solid-checker contract generate --integrity <SRI> [OPTIONS]

Generates an unaccepted stable schema-version-1 proposal. Exact artifact
identity is acquired independently and Rust owns all semantic normalization.
The proposal does not become analyzer input until proof verification issues a
receipt for its exact bytes.

Options:
  --package-root <DIR>   Package root (default: current directory)
  --output <FILE>        Output path (default: solid-reactivity.json)
  --integrity <SRI>      Exact installed package tarball integrity (required)
  --entrypoint <SUBPATH> Exact exported subpath (repeatable; default: all finite subpaths)
  --conditions <LIST>    Exact runtime conditions, e.g. browser,development
  -h, --help             Show this help
`;

function parseArguments(arguments_) {
  const options = {
    packageRoot: process.cwd(),
    output: "",
    integrity: "",
    entrypoints: [],
    conditions: [],
    certificationImporter: ""
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
    else if (key === "--output") options.output = value;
    else if (key === "--integrity") options.integrity = value;
    else if (key === "--entrypoint") options.entrypoints.push(value);
    else if (key === "--conditions") {
      options.conditions.push(...value.split(",").map(item => item.trim()).filter(Boolean));
    } else if (key === "--certification-importer") {
      options.certificationImporter = resolve(value);
    } else {
      throw new Error(`unknown contract generation argument ${key}`);
    }
  }
  return options;
}

function terminalExportTargets(value, targets = []) {
  if (typeof value === "string") targets.push(value);
  else if (Array.isArray(value)) {
    for (const child of value) terminalExportTargets(child, targets);
  } else if (value && typeof value === "object") {
    for (const child of Object.values(value)) terminalExportTargets(child, targets);
  }
  return targets;
}

function packageFiles(packageRoot, limit = 100_000) {
  const files = [];
  const visit = (directory, prefix) => {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      if (entry.name === "node_modules") continue;
      const relative = prefix ? `${prefix}/${entry.name}` : entry.name;
      if (entry.isDirectory()) visit(join(directory, entry.name), relative);
      else if (entry.isFile()) files.push(`./${relative}`);
      if (files.length > limit) {
        throw new Error(`package file census exceeds ${limit} entries`);
      }
    }
  };
  visit(packageRoot, "");
  return files.sort();
}

function expandWildcardEntrypoint(key, target, files) {
  if (
    (key.match(/\*/g)?.length ?? 0) !== 1 ||
    (target.match(/\*/g)?.length ?? 0) !== 1 ||
    !target.startsWith("./") ||
    target.includes("\\") ||
    target.split("/").includes("..")
  ) {
    return null;
  }
  const [prefix, suffix] = target.split("*");
  const matches = [];
  for (const file of files) {
    if (!file.startsWith(prefix) || !file.endsWith(suffix)) continue;
    const capture = file.slice(prefix.length, file.length - suffix.length);
    if (
      !capture ||
      capture.split("/").some(part => !part || [".", ".."].includes(part))
    ) {
      continue;
    }
    matches.push(key.replace("*", capture));
  }
  return matches;
}

// This is a compiled adapter copy of proof policy 2's Rust-owned
// resourceBudgets.artifactCaseCandidates value. The workflow test pins it to
// the generated audit manifest so a policy change cannot silently drift.
export const ARTIFACT_CASE_CANDIDATE_LIMIT = 1_024;

export function finiteEntrypoints(
  manifest,
  requested,
  packageRoot = null,
  {
    artifactCaseCandidateLimit = ARTIFACT_CASE_CANDIDATE_LIMIT
  } = {}
) {
  // Every entrypoint has at least one active artifact candidate. Condition
  // variants are counted only after exact branch selection below; multiplying
  // every entrypoint by the package-global partition count rejects packages
  // whose many unconditional subpaths all resolve identically.
  const withinBudget = entrypointCount => entrypointCount <= artifactCaseCandidateLimit;
  if (requested.length) {
    const entrypoints = [...new Set(requested)].sort();
    if (!withinBudget(entrypoints.length)) {
      throw new Error(
        `${entrypoints.length} artifact-case candidates exceed the proof-policy resource limit of ${artifactCaseCandidateLimit}`
      );
    }
    return {
      entrypoints,
      wildcardRefusals: [],
      wildcardBranchRefusals: [],
      wildcardResourceRefusals: []
    };
  }
  const exports_ = manifest.exports;
  if (!exports_ || typeof exports_ !== "object" || Array.isArray(exports_)) {
    return {
      entrypoints: ["."],
      wildcardRefusals: [],
      wildcardBranchRefusals: [],
      wildcardResourceRefusals: []
    };
  }
  const keys = Object.keys(exports_);
  const subpaths = keys.filter(key => key.startsWith("."));
  if (!subpaths.length) {
    return {
      entrypoints: ["."],
      wildcardRefusals: [],
      wildcardBranchRefusals: [],
      wildcardResourceRefusals: []
    };
  }
  const wildcardKeys = subpaths.filter(key => key.includes("*")).sort();
  const wildcardRefusals = [];
  const wildcardBranchRefusals = [];
  const wildcardResourceRefusals = [];
  const entrypoints = subpaths.filter(key => !key.includes("*"));
  if (!withinBudget(new Set(entrypoints).size)) {
    throw new Error(
      `${new Set(entrypoints).size} explicit artifact-case candidates exceed the proof-policy resource limit of ${artifactCaseCandidateLimit}`
    );
  }
  const files = packageRoot && wildcardKeys.length ? packageFiles(packageRoot) : [];
  for (const key of wildcardKeys) {
    const targets = terminalExportTargets(exports_[key]);
    const expanded = packageRoot
      ? targets.map(target => expandWildcardEntrypoint(key, target, files))
      : [];
    const materialized = expanded.filter(matches_ => matches_ !== null && matches_.length > 0);
    if (materialized.length === 0) {
      wildcardRefusals.push(key);
      continue;
    }
    for (let index = 0; index < expanded.length; index += 1) {
      if (expanded[index] === null || expanded[index].length === 0) {
        wildcardBranchRefusals.push({ entrypoint: key, target: targets[index] });
      }
    }
    const additions = materialized.flat();
    const candidateCount = new Set([...entrypoints, ...additions]).size;
    if (!withinBudget(candidateCount)) {
      wildcardResourceRefusals.push({
        entrypoint: key,
        candidates: candidateCount,
        limit: artifactCaseCandidateLimit
      });
      continue;
    }
    entrypoints.push(...additions);
  }
  entrypoints.sort();
  if (!entrypoints.length) {
    throw new Error(
      `package exports ${wildcardRefusals.join(", ")}; pass each finite --entrypoint explicitly so generation does not guess the public surface`
    );
  }
  return {
    entrypoints: [...new Set(entrypoints)],
    wildcardRefusals,
    wildcardBranchRefusals,
    wildcardResourceRefusals
  };
}

function specifierFor(packageName, entrypoint) {
  if (entrypoint === ".") return packageName;
  if (!entrypoint.startsWith("./")) {
    throw new Error(`entrypoint ${JSON.stringify(entrypoint)} must be "." or start with "./"`);
  }
  return `${packageName}/${entrypoint.slice(2)}`;
}

// The census and the artifact-case disposition rules must agree on exactly one
// standard condition vocabulary; both read the resolver's lists.
const mutuallyExclusiveConditionAxes = MUTUALLY_EXCLUSIVE_CONDITION_AXES;

export function finiteConditionPartitions(manifest, requested) {
  if (requested.length) return [[...new Set(requested)].sort()];
  const conditions = new Set();
  const visit = value => {
    if (Array.isArray(value)) {
      for (const child of value) visit(child);
      return;
    }
    if (!value || typeof value !== "object") return;
    for (const [key, child] of Object.entries(value)) {
      if (!key.startsWith(".") && !RESOLVER_STANDARD_CONDITIONS.includes(key)) {
        conditions.add(key);
      }
      visit(child);
    }
  };
  visit(manifest.exports);
  const names = [...conditions].sort();
  const grouped = new Set(mutuallyExclusiveConditionAxes.flat());
  const axes = [
    ...mutuallyExclusiveConditionAxes
      .map(axis => axis.filter(condition => conditions.has(condition)))
      .filter(axis => axis.length > 0)
      .map(axis => [null, ...axis]),
    ...names.filter(condition => !grouped.has(condition)).map(condition => [null, condition])
  ];
  const partitionCount = axes.reduce((count, axis) => count * axis.length, 1);
  if (partitionCount > 256) {
    throw new Error(
      `package exports select ${partitionCount} valid condition partitions; pass an exact --conditions list because the finite partition would exceed 256 cases`
    );
  }
  let partitions = [[]];
  for (const axis of axes) {
    partitions = partitions.flatMap(partition =>
      axis.map(condition => condition === null ? partition : [...partition, condition])
    );
  }
  // Preserve the Cartesian census order with the empty/default selection
  // first. Besides being stable, this keeps the unconditioned package branch
  // as the representative when an explicitly conditioned branch resolves to
  // identical bytes and the semantic merge can retain only one of them.
  return partitions.map(partition => partition.sort());
}

function selectedBranchIdentity({ manifest, entrypoint, conditions, packageRoot, axis }) {
  try {
    const selected = resolvePackageExport({
      packageRoot,
      manifest,
      entrypoint,
      conditions,
      axis,
      resolutionKind: "import"
    });
    return {
      status: "selected",
      path: selected.file.path,
      digest: selected.file.digest,
      branch: selected.trace.branch,
      steps: selected.trace.steps
    };
  } catch (error) {
    return {
      status: "refused",
      name: error?.name ?? "Error",
      code: error?.code ?? null,
      message: error?.message ?? String(error)
    };
  }
}

export function finiteArtifactCandidates(
  manifest,
  entrypoints,
  partitions,
  packageRoot,
  { artifactCaseCandidateLimit = ARTIFACT_CASE_CANDIDATE_LIMIT } = {}
) {
  const candidates = [];
  for (const entrypoint of entrypoints) {
    const selected = new Set();
    for (const conditions of partitions) {
      const identity = JSON.stringify([
        selectedBranchIdentity({
          manifest,
          entrypoint,
          conditions,
          packageRoot,
          axis: "runtime"
        }),
        selectedBranchIdentity({
          manifest,
          entrypoint,
          conditions,
          packageRoot,
          axis: "declarations"
        })
      ]);
      if (selected.has(identity)) continue;
      selected.add(identity);
      candidates.push({ entrypoint, conditions });
      if (candidates.length > artifactCaseCandidateLimit) {
        throw new Error(
          `${candidates.length} exact artifact-case candidates exceed the proof-policy resource limit of ${artifactCaseCandidateLimit}`
        );
      }
    }
  }
  return candidates;
}

async function checked(args, cwd) {
  const child = await runNativeAsync("solid-checker", args, {
    cwd,
    env: { SOLID_CHECKER_DAEMON: "0" }
  });
  if (child.error) throw new Error(`could not start the native checker: ${child.error.message}`);
  if (child.status !== 0) {
    throw new Error(child.stderr.trim() || child.stdout.trim() || `native checker exited ${child.status}`);
  }
  return child;
}

function projectFiles(resolution) {
  const files = new Set([resolution.runtime.path]);
  for (const entry of resolution.closure.entries ?? []) {
    if (!["runtime", "literal-dynamic-chunk"].includes(entry.role)) continue;
    files.add(resolve(resolution.packageRoot, entry.path));
  }
  return [...files].sort();
}

const ARTIFACT_ANALYSIS_CONCURRENCY = 4;
// Type Facts can share one TypeScript program across compatible entry files,
// but a package-wide demand set is not linear: Kobalte's 629 exact targets in
// one request takes longer than the former singleton total. Keep each shared
// acquisition bounded while still amortizing process and program setup.
const ARTIFACT_ANALYSIS_BATCH_TARGET_LIMIT = 16;

export function artifactAnalysisBatchConcurrencyLimit(env = process.env) {
  const raw = env.SOLID_CHECKER_ARTIFACT_ANALYSIS_BATCH_CONCURRENCY;
  if (raw === undefined || raw === "") return 8;
  const limit = Number(raw);
  if (!Number.isInteger(limit) || limit <= 0) {
    throw new Error(
      "SOLID_CHECKER_ARTIFACT_ANALYSIS_BATCH_CONCURRENCY must be a positive integer"
    );
  }
  return Math.min(8, limit);
}

export function recommendedArtifactAnalysisBatchConcurrency(
  batchCount,
  parallelism = availableParallelism(),
  configuredLimit = artifactAnalysisBatchConcurrencyLimit()
) {
  if (!Number.isInteger(batchCount) || batchCount < 0) {
    throw new Error("artifact analysis batch count must be a non-negative integer");
  }
  if (!Number.isInteger(configuredLimit) || configuredLimit <= 0) {
    throw new Error("artifact analysis batch concurrency limit must be a positive integer");
  }
  const hostLimit = Number.isInteger(parallelism) && parallelism > 0
    ? Math.min(8, parallelism, configuredLimit)
    : 1;
  const demandLimit = batchCount < 32
    ? 1
    : batchCount < 128
      ? 2
      : batchCount < 512
        ? 4
        : 8;
  return Math.min(hostLimit, demandLimit);
}

export function partitionArtifactAnalysisBatches(
  candidates,
  targetLimit = ARTIFACT_ANALYSIS_BATCH_TARGET_LIMIT
) {
  if (!Number.isInteger(targetLimit) || targetLimit <= 0) {
    throw new Error("artifact analysis batch target limit must be a positive integer");
  }
  const compatible = new Map();
  for (const candidate of candidates) {
    // TypeScript answers are program-scoped. Equal export conditions do not
    // make two entry files compatible when their exact source closures differ:
    // opening their union can turn an unanswered fact into an answer. Batch
    // only targets whose pinned program and runtime conditions are identical.
    const key = JSON.stringify([
      candidate.prepared.conditions,
      projectFiles(candidate.prepared.resolution)
    ]);
    const batch = compatible.get(key) ?? [];
    batch.push(candidate);
    compatible.set(key, batch);
  }
  return [...compatible.values()].flatMap(batch => {
    const chunks = [];
    for (let offset = 0; offset < batch.length; offset += targetLimit) {
      chunks.push(batch.slice(offset, offset + targetLimit));
    }
    return chunks;
  });
}

async function mapConcurrent(items, concurrency, worker) {
  const results = new Array(items.length);
  let next = 0;
  await Promise.all(
    Array.from({ length: Math.min(concurrency, items.length) }, async () => {
      while (next < items.length) {
        const index = next++;
        results[index] = await worker(items[index], index);
      }
    })
  );
  return results;
}

function stableRefusalReason(error, { packageRoot, scratch }) {
  return (error?.message ?? String(error))
    .replaceAll(packageRoot, "<package-root>")
    .replaceAll(scratch, "<scratch>");
}

// Additive: `inapplicable` sits beside `refusals` under the same envelope
// version. Every existing consumer validates `format`, `refusalVersion`, and
// `Array.isArray(refusals)`, and every one of them counts `refusals.length` as
// the refusal total — so a separate array is what makes "never counted as a
// refusal" true by construction rather than by editing each counter.
export const CERTIFICATION_INPUTS_FORMAT = "solid-checker-contract-certification-inputs";

/// Records, next to an emitted proposal, exactly what `contract certify` would
/// otherwise regenerate in-process: the emitted artifact cases with their
/// resolutions, and the identity they were generated under. The document and
/// plan are bound by digest so a later `--proposal` reuse cannot pair these
/// inputs with different bytes. Nothing here is authority: Rust verifies every
/// resolution against the authenticated archive and treats the document as an
/// untrusted candidate, exactly as it does for an in-process generation.
function writeCertificationInputs(output, plan, {
  manifest,
  integrity,
  packageRoot,
  certificationImporter,
  entrypoints,
  conditions,
  certificationInputs
}) {
  const digest = path => `sha256:${createHash("sha256").update(readFileSync(path)).digest("hex")}`;
  writeFileSync(
    `${output}.certification-inputs.json`,
    `${JSON.stringify(
      {
        format: CERTIFICATION_INPUTS_FORMAT,
        inputsVersion: 1,
        package: { name: manifest.name, version: manifest.version },
        integrity,
        packageRoot,
        certificationImporter: certificationImporter || null,
        entrypoints,
        conditions,
        document: { path: output, sha256: digest(output) },
        plan: { path: plan, sha256: digest(plan) },
        certificationInputs
      },
      null,
      2
    )}\n`
  );
}

function writeProposalRefusalAudit(output, manifest, refusals, inapplicable = []) {
  mkdirSync(dirname(output), { recursive: true });
  writeFileSync(
    `${output}.refusals.json`,
    `${JSON.stringify(
      {
        format: "solid-checker-contract-proposal-refusals",
        refusalVersion: 1,
        package: { name: manifest.name, version: manifest.version },
        refusals,
        inapplicable
      },
      null,
      2
    )}\n`
  );
}

export async function retainIndependentlyMergeableProposals(proposals, attemptMerge) {
  let merged = null;
  let acceptedCount = 0;
  const rejected = [];
  for (const [index, candidate] of proposals.entries()) {
    try {
      merged = await attemptMerge(merged, candidate, index);
      acceptedCount += 1;
    } catch (error) {
      rejected.push({ candidate, error });
    }
  }
  return { merged, acceptedCount, rejected };
}

export async function retainIndependentlyMergeableProposalBatches(
  proposals,
  attemptMerge,
  rootFailure = null
) {
  let merged = null;
  const accepted = [];
  const rejected = [];
  const consider = async (start, end, knownFailure = null) => {
    const candidates = proposals.slice(start, end);
    if (knownFailure === null) {
      try {
        merged = await attemptMerge(accepted, candidates, { start, end });
        accepted.push(...candidates);
        return;
      } catch (error) {
        knownFailure = error;
      }
    }
    if (candidates.length === 1) {
      rejected.push({ candidate: candidates[0], error: knownFailure });
      return;
    }
    const middle = start + Math.floor(candidates.length / 2);
    await consider(start, middle);
    await consider(middle, end);
  };
  if (proposals.length > 0) await consider(0, proposals.length, rootFailure);
  return { merged, acceptedCount: accepted.length, rejected };
}

export function prepareArtifact({
  packageRoot,
  manifest,
  integrity,
  entrypoint,
  conditions,
  resolutionSession,
  certificationImporter,
  acceptedDependencies
}) {
  const specifier = specifierFor(manifest.name, entrypoint);
  const importer = certificationImporter ||
    join(dirname(packageRoot), `.solid-checker-acquisition-${randomUUID()}.mjs`);
  const resolution = resolutionSession.resolve({
    importer,
    specifier,
    packageRoot,
    conditions: [...new Set([...conditions, "import"])],
    resolutionKind: "import",
    integrity,
    acceptedDependencies
  });
  return {
    entrypoint,
    conditions,
    resolution,
    identity: JSON.stringify({
      entrypoint,
      runtime: resolution.runtime,
      declarations: resolution.declarations,
      runtimeTrace: resolution.runtimeTrace,
      declarationTrace: resolution.declarationTrace,
      closure: resolution.closure.digest
    })
  };
}

async function analyzeArtifact({
  packageRoot,
  manifest,
  prepared,
  scratch,
  acceptedContractCatalog,
  receiptTrustConfiguration,
  proposalDependencyCatalog
}) {
  const startedAt = performance.now();
  const { entrypoint, conditions, resolution, identity } = prepared;
  const id = randomUUID();
  const project = join(scratch, `${id}-tsconfig.json`);
  const resolutionPath = join(scratch, `${id}-resolution.json`);
  const runtimeResolutions = join(scratch, `${id}-runtime-resolutions.json`);
  const output = join(scratch, `${id}-proposal.json`);
  const plan = join(scratch, `${id}-proposal-plan.json`);
  writeFileSync(
    project,
    `${JSON.stringify(
      {
        compilerOptions: {
          allowJs: true,
          checkJs: true,
          jsx: "preserve",
          module: "ESNext",
          moduleResolution: "Bundler",
          skipLibCheck: true,
          target: "ES2022"
        },
        files: projectFiles(resolution)
      },
      null,
      2
    )}\n`
  );
  writeFileSync(resolutionPath, `${JSON.stringify(resolution, null, 2)}\n`);
  writeFileSync(runtimeResolutions, '{"schemaVersion":1,"resolutions":[]}\n');
  const analyzerArguments = [
      "--project",
      project,
      "--emit-contract",
      output,
      "--emit-proposal-plan",
      plan,
      "--runtime-module-resolutions",
      runtimeResolutions,
      "--contract-resolution",
      resolutionPath,
      "--package-name",
      manifest.name,
      "--package-version",
      manifest.version,
      "--contract-entry-file",
      resolution.runtime.path,
      "--contract-package-root",
      packageRoot,
      "--runtime-conditions",
      [...new Set([...conditions, "import"])].sort().join(",")
    ];
  if (acceptedContractCatalog) {
    analyzerArguments.push("--accepted-contracts", acceptedContractCatalog);
  }
  if (receiptTrustConfiguration) {
    analyzerArguments.push("--receipt-trust-configuration", receiptTrustConfiguration);
  }
  if (proposalDependencyCatalog) {
    analyzerArguments.push("--proposal-dependencies", proposalDependencyCatalog);
  }
  await checked(
    analyzerArguments,
    packageRoot
  );
  return {
    document: output,
    plan,
    entrypoint,
    conditions,
    resolution,
    identity,
    analysisDurationMs: performance.now() - startedAt
  };
}

async function analyzeArtifactsBatch({
  packageRoot,
  manifest,
  candidates,
  scratch,
  acceptedContractCatalog,
  receiptTrustConfiguration,
  proposalDependencyCatalog,
  batchTargetLimit
}) {
  const analysisBatches = partitionArtifactAnalysisBatches(candidates, batchTargetLimit);
  const batched = await mapConcurrent(
    analysisBatches,
    recommendedArtifactAnalysisBatchConcurrency(analysisBatches.length),
    async (batch, batchIndex) => {
      const startedAt = performance.now();
      const id = randomUUID();
      const project = join(scratch, `${id}-batch-tsconfig.json`);
      const runtimeResolutions = join(scratch, `${id}-batch-runtime-resolutions.json`);
      const batchRequest = join(scratch, `${id}-contract-batch.json`);
      const batchResults = join(scratch, `${id}-contract-batch-results.json`);
      const files = new Set();
      const targets = batch.map(candidate => {
        for (const file of projectFiles(candidate.prepared.resolution)) files.add(file);
        const resolution = join(scratch, `${id}-${candidate.index}-resolution.json`);
        const output = join(scratch, `${id}-${candidate.index}-proposal.json`);
        const plan = join(scratch, `${id}-${candidate.index}-proposal-plan.json`);
        writeFileSync(
          resolution,
          `${JSON.stringify(candidate.prepared.resolution, null, 2)}\n`
        );
        return {
          index: candidate.index,
          output,
          plan,
          resolution,
          entryFile: candidate.prepared.resolution.runtime.path,
          sourceFiles: projectFiles(candidate.prepared.resolution)
        };
      });
      writeFileSync(project, `${JSON.stringify({
        compilerOptions: {
          allowJs: true,
          checkJs: true,
          jsx: "preserve",
          module: "ESNext",
          moduleResolution: "Bundler",
          skipLibCheck: true,
          target: "ES2022"
        },
        files: [...files].sort()
      }, null, 2)}\n`);
      writeFileSync(runtimeResolutions, '{"schemaVersion":1,"resolutions":[]}\n');
      writeFileSync(batchRequest, `${JSON.stringify({
        schemaVersion: 1,
        targets
      }, null, 2)}\n`);
      const analyzerArguments = [
        "--project",
        project,
        "--emit-contract-batch",
        batchRequest,
        "--contract-batch-results",
        batchResults,
        "--runtime-module-resolutions",
        runtimeResolutions,
        "--package-name",
        manifest.name,
        "--package-version",
        manifest.version,
        "--contract-package-root",
        packageRoot,
        "--runtime-conditions",
        [...new Set([...batch[0].prepared.conditions, "import"])].sort().join(",")
      ];
      if (acceptedContractCatalog) {
        analyzerArguments.push("--accepted-contracts", acceptedContractCatalog);
      }
      if (receiptTrustConfiguration) {
        analyzerArguments.push(
          "--receipt-trust-configuration",
          receiptTrustConfiguration
        );
      }
      if (proposalDependencyCatalog) {
        analyzerArguments.push("--proposal-dependencies", proposalDependencyCatalog);
      }
      try {
        await checked(analyzerArguments, packageRoot);
      } catch (error) {
        return batch.map(candidate => ({
          index: candidate.index,
          candidate: candidate.prepared,
          error
        }));
      }
      const results = JSON.parse(readFileSync(batchResults, "utf8"));
      const resultByIndex = new Map(results.map(result => [result.index, result]));
      const duration = (performance.now() - startedAt) / Math.max(1, batch.length);
      return batch.map(candidate => {
        const result = resultByIndex.get(candidate.index);
        const target = targets.find(target => target.index === candidate.index);
        if (!result?.success) {
          return {
            index: candidate.index,
            candidate: candidate.prepared,
            error: new Error(result?.error ?? `contract batch ${batchIndex} omitted target`)
          };
        }
        return {
          index: candidate.index,
          proposal: {
            document: target.output,
            plan: target.plan,
            entrypoint: candidate.prepared.entrypoint,
            conditions: candidate.prepared.conditions,
            resolution: candidate.prepared.resolution,
            identity: candidate.prepared.identity,
            analysisDurationMs: Number.isFinite(result.durationNs)
              ? result.durationNs / 1_000_000
              : duration
          }
        };
      });
    }
  );
  return batched.flat();
}

function emitGenerationTiming(timing) {
  if (!timing) return;
  process.stderr.write(`${JSON.stringify({ contractGenerationTiming: timing })}\n`);
}

export async function generatePackageContract(
  arguments_,
  {
    quiet = false,
    acceptedDependencies = {},
    acceptedContractCatalog = "",
    receiptTrustConfiguration = "",
    proposalDependencies = {},
    proposalDependencyCatalog = "",
    privateGraphPreparation = false,
    exactConditions = null,
    artifactAnalysisBatchTargetLimit = ARTIFACT_ANALYSIS_BATCH_TARGET_LIMIT
  } = {}
) {
  if (Object.keys(acceptedDependencies).length > 0) {
    if (!acceptedContractCatalog || !receiptTrustConfiguration) {
      throw new Error(
        "accepted dependency identities require an authenticated contract catalog and trust configuration"
      );
    }
  }
  if (Object.keys(proposalDependencies).length > 0 && !proposalDependencyCatalog) {
    throw new Error(
      "private graph proposal dependencies require a private proposal dependency catalog"
    );
  }
  if (
    Object.keys(proposalDependencies).length > 0 &&
    (Object.keys(acceptedDependencies).length > 0 ||
      acceptedContractCatalog ||
      receiptTrustConfiguration)
  ) {
    throw new Error(
      "private graph proposal dependencies cannot be combined with accepted receipt authority"
    );
  }
  const resolutionDependencies = Object.keys(proposalDependencies).length > 0
    ? proposalDependencies
    : acceptedDependencies;
  const generationStartedAt = performance.now();
  const timing = process.env.SOLID_CHECKER_TIMINGS
    ? {
        censusMs: 0,
        preparationMs: 0,
        analysisMs: 0,
        mergeMs: 0,
        validationMs: 0,
        artifactCaseCandidates: 0,
        preparedCases: 0,
        analysisGroups: 0,
        exactSourceProgramBatches: 0,
        exactSourceProgramBatchSizes: [],
        factBuildsAvoided: 0,
        analyzedTargets: 0,
        targets: [],
        totalMs: 0
      }
    : null;
  const options = parseArguments(arguments_);
  if (options.help) {
    process.stdout.write(packageContractHelp);
    return;
  }
  if (!options.integrity) {
    throw new Error(
      "--integrity is required; exact package identity cannot be inferred from package.json"
    );
  }
  const packageRoot = resolve(options.packageRoot);
  const manifestPath = join(packageRoot, "package.json");
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  if (!manifest.name || !manifest.version) {
    throw new Error(`${manifestPath} must declare exact name and version`);
  }
  const censusStartedAt = performance.now();
  const partitions = exactConditions === null
    ? finiteConditionPartitions(manifest, options.conditions)
    : [[...new Set(exactConditions)].sort()];
  const {
    entrypoints,
    wildcardRefusals,
    wildcardBranchRefusals,
    wildcardResourceRefusals
  } = finiteEntrypoints(
    manifest,
    options.entrypoints,
    packageRoot
  );
  const artifactCandidates = finiteArtifactCandidates(
    manifest,
    entrypoints,
    partitions,
    packageRoot
  );
  if (timing) timing.censusMs = performance.now() - censusStartedAt;
  const output = resolve(options.output || join(packageRoot, "solid-reactivity.json"));
  const scratch = mkdtempSync(join(tmpdir(), "solid-checker-contract-"));
  const proposals = [];
  let emittedArtifactCases = 0;
  let certificationProposals = [];
  const inapplicable = [];
  const refusals = wildcardRefusals.map(entrypoint => ({
    entrypoint,
    conditions: null,
    stage: "entrypoint-census",
    applicability: ARTIFACT_APPLICABILITY.RuntimeModule,
    reason: "wildcard export requires an explicit finite --entrypoint census"
  }));
  refusals.push(...wildcardBranchRefusals.map(({ entrypoint, target }) => ({
    entrypoint,
    conditions: null,
    stage: "entrypoint-census",
    applicability: ARTIFACT_APPLICABILITY.MissingPublishedTarget,
    reason: `wildcard export branch ${JSON.stringify(target)} has no published target`
  })));
  refusals.push(...wildcardResourceRefusals.map(({ entrypoint, candidates, limit }) => ({
    entrypoint,
    conditions: null,
    stage: "entrypoint-census",
    applicability: ARTIFACT_APPLICABILITY.RuntimeModule,
    reason: `finite wildcard expansion would require ${candidates} artifact-case candidates, exceeding the proof-policy resource limit of ${limit}`
  })));
  try {
    const preparationStartedAt = performance.now();
    const preparedCases = [];
    const preparationOutcomes = [];
    const resolutionSession = new ArtifactResolutionSession();
    let caseIndex = 0;
    for (const { entrypoint, conditions } of artifactCandidates) {
      const disposition = artifactCaseDisposition({
        manifest,
        packageRoot,
        entrypoint,
        conditions
      });
      if (disposition) {
        inapplicable.push({
          entrypoint,
          conditions,
          stage: "artifact-case",
          class: disposition.class,
          reason: disposition.reason
        });
        caseIndex += 1;
        continue;
      }
      try {
        preparedCases.push({
          index: caseIndex,
          prepared: prepareArtifact({
            packageRoot,
            manifest,
            integrity: options.integrity,
            entrypoint,
            conditions,
            resolutionSession,
            certificationImporter: options.certificationImporter,
            acceptedDependencies: resolutionDependencies
          })
        });
      } catch (error) {
        preparationOutcomes.push({
          index: caseIndex,
          candidate: { entrypoint, conditions },
          error
        });
      }
      caseIndex += 1;
    }
    if (timing) {
      timing.preparationMs = performance.now() - preparationStartedAt;
      timing.artifactCaseCandidates = caseIndex;
      timing.preparedCases = preparedCases.length;
      timing.resolutionSession = resolutionSession.statistics();
    }
    const groups = new Map();
    for (const candidate of preparedCases) {
      const group = groups.get(candidate.prepared.identity) ?? [];
      group.push(candidate);
      groups.set(candidate.prepared.identity, group);
    }
    if (timing) timing.analysisGroups = groups.size;
    const analysisStartedAt = performance.now();
    const groupedCandidates = [...groups.values()];
    if (timing) {
      const exactSourceProgramBatches = partitionArtifactAnalysisBatches(
        groupedCandidates.map(group => group[0]),
        artifactAnalysisBatchTargetLimit
      );
      timing.exactSourceProgramBatches = exactSourceProgramBatches.length;
      timing.exactSourceProgramBatchSizes = exactSourceProgramBatches.map(batch => batch.length);
      timing.factBuildsAvoided = exactSourceProgramBatches.reduce(
        (total, batch) => total + Math.max(0, batch.length - 1),
        0
      );
    }
    const primaryOutcomes = await analyzeArtifactsBatch({
      packageRoot,
      manifest,
      candidates: groupedCandidates.map(group => group[0]),
      scratch,
      acceptedContractCatalog,
      receiptTrustConfiguration,
      proposalDependencyCatalog,
      batchTargetLimit: artifactAnalysisBatchTargetLimit
    });
    const primaryOutcomeByIndex = new Map(
      primaryOutcomes.map(outcome => [outcome.index, outcome])
    );
    const analyzedGroups = await mapConcurrent(
      groupedCandidates,
      ARTIFACT_ANALYSIS_CONCURRENCY,
      async group => {
        const primary = primaryOutcomeByIndex.get(group[0].index) ?? {
          index: group[0].index,
          candidate: group[0].prepared,
          error: new Error("contract emission batch omitted its primary artifact case")
        };
        if (primary.proposal) return [primary];

        // A shared analysis can reject one exact entry file without making the
        // remaining artifact candidates for that identity equivalent. Preserve
        // the old ordered fallback semantics, but pay the per-target process
        // cost only on this exceptional path.
        const outcomes = [primary];
        for (const candidate of group.slice(1)) {
          try {
            const proposal = await analyzeArtifact({
              packageRoot,
              manifest,
              prepared: candidate.prepared,
              scratch,
              acceptedContractCatalog,
              receiptTrustConfiguration,
              proposalDependencyCatalog
            });
            outcomes.push({ index: candidate.index, proposal });
            break;
          } catch (error) {
            outcomes.push({ index: candidate.index, candidate: candidate.prepared, error });
          }
        }
        return outcomes;
      }
    );
    if (timing) timing.analysisMs = performance.now() - analysisStartedAt;
    const outcomes = [...preparationOutcomes, ...analyzedGroups.flat()]
      .sort((left, right) => left.index - right.index);
    for (const outcome of outcomes) {
      if (outcome.proposal) {
        proposals.push(outcome.proposal);
        if (timing) {
          timing.analyzedTargets += 1;
          timing.targets.push({
            entrypoint: outcome.proposal.entrypoint,
            conditions: outcome.proposal.conditions,
            analysisMs: outcome.proposal.analysisDurationMs
          });
        }
      } else {
        const { entrypoint, conditions } = outcome.candidate;
        refusals.push({
          entrypoint,
          conditions,
          stage: "artifact-case",
          applicability: artifactApplicabilityForRefusal(outcome.error),
          reason: stableRefusalReason(outcome.error, { packageRoot, scratch })
        });
      }
    }
    if (proposals.length === 0) {
      // The benchmark and row ledger need the complete artifact-case census,
      // not only the first refusal repeated in the thrown message. Persist the
      // structured audit before taking the full-refusal exit.
      writeProposalRefusalAudit(output, manifest, refusals, inapplicable);
      const first = refusals[0];
      // When nothing refused, the refusal clause names no cause at all and the
      // signature is unclassifiable. Name the first inapplicable class and
      // reason instead, so an all-inapplicable census still says why.
      const firstInapplicable = inapplicable[0];
      throw new Error(
        `no certifiable artifact case; ${refusals.length} case(s) refused` +
          (inapplicable.length
            ? ` and ${inapplicable.length} case(s) recorded inapplicable`
            : "") +
          (first
            ? `; first refusal: ${first.entrypoint}: ${first.reason}`
            : firstInapplicable
              ? `; first inapplicable: ${firstInapplicable.entrypoint}: ` +
                `${firstInapplicable.class}: ${firstInapplicable.reason}`
              : "")
      );
    }
    mkdirSync(dirname(output), { recursive: true });
    const merge = async (inputs, document, plan) =>
      checked(
        [
          ...inputs.flatMap(proposal => ["--merge-contract", proposal.document]),
          "--merge-contract-output",
          document,
          ...inputs.flatMap(proposal => ["--merge-proposal-plan", proposal.plan]),
          "--merge-proposal-plan-output",
          plan
        ],
        packageRoot
      );
    const mergeStartedAt = performance.now();
    try {
      if (privateGraphPreparation && proposals.length === 1) {
        // The proposal remains untrusted until the one native graph
        // transaction re-decodes it, replays its archive/resolution, and
        // certifies its exact case. Avoid two extra native processes merely
        // to merge a singleton and validate Rust's just-emitted open bytes.
        copyFileSync(proposals[0].document, output);
        copyFileSync(proposals[0].plan, `${output}.proposal.json`);
        emittedArtifactCases = 1;
        certificationProposals = proposals;
      } else try {
        await merge(proposals, output, `${output}.proposal.json`);
        emittedArtifactCases = proposals.length;
        certificationProposals = proposals;
      } catch (rootFailure) {
        // One contradictory proposal must not erase unrelated exact cases. The
        // fallback preserves greedy input-order semantics, but accepts a whole
        // valid interval at once and bisects only failed intervals. This avoids
        // reparsing an ever-growing prefix once per wildcard entrypoint.
        const fallback = await retainIndependentlyMergeableProposalBatches(
          proposals,
          async (accepted, candidates, { start, end }) => {
            const nextDocument = join(scratch, `merge-${start}-${end}.json`);
            const nextPlan = `${nextDocument}.proposal.json`;
            const proposalsToMerge = [...accepted, ...candidates];
            await merge(proposalsToMerge, nextDocument, nextPlan);
            return {
              document: nextDocument,
              plan: nextPlan,
              certificationProposals: proposalsToMerge
            };
          },
          rootFailure
        );
        for (const { candidate, error } of fallback.rejected) {
          refusals.push({
            entrypoint: candidate.entrypoint,
            conditions: candidate.conditions,
            stage: "proposal-merge",
            applicability: ARTIFACT_APPLICABILITY.RuntimeModule,
            reason: stableRefusalReason(error, { packageRoot, scratch })
          });
        }
        if (!fallback.merged) {
          writeProposalRefusalAudit(output, manifest, refusals, inapplicable);
          throw new Error("no independently mergeable artifact case remains");
        }
        emittedArtifactCases = fallback.acceptedCount;
        certificationProposals = fallback.merged.certificationProposals;
        copyFileSync(fallback.merged.document, output);
        copyFileSync(fallback.merged.plan, `${output}.proposal.json`);
      }
    } finally {
      if (timing) timing.mergeMs = performance.now() - mergeStartedAt;
    }
    const validationStartedAt = performance.now();
    if (!privateGraphPreparation) {
      await checked(["--validate-contract", output], packageRoot);
    }
    if (timing) timing.validationMs = performance.now() - validationStartedAt;
    writeProposalRefusalAudit(output, manifest, refusals, inapplicable);
  } finally {
    rmSync(scratch, { recursive: true, force: true });
  }
  const result = {
    package: manifest.name,
    version: manifest.version,
    output,
    plan: `${output}.proposal.json`,
    schemaVersion: 1,
    entrypoints: entrypoints.length,
    artifactCases: emittedArtifactCases,
    refusedArtifactCases: refusals.length,
    inapplicableArtifactCases: inapplicable.length,
    certificationInputs: certificationProposals.map(proposal => ({
      entrypoint: proposal.entrypoint,
      conditions: proposal.conditions,
      resolution: proposal.resolution
    })),
    accepted: false
  };
  writeCertificationInputs(output, result.plan, {
    manifest,
    integrity: options.integrity,
    packageRoot,
    certificationImporter: options.certificationImporter,
    entrypoints: options.entrypoints,
    conditions: options.conditions,
    certificationInputs: result.certificationInputs
  });
  if (!quiet) {
    process.stdout.write(
      `generated unaccepted stable contract proposal for ${manifest.name}@${manifest.version} at ${output}` +
        (refusals.length
          ? `; ${refusals.length} artifact case(s) refused and omitted`
          : "") +
        (inapplicable.length
          ? `; ${inapplicable.length} artifact case(s) recorded inapplicable`
          : "") +
        "; proof verification must issue its receipt\n"
    );
  }
  if (timing) {
    timing.totalMs = performance.now() - generationStartedAt;
    emitGenerationTiming(timing);
  }
  return result;
}
