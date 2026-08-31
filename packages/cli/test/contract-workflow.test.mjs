import assert from "node:assert/strict";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "vitest";

import { createRuntimeProbeHarness } from "../scripts/contract-probe-harness.mjs";
import { ArtifactResolutionError } from "../scripts/artifact-resolution.mjs";
import {
  buildPublishedGraphExecutionRequest,
  CertificationRefusal,
  certifyContract,
  isExactDependencyCompositionRefusal,
  isReusableDependencyRefusalAudit,
  parseCertifyArguments,
  publishedGraphPreparationConcurrency,
  runContractCertificationPipeline,
  validatedReusableDependencyRefusalAuditBytes
} from "../scripts/certify-contract.mjs";
import { parseProbeArguments } from "../scripts/probe-contract.mjs";
import { parseReviewArguments } from "../scripts/review-contract.mjs";
import { parseVerifyArguments } from "../scripts/verify-contract.mjs";
import {
  ARTIFACT_CASE_CANDIDATE_LIMIT,
  ARTIFACT_APPLICABILITY,
  artifactAnalysisBatchConcurrencyLimit,
  artifactApplicabilityForRefusal,
  finiteArtifactCandidates,
  finiteConditionPartitions,
  finiteEntrypoints,
  generatePackageContract,
  partitionArtifactAnalysisBatches,
  recommendedArtifactAnalysisBatchConcurrency,
  retainIndependentlyMergeableProposalBatches,
  retainIndependentlyMergeableProposals
} from "../scripts/generate-package-contract.mjs";

test("artifact analysis batches only compatible demands under a bounded target count", () => {
  const candidates = Array.from({ length: 35 }, (_, index) => ({
    index,
    prepared: {
      conditions: index < 33 ? ["import"] : ["browser", "import"],
      resolution: {
        packageRoot: "/package",
        runtime: { path: "/package/index.ts" },
        closure: { entries: [] }
      }
    }
  }));
  const batches = partitionArtifactAnalysisBatches(candidates, 16);
  assert.deepEqual(batches.map(batch => batch.length), [16, 16, 1, 2]);
  assert.ok(batches.every(batch =>
    batch.every(candidate =>
      JSON.stringify(candidate.prepared.conditions) ===
        JSON.stringify(batch[0].prepared.conditions)
    )
  ));
  assert.throws(
    () => partitionArtifactAnalysisBatches(candidates, 0),
    /positive integer/
  );

  const incompatiblePrograms = candidates.slice(0, 2).map((candidate, index) => ({
    ...candidate,
    prepared: {
      ...candidate.prepared,
      resolution: {
        ...candidate.prepared.resolution,
        runtime: { path: `/package/entry-${index}.ts` }
      }
    }
  }));
  assert.deepEqual(
    partitionArtifactAnalysisBatches(incompatiblePrograms, 16).map(batch => batch.length),
    [1, 1]
  );
});

test("artifact analysis batch fanout grows only for genuinely wide exact demand sets", () => {
  assert.equal(recommendedArtifactAnalysisBatchConcurrency(0, 14), 1);
  assert.equal(recommendedArtifactAnalysisBatchConcurrency(1, 14), 1);
  assert.equal(recommendedArtifactAnalysisBatchConcurrency(31, 14), 1);
  assert.equal(recommendedArtifactAnalysisBatchConcurrency(32, 14), 2);
  assert.equal(recommendedArtifactAnalysisBatchConcurrency(127, 14), 2);
  assert.equal(recommendedArtifactAnalysisBatchConcurrency(128, 14), 4);
  assert.equal(recommendedArtifactAnalysisBatchConcurrency(511, 14), 4);
  assert.equal(recommendedArtifactAnalysisBatchConcurrency(512, 14), 8);
  assert.equal(recommendedArtifactAnalysisBatchConcurrency(512, 4), 4);
  assert.equal(recommendedArtifactAnalysisBatchConcurrency(512, 1), 1);
  assert.equal(recommendedArtifactAnalysisBatchConcurrency(512, Number.NaN), 1);
  assert.equal(recommendedArtifactAnalysisBatchConcurrency(512, 14, 2), 2);
  assert.throws(
    () => recommendedArtifactAnalysisBatchConcurrency(-1, 14),
    /non-negative integer/
  );
  assert.throws(
    () => recommendedArtifactAnalysisBatchConcurrency(512, 14, 0),
    /positive integer/
  );
});

