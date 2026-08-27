// Replacement package-contract proposal orchestration.
//
// Node owns acquisition, temporary/process lifecycle, and writing bytes that
// Rust already produced. Semantic analysis, proposal construction, proof
// planning, and probe planning are explicit Rust calls. This module never
// reads or merges a semantic summary.

import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";

import { resolvePackageArtifacts } from "./artifact-resolution.mjs";

export const contractProposalStages = Object.freeze([
  "packageDiscovery",
  "artifactResolution",
  "semanticAnalysis",
  "proposalConstruction",
  "proofPlanning",
  "probePlanning",
  "emission"
]);

function requireFunction(owner, name) {
  const operation = owner?.[name];
  if (typeof operation !== "function") {
    throw new TypeError(`contract proposal stage ${name} is not configured`);
  }
  return operation;
}

function requireRustProduct(value, stage) {
  if (!value || typeof value !== "object" || value.authority !== "rust") {
    throw new TypeError(`contract proposal stage ${stage} must return a Rust-owned product`);
  }
  return value;
}

/// Phase 7 standalone acquisition adapter for the replacement pipeline. It
/// produces the exact manifest and independently resolved runtime/declaration
/// artifact record consumed by Rust proposal construction.
export function standaloneProposalAcquisition() {
  return {
    async discoverPackage(request) {
      const packageRoot = resolve(request.packageRoot);
      const manifestPath = join(packageRoot, "package.json");
      const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
      if (!manifest.name || !manifest.version) {
        throw new TypeError(`${manifestPath} must declare an exact package name and version`);
      }
      return Object.freeze({ packageRoot, manifestPath, manifest });
    },
    async resolveArtifacts({ request, packageDiscovery }) {
      return resolvePackageArtifacts({
        importer: request.importer,
        specifier: request.specifier,
        packageRoot: packageDiscovery.packageRoot,
        conditions: request.conditions ?? [],
        resolutionKind: request.resolutionKind ?? "import",
        integrity: request.integrity,
        acceptedDependencies: request.acceptedDependencies ?? {}
      });
    }
  };
}

/// Runs every replacement-generator stage exactly once and in dependency
/// order. The stage adapters are injected so this seam can be tested without
/// launching package code or manufacturing semantic fixtures in JavaScript.
export async function runContractProposalPipeline({
  request,
  acquisition,
  rust,
  output
}) {
  const packageDiscovery = await requireFunction(acquisition, "discoverPackage")(request);
  const artifactResolution = await requireFunction(acquisition, "resolveArtifacts")({
    request,
    packageDiscovery
  });
  const semanticAnalysis = requireRustProduct(
    await requireFunction(rust, "analyze")({
      request,
      packageDiscovery,
      artifactResolution
    }),
    "semanticAnalysis"
  );
  const proposalConstruction = requireRustProduct(
    await requireFunction(rust, "constructProposal")({ semanticAnalysis }),
    "proposalConstruction"
  );
  const proofPlanning = requireRustProduct(
    await requireFunction(rust, "planProofs")({ proposalConstruction }),
    "proofPlanning"
  );
  const probePlanning = requireRustProduct(
    await requireFunction(rust, "planProbes")({ proposalConstruction, proofPlanning }),
    "probePlanning"
  );
  return await requireFunction(output, "emit")({
    request,
    packageDiscovery,
    artifactResolution,
    semanticAnalysis,
    proposalConstruction,
    proofPlanning,
    probePlanning
  });
}
