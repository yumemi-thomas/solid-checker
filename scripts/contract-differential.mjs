#!/usr/bin/env bun

// Temporary-v2 differential audit:
//   1. analyze a package implementation as project source;
//   2. generate an unaccepted normalized proposal and proposal plan;
//   3. replay the repository-owned source census as a complete proof
//      transcript, producing a receipt-issued contract;
//   4. bind that exact contract to the installed artifact through an accepted
//      catalog and compare consumer findings with the source findings.
//
// JavaScript only orchestrates exact artifacts and opaque workflow documents.
// Rust owns proposal semantics, closure, proof replay, receipt issuance, and
// the accepted analyzer index.

import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";

import { resolvePackageArtifacts } from "../packages/cli/scripts/artifact-resolution.mjs";

const root = resolve(import.meta.dirname, "..");
const checker = process.env.SOLID_CHECKER_NATIVE_BIN ?? join(root, "rust/target/debug/solid-checker-rust");
const typeFacts = process.env.SOLID_TYPEFACTS_BIN ?? join(root, "bin/solid-typefacts");
const auditedSolid = join(root, "rust/target/tsc-oracle/v2/node_modules/solid-js");
const cli = join(root, "packages/cli/bin/solid-checker.mjs");

if (!existsSync(checker) || !existsSync(typeFacts) || !existsSync(auditedSolid)) {
  throw new Error(
    `contract differential needs checker, TypeFacts, and the provisioned v2 solid-js package (${checker}, ${typeFacts}, ${auditedSolid})`
  );
}

const PROOF_FAMILIES = [
  "package-identity",
  "manifest-entrypoint",
  "export-resolution",
  "artifact-declarations",
  "export-identity",
  "module-closure",
  "selected-signature",
  "argument-binding",
  "rest-spread-coverage",
  "callable-path",
  "operation-reachability",
  "operation-cardinality",
  "recursive-value-shape",
  "guard-partition",
  "compiler-reconciliation",
  "accepted-dependency-composition",
  "domain-exhaustiveness",
  "probe-consistency"
];

function write(path, contents) {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, contents);
}

function writeJson(path, value) {
  write(path, `${JSON.stringify(value, null, 2)}\n`);
}

function linkSolid(projectRoot) {
  mkdirSync(join(projectRoot, "node_modules"), { recursive: true });
  symlinkSync(auditedSolid, join(projectRoot, "node_modules/solid-js"), "dir");
}

function runCli(arguments_, cwd) {
  return execFileSync(process.execPath, [cli, ...arguments_], {
    cwd,
    encoding: "utf8",
    env: {
      ...process.env,
      SOLID_CHECKER_NATIVE_BIN: checker,
      SOLID_TYPEFACTS_BIN: typeFacts
    },
    maxBuffer: 32 * 1024 * 1024
  });
}

function analyze(project) {
  const output = execFileSync(
    checker,
    ["--format", "json", "--project", join(project, "tsconfig.json")],
    {
      cwd: root,
      encoding: "utf8",
      env: { ...process.env, SOLID_TYPEFACTS_BIN: typeFacts },
      maxBuffer: 32 * 1024 * 1024
    }
  );
  const snapshot = JSON.parse(output);
  return (snapshot.findings ?? [])
    .map(finding => ({
      rule: finding.rule,
      id: finding.id,
      kind: finding.kind,
      severity: finding.severity,
      analysisContext: finding.analysisContext ?? null
    }))
    .sort((left, right) => JSON.stringify(left).localeCompare(JSON.stringify(right)));
}

function projectConfig(include) {
  return {
    compilerOptions: {
      allowJs: true,
      checkJs: true,
      jsx: "preserve",
      module: "ESNext",
      moduleResolution: "Bundler",
      skipLibCheck: true,
      strict: true,
      target: "ES2022"
    },
    include
  };
}

function applicationSource(importPath) {
  return `import { createSignal } from "solid-js";\nimport { runInline } from ${JSON.stringify(importPath)};\n\nconst [count] = createSignal(0);\nfunction readCount() {\n  return count();\n}\n\nrunInline(readCount);\n`;
}

function createSourceProject(directory) {
  linkSolid(directory);
  writeJson(join(directory, "tsconfig.json"), projectConfig(["*.ts", "*.tsx"]));
  write(
    join(directory, "implementation.ts"),
    `export function runInline(callback: () => unknown) {\n  callback();\n}\n`
  );
  write(join(directory, "App.tsx"), applicationSource("./implementation"));
}

function createPackage(packageRoot, solidVersion) {
  writeJson(join(packageRoot, "package.json"), {
    name: "differential-package",
    version: "1.0.0",
    type: "module",
    exports: { ".": { types: "./index.d.ts", import: "./index.js" } },
    dependencies: { "solid-js": solidVersion }
  });
  write(
    join(packageRoot, "index.js"),
    `export function runInline(callback) {\n  callback();\n}\n`
  );
  write(
    join(packageRoot, "index.d.ts"),
    "export declare function runInline(callback: () => unknown): void;\n"
  );
}