test("artifact analysis batch fanout accepts only a bounded positive environment cap", () => {
  assert.equal(artifactAnalysisBatchConcurrencyLimit({}), 8);
  assert.equal(artifactAnalysisBatchConcurrencyLimit({
    SOLID_CHECKER_ARTIFACT_ANALYSIS_BATCH_CONCURRENCY: "2"
  }), 2);
  assert.equal(artifactAnalysisBatchConcurrencyLimit({
    SOLID_CHECKER_ARTIFACT_ANALYSIS_BATCH_CONCURRENCY: "24"
  }), 8);
  assert.throws(
    () => artifactAnalysisBatchConcurrencyLimit({
      SOLID_CHECKER_ARTIFACT_ANALYSIS_BATCH_CONCURRENCY: "0"
    }),
    /positive integer/
  );
});

const proofPolicy = JSON.parse(readFileSync(
  new URL("../../../docs/package-contract-v2/phase19/proof-policy-v2.json", import.meta.url),
  "utf8"
));

test("artifact refusals carry verifier-owned applicability classes", () => {
  const classify = (code, message = code) =>
    artifactApplicabilityForRefusal(new ArtifactResolutionError(code, message));
  assert.equal(classify("target-not-found"), ARTIFACT_APPLICABILITY.MissingPublishedTarget);
  assert.equal(classify("conditions-unmatched"), ARTIFACT_APPLICABILITY.UnsupportedConditionSet);
  assert.equal(classify("declarations-not-found"), ARTIFACT_APPLICABILITY.UnsupportedArtifactShape);
  assert.equal(
    classify("module-not-found", "local closure module ./absent.js was not found"),
    ARTIFACT_APPLICABILITY.MissingPublishedTarget
  );
  assert.equal(
    classify("module-not-found", "dependency under node_modules/pkg is unsupported"),
    ARTIFACT_APPLICABILITY.UnsupportedArtifactShape
  );
  assert.equal(
    artifactApplicabilityForRefusal(new Error("semantic refusal")),
    ARTIFACT_APPLICABILITY.RuntimeModule
  );
});

test("review exposes only stable proposal inspection", () => {
  assert.deepEqual(parseReviewArguments(["proposal.json", "--output", "review.json"]), {
    proposal: "proposal.json",
    output: "review.json",
    help: false
  });
  assert.throws(() => parseReviewArguments(["proposal.json", "--promote", "reviewed"]), /unknown/);
});

test("policy-1 caller-proof verification is retired", () => {
  assert.deepEqual(parseVerifyArguments(["--help"]), { help: true });
  assert.throws(
    () => parseVerifyArguments(["proposal.json"]),
    /proof-file issuance was retired/
  );
});

test("policy-2 certification accepts no caller-authored proof or receipt input", () => {
  assert.equal(
    parseCertifyArguments(["--integrity", "sha512-cGlubmVk"]).integrity,
    "sha512-cGlubmVk"
  );
  assert.throws(() => parseCertifyArguments([]), /--integrity is required/);
  assert.throws(
    () => parseCertifyArguments(["--integrity", "sha512-cGlubmVk", "--proof", "proof.json"]),
    /unknown contract certification argument --proof/
  );
  assert.throws(
    () => parseCertifyArguments(["--integrity", "sha512-cGlubmVk", "--receipt", "receipt.json"]),
    /unknown contract certification argument --receipt/
  );
});

