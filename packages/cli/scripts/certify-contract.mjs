// Policy-2 certification transaction.
//
// Node owns exact registry acquisition, temporary files, producer process
// lifecycle, and the final publication transaction. Rust owns proposal
// semantics, demand derivation, witness verification, accepted bytes, and the
// catalog bytes. Audit output is diagnostic only and is never an authority
// input to this command.

import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import process from "node:process";

import { runNativeAsync } from "../bin/launcher.mjs";
import { generatePackageContract } from "./generate-package-contract.mjs";

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
  --registry-origin <URL> Exact HTTPS registry origin (default: npm registry)
  --entrypoint <SUBPATH>  Exact exported subpath (repeatable)
  --conditions <LIST>     Exact runtime conditions, comma-separated
  --audit-output <FILE>   Write a non-replayable diagnostic transcript
  -h, --help              Show this help
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
    auditOutput: "",
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

async function acquirePublishedArtifact({ options, manifest, scratch, fetch_ = fetch }) {
  const metadataResponse = await fetch_(
    exactRegistryPackageUrl(options.registryOrigin, manifest.name),
    { headers: { accept: "application/json" } }
  );
  const metadataBytes = await checkedResponse(metadataResponse, "registry metadata acquisition");
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
  const archiveResponse = await fetch_(selected.dist.tarball, {
    headers: { accept: "application/octet-stream" }
  });
  const archiveBytes = await checkedResponse(archiveResponse, "package archive acquisition");
  const metadataPath = join(scratch, "registry-metadata.json");
  const archivePath = join(scratch, "package.tgz");
  writeFileSync(metadataPath, metadataBytes);
  writeFileSync(archivePath, archiveBytes);
  return Object.freeze({
    registryOrigin: options.registryOrigin,
    metadataPath,
    archivePath,
    package: manifest.name,
    version: manifest.version,
    integrity: options.integrity
  });
}

async function planDemands({ options, generated, artifactSnapshot, scratch }) {
  const plans = [];
  for (const [index, input] of generated.certificationInputs.entries()) {
    const requestPath = join(scratch, `certification-request-${index}.json`);
    const outputPath = join(scratch, `certification-plan-${index}.json`);
    writeFileSync(
      requestPath,
      `${JSON.stringify(
        {
          schemaVersion: 1,
          proposal: generated.output,
          resolution: input.resolution,
          exportConditions: [...new Set([...input.conditions, "import"])].sort(),
          registryOrigin: artifactSnapshot.registryOrigin,
          registryMetadata: artifactSnapshot.metadataPath,
          archive: artifactSnapshot.archivePath
        },
        null,
        2
      )}\n`
    );
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

function writeAudit(path, manifest, refusal, demandPlans) {
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

export async function certifyContract(arguments_, { fetch_ = fetch } = {}) {
  const options = parseCertifyArguments(arguments_);
  if (options.help) {
    process.stdout.write(contractCertifyHelp);
    return;
  }
  options.packageRoot = resolve(options.packageRoot);
  options.catalog = resolve(
    options.catalog || join(options.packageRoot, ".solid-checker", "accepted-contracts.json")
  );
  const manifest = JSON.parse(readFileSync(join(options.packageRoot, "package.json"), "utf8"));
  if (!manifest.name || !manifest.version) {
    throw new Error("package.json must declare an exact package name and version");
  }
  const scratch = mkdtempSync(join(tmpdir(), "solid-checker-certify-"));
  const demandPlans = [];
  try {
    await runContractCertificationPipeline({
      request: options,
      acquisition: {
        acquireArtifacts: async () =>
          acquirePublishedArtifact({ options, manifest, scratch, fetch_ })
      },
      proposal: {
        generate: async () => {
          const generationArguments = [
            "--package-root",
            options.packageRoot,
            "--output",
            join(scratch, "solid-reactivity.json"),
            "--integrity",
            options.integrity,
            ...options.entrypoints.flatMap(entrypoint => ["--entrypoint", entrypoint])
          ];
          if (options.conditions.length) {
            generationArguments.push("--conditions", options.conditions.join(","));
          }
          const generated = await generatePackageContract(generationArguments, { quiet: true });
          return { authority: "rust", generated };
        }
      },
      rust: {
        planDemands: async ({ artifactSnapshot, openProposal }) => {
          const plans = await planDemands({
            options,
            generated: openProposal.generated,
            artifactSnapshot,
            scratch
          });
          demandPlans.push(...plans);
          return { authority: "rust", plans };
        },
        certify: async () => {
          throw new CertificationRefusal({
            stage: "certification",
            owner: "certifier",
            reason: "the complete authenticated witness set was not constructed"
          });
        }
      },
      evidence: {
        // Artifact-wide bindings were constructed inside Rust's opaque plan.
        // The remaining adapters fail closed until each can return live
        // authenticated typestate instead of caller-authored JSON.
        obtainWitnesses: async ({ demandPlan }) => unavailableWitnessRefusal(demandPlan.plans)
      },
      issuer: {
        issue: async () => {
          throw new CertificationRefusal({
            stage: "receipt-issuance",
            owner: "trust",
            reason: "no configured policy-2 receipt issuer is available"
          });
        }
      },
      publication: {
        commit: async () => {
          throw new Error("catalog publication was reached without a complete policy-2 result");
        }
      }
    });
  } catch (error) {
    const refusal =
      error instanceof CertificationRefusal
        ? error
        : new CertificationRefusal({
            stage: demandPlans.length ? "witness-acquisition" : "artifact-or-demand-planning",
            owner: demandPlans.length ? "certifier" : "artifact-provenance",
            reason: error instanceof Error ? error.message : String(error)
          });
    writeAudit(options.auditOutput, manifest, refusal, demandPlans);
    throw refusal;
  } finally {
    rmSync(scratch, { recursive: true, force: true });
  }
}