function assertProposalSemantics(document) {
  assert.equal(document.schemaVersion, 2);
  assert.equal(document.semanticModelVersion, 1);
  const artifact = document.entrypoints?.["."]?.cases?.[0];
  const summary = document.summaries?.[artifact?.exports?.runInline];
  assert.ok(summary, "proposal must preserve exact runInline export identity");
  const operations = new Map((summary.call?.operations ?? []).map(operation => [operation.id, operation]));
  const schedules = (summary.call?.callbacks ?? [])
    .filter(callback => callback.from?.arg === 0 && callback.from?.path?.length === 0)
    .map(callback => operations.get(callback.operation)?.at?.schedule)
    .sort();
  assert.deepEqual(
    schedules,
    ["same-stack"],
    "proposal must retain the same-stack callback operation"
  );
}

function proofFor(plan, sourceCensus) {
  return {
    format: "solid-checker-contract-proof-transcript",
    proofVersion: 1,
    semanticModelVersion: plan.semanticModelVersion,
    semanticDigest: plan.semanticDigest,
    verifierBuild: "solid-checker-contract-differential-v2",
    claims: plan.closureCandidates.map(claim => ({
      claimId: claim.claimId,
      subject: claim.subject,
      families: PROOF_FAMILIES.map(family => ({
        family,
        transcript: `repository differential source census ${sourceCensus}; ${claim.claimId}; ${family}`,
        enumerated: [sourceCensus],
        classified: [sourceCensus],
        unresolved: [],
        complete: true
      }))
    })),
    probeContradictions: []
  };
}

const temporary = mkdtempSync(join(tmpdir(), "solid-checker-contract-differential-"));
try {
  const source = join(temporary, "source");
  const consumer = join(temporary, "consumer");
  const packageRoot = join(temporary, "package-artifact");
  const workflow = join(temporary, "workflow");
  const proposalPath = join(workflow, "proposal.json");
  const planPath = `${proposalPath}.proposal.json`;
  const proofPath = join(workflow, "proof.json");
  const acceptedDirectory = join(consumer, ".solid-checker");
  const acceptedPath = join(acceptedDirectory, "differential.accepted.json");
  const receiptPath = join(acceptedDirectory, "differential.receipt.json");
  const solidVersion = JSON.parse(readFileSync(join(auditedSolid, "package.json"), "utf8")).version;
  const integrity = `sha512-${createHash("sha512").update("solid-checker differential package v2").digest("base64")}`;

  createSourceProject(source);
  createPackage(packageRoot, solidVersion);
  linkSolid(packageRoot);
  runCli(
    [
      "contract",
      "generate",
      "--package-root",
      packageRoot,
      "--integrity",
      integrity,
      "--output",
      proposalPath
    ],
    packageRoot
  );

  const proposal = JSON.parse(readFileSync(proposalPath, "utf8"));
  const plan = JSON.parse(readFileSync(planPath, "utf8"));
  assertProposalSemantics(proposal);
  assert.ok(plan.closureCandidates.length > 0, "differential proposal must expose proof candidates");
  const artifactCases = new Set(plan.closureCandidates.map(claim => claim.subject.artifactCase));
  assert.equal(artifactCases.size, 1, "differential package must resolve one exact artifact case");
  const sourceCensus = `sha256:${createHash("sha256")
    .update(readFileSync(join(source, "implementation.ts")))
    .digest("hex")}`;
  writeJson(proofPath, proofFor(plan, sourceCensus));
  mkdirSync(acceptedDirectory, { recursive: true });
  runCli(
    [
      "contract",
      "verify",
      proposalPath,
      "--plan",
      planPath,
      "--proof",
      proofPath,
      "--artifact-case",
      [...artifactCases][0],
      "--output",
      acceptedPath,
      "--receipt",
      receiptPath
    ],
    packageRoot
  );

  linkSolid(consumer);
  symlinkSync(packageRoot, join(consumer, "node_modules/differential-package"), "dir");
  writeJson(join(consumer, "tsconfig.json"), projectConfig(["App.tsx"]));
  write(join(consumer, "App.tsx"), applicationSource("differential-package"));
  const resolvedImport = resolvePackageArtifacts({
    importer: join(consumer, "App.tsx"),
    specifier: "differential-package",
    packageRoot: join(consumer, "node_modules/differential-package"),
    conditions: ["import"],
    resolutionKind: "import",
    integrity
  });
  writeJson(join(acceptedDirectory, "accepted-contracts.json"), {
    format: "solid-checker-accepted-contract-catalog",
    catalogVersion: 1,
    contracts: [
      {
        document: ".solid-checker/differential.accepted.json",
        receipt: ".solid-checker/differential.receipt.json",
        import: resolvedImport
      }
    ]
  });

  const sourceFindings = analyze(source);
  const consumerFindings = analyze(consumer);
  assert.deepEqual(
    consumerFindings,
    sourceFindings,
    `accepted temporary-v2 boundary changed semantic findings\nsource: ${JSON.stringify(sourceFindings)}\nconsumer: ${JSON.stringify(consumerFindings)}`
  );
  console.log(
    `contract differential: source and receipt-issued temporary-v2 consumers agree (${sourceFindings.length} finding${sourceFindings.length === 1 ? "" : "s"})`
  );
} finally {
  rmSync(temporary, { recursive: true, force: true });
}