test("ordinary dependency-aware generation still requires authenticated analyzer input", async () => {
  await assert.rejects(
    generatePackageContract(["--integrity", "sha512-fixture"], {
      quiet: true,
      acceptedDependencies: {
        dependency: {
          packageName: "dependency",
          artifactCase: "sha256:" + "a".repeat(64),
          acceptedContractDigest: "sha256:" + "b".repeat(64)
        }
      }
    }),
    /require an authenticated contract catalog and trust configuration/
  );
});

test("private graph proposal dependencies require proposal material but no receipt authority", async () => {
  await assert.rejects(
    generatePackageContract(["--integrity", "sha512-fixture"], {
      quiet: true,
      proposalDependencies: {
        dependency: {
          packageName: "dependency",
          artifactCase: "sha256:" + "a".repeat(64),
          acceptedContractDigest: "sha256:" + "b".repeat(64)
        }
      }
    }),
    /require a private proposal dependency catalog/
  );
});

test("external import-then-export binding refusals enter graph acquisition", () => {
  assert.equal(
    isExactDependencyCompositionRefusal({
      reason: "accepted dependency @corvu/disclosure has no exact runtime binding for export useContext"
    }),
    true
  );
  assert.equal(
    isExactDependencyCompositionRefusal({ reason: "resolved artifact is missing a local export" }),
    false
  );
});

test("proposal refusal reuse requires a complete current exact dependency refusal census", () => {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-refusal-reuse-"));
  const manifest = {
    name: "refusal-reuse-fixture",
    version: "1.0.0",
    type: "module",
    exports: {
      ".": { types: "./index.d.ts", import: "./index.js" },
      "./secondary": { types: "./secondary.d.ts", import: "./secondary.js" }
    }
  };
  const refusal = {
    entrypoint: ".",
    conditions: [],
    stage: "artifact-case",
    applicability: "runtime-module",
    reason: "accepted dependency dependency has no exact runtime binding for export default"
  };
  const secondaryRefusal = { ...refusal, entrypoint: "./secondary" };
  const audit = {
    format: "solid-checker-contract-proposal-refusals",
    refusalVersion: 1,
    package: { name: manifest.name, version: manifest.version },
    refusals: [refusal, secondaryRefusal]
  };
  const validation = {
    manifest,
    packageRoot: root,
    integrity: "sha512-fixture",
    certificationImporter: join(root, "certification-importer.mjs")
  };
  const validate = candidate => isReusableDependencyRefusalAudit({
    audit: candidate,
    ...validation
  });
  writeFileSync(join(root, "package.json"), JSON.stringify(manifest));
  writeFileSync(join(root, "index.js"), 'export { default } from "dependency";\n');
  writeFileSync(join(root, "index.d.ts"), 'export { default } from "dependency";\n');
  writeFileSync(join(root, "secondary.js"), 'export { default } from "dependency";\n');
  writeFileSync(join(root, "secondary.d.ts"), 'export { default } from "dependency";\n');
  writeFileSync(join(root, "certification-importer.mjs"), "export {};\n");
  try {
    assert.equal(validate(audit), true);
    assert.equal(validate({ ...audit, refusals: [] }), false);
    assert.equal(validate({ ...audit, refusals: [refusal, refusal] }), false);
    assert.equal(
      validate({ ...audit, refusals: [refusal] }),
      false,
      "an incomplete current artifact-case census cannot be reused"
    );
    assert.equal(
      validate({ ...audit, package: { ...audit.package, version: "1.0.1" } }),
      false
    );
    assert.equal(
      validate({
        ...audit,
        refusals: [
          { ...refusal, reason: "resolved artifact is missing a local export" },
          secondaryRefusal
        ]
      }),
      false
    );
    assert.equal(
      validate({
        ...audit,
        refusals: [{ ...refusal, stage: "proposal-merge" }, secondaryRefusal]
      }),
      false
    );

    const retainedBytes = Buffer.from(`${JSON.stringify(audit)}\n`);
    assert.strictEqual(
      validatedReusableDependencyRefusalAuditBytes({
        auditBytes: retainedBytes,
        ...validation
      }),
      retainedBytes,
      "the exact parsed bytes are retained for the scratch census"
    );
    assert.equal(
      validatedReusableDependencyRefusalAuditBytes({
        auditBytes: Buffer.from("{ malformed"),
        ...validation
      }),
      null,
      "malformed diagnostic input falls back to ordinary proposal generation"
    );

    writeFileSync(join(root, "index.js"), "export default function current() {}\n");
    writeFileSync(join(root, "index.d.ts"), "export default function current(): void;\n");
    assert.equal(validate(audit), false, "source changes invalidate the earlier refusal");
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("published graph preparation uses a bounded run-wide worker cap", () => {
  assert.equal(publishedGraphPreparationConcurrency({}), 8);
  assert.equal(
    publishedGraphPreparationConcurrency({ SOLID_CHECKER_GRAPH_CONCURRENCY: "2" }),
    2
  );
  assert.throws(
    () => publishedGraphPreparationConcurrency({ SOLID_CHECKER_GRAPH_CONCURRENCY: "0" }),
    /positive integer/
  );
});

test("published graph execution transports exact lock bytes and no caller receipt authority", () => {
  const state = (name, locator, sourceDependencies = []) => ({
    node: { bunLockPath: "/project/bun.lock", lockLocator: locator },
    planning: {
      schemaVersion: 1,
      proposal: `/scratch/${name}.json`,
      resolution: { packageName: name }
    },
    sourceDependencies
  });
  const leaf = state("leaf", "leaf@2.0.0");
  const root = state("root", "root@1.0.0", [{
    packageName: "types-only",
    packageVersion: "3.0.0",
    registryOrigin: "https://registry.npmjs.org",
    registryMetadata: "/scratch/types-only-metadata.json",
    archive: "/scratch/types-only.tgz",
    lockfile: "/project/bun.lock",
    lockLocator: "types-only@3.0.0",
    installedPackageRoot: "/project/node_modules/types-only",
    callerDigest: "sha256:not-authority"
  }]);
  const execution = buildPublishedGraphExecutionRequest({
    cases: [{ root, nodes: [leaf, root] }],
    typefactsExecutable: "/bin/typefacts",
    issuerConfiguration: "/config/issuer.json",
    catalogRoot: "/catalog",
    trustConfigurationOutput: "/config/trust.json"
  });
  assert.equal(execution.schemaVersion, 3);
  assert.equal(execution.graph.root.lockLocator, "root@1.0.0");
  assert.deepEqual(execution.graph.root.sourceDependencies, [{
    packageName: "types-only",
    packageVersion: "3.0.0",
    registryOrigin: "https://registry.npmjs.org",
    registryMetadata: "/scratch/types-only-metadata.json",
    archive: "/scratch/types-only.tgz",
    lockfile: "/project/bun.lock",
    lockLocator: "types-only@3.0.0",
    installedPackageRoot: "/project/node_modules/types-only"
  }]);
  assert.deepEqual(execution.graph.dependencies.map(node => node.lockLocator), ["leaf@2.0.0"]);
  assert.equal(JSON.stringify(execution).includes("acceptedContractDigest"), false);
  assert.equal(JSON.stringify(execution).includes("callerDigest"), false);
  assert.equal(JSON.stringify(execution).includes("receipt"), false);
});

test("published graph case-set execution deduplicates canonical node transport", () => {
  const state = (key, name, locator) => ({
    node: { key, bunLockPath: "/project/bun.lock", lockLocator: locator },
    planning: {
      schemaVersion: 1,
      proposal: `/scratch/${name}.json`,
      resolution: { packageName: name }
    },
    sourceDependencies: []
  });
  const shared = state("shared-full-identity", "shared", "shared@1.0.0");
  const left = state("left-full-identity", "left", "left@1.0.0");
  const right = state("right-full-identity", "right", "right@1.0.0");
  const execution = buildPublishedGraphExecutionRequest({
    cases: [
      { root: left, nodes: [shared, left] },
      { root: right, nodes: [shared, right] }
    ],
    typefactsExecutable: "/bin/typefacts",
    issuerConfiguration: "/config/issuer.json",
    catalogRoot: "/catalog",
    trustConfigurationOutput: "/config/trust.json"
  });

  assert.equal(execution.schemaVersion, 5);
  assert.deepEqual(
    execution.graphCaseSet.nodes.map(node => node.key),
    ["left-full-identity", "right-full-identity", "shared-full-identity"]
  );
  assert.deepEqual(execution.graphCaseSet.cases, [
    { root: "left-full-identity", nodes: ["left-full-identity", "shared-full-identity"] },
    { root: "right-full-identity", nodes: ["right-full-identity", "shared-full-identity"] }
  ]);
  assert.equal(JSON.stringify(execution).includes("receipt"), false);
});

test("certification publishes only after every authority stage succeeds", async () => {
  const stages = [];
  const result = await runContractCertificationPipeline({
    request: { package: "example" },
    acquisition: {
      acquireArtifacts: async () => (stages.push("artifact"), { snapshot: "exact" })
    },
    proposal: {
      generate: async () => (stages.push("proposal"), { authority: "rust" })
    },
    rust: {
      planDemands: async () => (stages.push("demands"), { authority: "rust" }),
      certify: async () => (stages.push("certify"), { authority: "rust" })
    },
    evidence: {
      obtainWitnesses: async () => (stages.push("witnesses"), { live: true })
    },
    issuer: {
      issue: async () => (stages.push("receipt"), { authority: "configured-issuer" })
    },
    publication: {
      commit: async () => (stages.push("publish"), "published")
    }
  });
  assert.equal(result, "published");
  assert.deepEqual(stages, [
    "artifact",
    "proposal",
    "demands",
    "witnesses",
    "certify",
    "receipt",
    "publish"
  ]);
});

test("an intermediate certification refusal cannot reach catalog publication", async () => {
  let published = false;
  await assert.rejects(
    runContractCertificationPipeline({
      request: {},
      acquisition: { acquireArtifacts: async () => ({}) },
      proposal: { generate: async () => ({ authority: "rust" }) },
      rust: {
        planDemands: async () => ({ authority: "rust" }),
        certify: async () => ({ authority: "rust" })
      },
      evidence: {
        obtainWitnesses: async () => {
          throw new CertificationRefusal({
            stage: "witness-acquisition",
            owner: "probe-gate",
            demandId: "sha256:missing",
            family: "probe-consistency",
            reason: "missing live harness binding"
          });
        }
      },
      issuer: { issue: async () => ({ authority: "configured-issuer" }) },
      publication: { commit: async () => (published = true) }
    }),
    /witness-acquisition refused for demand sha256:missing/
  );
  assert.equal(published, false);
});

test("concrete acquisition failure writes only a non-replayable audit", async () => {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-certify-test-"));
  const catalog = join(root, "accepted-contracts.json");
  const audit = join(root, "audit.json");
  writeFileSync(join(root, "package.json"), '{"name":"example","version":"1.0.0"}\n');
  writeFileSync(catalog, "catalog-sentinel\n");
  const metadata = new TextEncoder().encode(
    JSON.stringify({
      versions: {
        "1.0.0": {
          name: "example",
          version: "1.0.0",
          dist: {
            integrity: "sha512-registry",
            tarball: "https://registry.npmjs.org/example/-/example-1.0.0.tgz"
          }
        }
      }
    })
  );
  const fetch_ = async () => ({
    ok: true,
    status: 200,
    arrayBuffer: async () => metadata.buffer
  });
  try {
    await assert.rejects(
      certifyContract(
        [
          "--package-root",
          root,
          "--integrity",
          "sha512-lockfile",
          "--catalog",
          catalog,
          "--audit-output",
          audit
        ],
        { fetch_ }
      ),
      /registry integrity .* disagrees/
    );
    assert.equal(readFileSync(catalog, "utf8"), "catalog-sentinel\n");
    assert.equal(existsSync(audit), true);
    const transcript = JSON.parse(readFileSync(audit, "utf8"));
    assert.equal(transcript.authoritative, false);
    assert.equal(transcript.replayable, false);
    assert.equal(transcript.status, "refused");
    assert.ok(transcript.stageDurationsMs.artifactAcquisition >= 0);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("probe parsing has no write or negative-discovery compatibility mode", () => {
  const parsed = parseProbeArguments([
    "proposal.json",
    "--request",
    "request.json",
    "--plan-only"
  ]);
  assert.equal(parsed.request, "request.json");
  assert.equal(parsed.planOnly, true);
  assert.throws(
    () => parseProbeArguments(["proposal.json", "--request", "request.json", "--write"]),
    /unknown/
  );
});

test("worker harness transports sequenced events and bounded drain counts", async () => {
  const harness = createRuntimeProbeHarness({
    drain: [
      { kind: "flush" },
      { kind: "microtasks", maxTurns: 2 },
      { kind: "macrotasks", maxTurns: 1 }
    ]
  });
  let flushed = 0;
  harness.emit({ marker: "first", kind: "call", phase: "enter" });
  harness.emit({ marker: "second", kind: "callback", ordinal: 0 });
  await harness.drain({ flush: () => (flushed += 1) });
  assert.deepEqual(harness.events().map(event => event.sequence), [0, 1]);
  assert.equal(harness.drainedMicrotasks(), 2);
  assert.equal(harness.drainedMacrotasks(), 1);
  assert.equal(flushed, 1);
});

test("finite entrypoint discovery keeps exact rows while refusing wildcard coverage", () => {
  assert.deepEqual(
    finiteEntrypoints(
      { exports: { ".": "./index.js", "./web": "./web.js", "./types/*": "./types/*.d.ts" } },
      []
    ),
    {
      entrypoints: [".", "./web"],
      wildcardRefusals: ["./types/*"],
      wildcardBranchRefusals: [],
      wildcardResourceRefusals: []
    }
  );
  assert.throws(
    () => finiteEntrypoints({ exports: { "./*": "./dist/*.js" } }, []),
    /pass each finite --entrypoint/
  );
});

test("a fully refused proposal writes every artifact-case refusal before throwing", async () => {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-full-refusal-audit-"));
  const output = join(root, "out", "proposal.json");
  writeFileSync(
    join(root, "package.json"),
    JSON.stringify({
      name: "fully-refused-fixture",
      version: "1.0.0",
      type: "module",
      exports: { ".": "./missing.js" }
    })
  );
  try {
    await assert.rejects(
      generatePackageContract(
        [
          "--package-root",
          root,
          "--integrity",
          "sha512-fixture",
          "--output",
          output
        ],
        { quiet: true }
      ),
      /no certifiable artifact case; 1 case\(s\) refused/
    );
    const audit = JSON.parse(readFileSync(`${output}.refusals.json`, "utf8"));
    assert.equal(audit.format, "solid-checker-contract-proposal-refusals");
    assert.equal(audit.refusalVersion, 1);
    assert.deepEqual(audit.package, { name: "fully-refused-fixture", version: "1.0.0" });
    assert.equal(audit.refusals.length, 1);
    assert.equal(audit.refusals[0].entrypoint, ".");
    assert.equal(audit.refusals[0].stage, "artifact-case");
    assert.match(audit.refusals[0].reason, /does not exist|is not a file|runtime target/i);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("finite wildcard entrypoints are enumerated from exact package files", () => {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-wildcard-test-"));
  mkdirSync(join(root, "dist", "components"), { recursive: true });
  writeFileSync(join(root, "dist", "components", "button.js"), "export const Button = 1;\n");
  writeFileSync(join(root, "dist", "components", "menu.js"), "export const Menu = 1;\n");
  try {
    assert.deepEqual(
      finiteEntrypoints(
        {
          exports: {
            ".": "./dist/index.js",
            "./components/*": "./dist/components/*.js",
            "./opaque/*": "./generated/no-star.js"
          }
        },
        [],
        root
      ),
      {
        entrypoints: [".", "./components/button", "./components/menu"],
        wildcardRefusals: ["./opaque/*"],
        wildcardBranchRefusals: [],
        wildcardResourceRefusals: []
      }
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("artifact-case candidate work limit matches proof policy 2", () => {
  assert.equal(
    ARTIFACT_CASE_CANDIDATE_LIMIT,
    proofPolicy.resourceBudgets.artifactCaseCandidates
  );
});

test("finite wildcard census refuses only expansions beyond the policy work limit", () => {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-wildcard-budget-"));
  try {
    mkdirSync(join(root, "dist", "small"), { recursive: true });
    mkdirSync(join(root, "src"), { recursive: true });
    writeFileSync(join(root, "dist", "small", "index.js"), "export const ok = true;\n");
    for (let index = 0; index < 5; index += 1) {
      writeFileSync(join(root, "src", `file-${index}.js`), "export const value = true;\n");
    }
    assert.deepEqual(
      finiteEntrypoints(
        {
          exports: {
            ".": "./index.js",
            "./*": "./dist/*/index.js",
            "./src/*": "./src/*.js"
          }
        },
        [],
        root,
        { conditionPartitionCount: 2, artifactCaseCandidateLimit: 6 }
      ),
      {
        entrypoints: [".", "./small"],
        wildcardRefusals: [],
        wildcardBranchRefusals: [],
        wildcardResourceRefusals: [
          { entrypoint: "./src/*", candidates: 7, limit: 6 }
        ]
      }
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("finite wildcard census unions materialized branches and retains absent branches", () => {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-wildcard-branches-"));
  try {
    mkdirSync(join(root, "dist"), { recursive: true });
    writeFileSync(join(root, "dist", "button.js"), "export const Button = 1;\n");
    assert.deepEqual(
      finiteEntrypoints(
        {
          exports: {
            "./*": {
              source: "./src/*.ts",
              default: "./dist/*.js"
            }
          }
        },
        [],
        root
      ),
      {
        entrypoints: ["./button"],
        wildcardRefusals: [],
        wildcardBranchRefusals: [
          { entrypoint: "./*", target: "./src/*.ts" }
        ],
        wildcardResourceRefusals: []
      }
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("artifact candidates count distinct active branches per entrypoint", () => {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-condition-budget-"));
  try {
    for (const name of ["default", "dev", "a", "b"]) {
      writeFileSync(join(root, `${name}.js`), `export const ${name} = 1;\n`);
      writeFileSync(join(root, `${name}.d.ts`), `export declare const ${name}: 1;\n`);
    }
    const manifest = {
      exports: {
        ".": { development: "./dev.js", default: "./default.js" },
        "./a": "./a.js",
        "./b": "./b.js"
      }
    };
    const candidates = finiteArtifactCandidates(
      manifest,
      [".", "./a", "./b"],
      [[], ["development"]],
      root,
      { artifactCaseCandidateLimit: 4 }
    );
    assert.deepEqual(candidates, [
      { entrypoint: ".", conditions: [] },
      { entrypoint: ".", conditions: ["development"] },
      { entrypoint: "./a", conditions: [] },
      { entrypoint: "./b", conditions: [] }
    ]);
    assert.throws(
      () => finiteArtifactCandidates(
        manifest,
        [".", "./a", "./b"],
        [[], ["development"]],
        root,
        { artifactCaseCandidateLimit: 3 }
      ),
      /4 exact artifact-case candidates exceed/
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("artifact candidate boundary accepts 1024 exact entrypoints and rejects 1025", () => {
  const entrypoints = Array.from({ length: 1_025 }, (_, index) => `./entry-${index}`);
  assert.equal(
    finiteEntrypoints({}, entrypoints.slice(0, 1_024), null, {
      artifactCaseCandidateLimit: 1_024
    }).entrypoints.length,
    1_024
  );
  assert.throws(
    () => finiteEntrypoints({}, entrypoints, null, {
      artifactCaseCandidateLimit: 1_024
    }),
    /1025 artifact-case candidates exceed/
  );
});

test("condition census enumerates compatible axes without contradictory cases", () => {
  const partitions = finiteConditionPartitions(
    {
      exports: {
        ".": {
          browser: { development: "./browser-dev.js", production: "./browser.js" },
          node: { development: "./node-dev.js", production: "./node.js" },
          worker: "./worker.js",
          csr: "./csr.js",
          "string-ssr": "./ssr.js",
          custom: "./custom.js"
        }
      }
    },
    []
  );
  assert.equal(partitions.length, 72);
  assert.deepEqual(partitions[0], []);
  for (const partition of partitions) {
    assert.ok(partition.filter(value => ["browser", "node", "deno", "worker"].includes(value)).length <= 1);
    assert.ok(partition.filter(value => ["development", "production"].includes(value)).length <= 1);
    assert.ok(partition.filter(value => ["csr", "string-ssr", "streaming-ssr"].includes(value)).length <= 1);
  }
  assert.ok(partitions.some(partition =>
    JSON.stringify(partition) === JSON.stringify(["browser", "custom", "development", "string-ssr"])
  ));
});

test("a merge contradiction refuses only its exact artifact candidate", async () => {
  const candidates = ["known-a", "contradictory-b", "known-c"].map(entrypoint => ({
    entrypoint
  }));
  const attempts = [];
  const result = await retainIndependentlyMergeableProposals(
    candidates,
    async (merged, candidate) => {
      attempts.push({ merged: merged?.members ?? [], candidate: candidate.entrypoint });
      if (candidate.entrypoint === "contradictory-b") throw new Error("invalid graph");
      return { members: [...(merged?.members ?? []), candidate.entrypoint] };
    }
  );

  assert.equal(result.acceptedCount, 2);
  assert.deepEqual(result.merged.members, ["known-a", "known-c"]);
  assert.deepEqual(result.rejected.map(item => item.candidate.entrypoint), ["contradictory-b"]);
  assert.deepEqual(attempts[2], { merged: ["known-a"], candidate: "known-c" });
});

test("batched merge isolation preserves order and accepts valid intervals together", async () => {
  const candidates = ["known-a", "contradictory-b", "known-c", "known-d"];
  const attempts = [];
  const result = await retainIndependentlyMergeableProposalBatches(
    candidates,
    async (accepted, interval) => {
      attempts.push({ accepted: [...accepted], interval: [...interval] });
      if (interval.includes("contradictory-b")) throw new Error("invalid graph");
      return { members: [...accepted, ...interval] };
    },
    new Error("the full interval is known to fail")
  );
  assert.equal(result.acceptedCount, 3);
  assert.deepEqual(result.merged.members, ["known-a", "known-c", "known-d"]);
  assert.deepEqual(result.rejected.map(item => item.candidate), ["contradictory-b"]);
  assert.deepEqual(attempts, [
    { accepted: [], interval: ["known-a", "contradictory-b"] },
    { accepted: [], interval: ["known-a"] },
    { accepted: ["known-a"], interval: ["contradictory-b"] },
    { accepted: ["known-a"], interval: ["known-c", "known-d"] }
  ]);
});
