import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  realpathSync,
  rmSync,
  writeFileSync
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

test("single-case graph retry follows an exact callback refusal without replacing accepted coverage", async () => {
  for (const variant of ["recover", "success", "existing", "multiple", "infrastructure", "other-family", "not-requested"]) {
    const scratch = mkdtempSync(join(tmpdir(), "single-case-graph-retry-"));
    try {
      const catalog = join(scratch, "catalog");
      if (variant === "existing") mkdirSync(catalog);
      const failure = new CertificationRefusal({ stage: "witness-acquisition", owner: "certifier",
        demandId: variant === "infrastructure" ? null : "exact-callback-demand",
        family: variant === "other-family" ? "recursive-value-shape" : "argument-binding", reason: "unproved" });
      const preparedCases = [{ artifactCase: { entrypoint: ".", conditions: [] } }];
      const events = [];
      const ordinary = { authority: "native-certification-complete", lane: "ordinary" };
      const recovered = { authority: "native-certification-complete", lane: "graph" };
      const run = executeNativeOrGraphCertification({
        options: { catalog }, scratch, graph: null,
        generated: { certificationInputs: variant === "multiple" ? [{}, {}] : [{}] },
        prepareGeneratedGraph: variant === "not-requested" ? null : async error => {
          assert.equal(error, failure);
          events.push("prepare");
          return { preparedCases };
        }
      }, {
        executeNative: async () => {
          events.push("ordinary");
          if (variant === "success") return ordinary;
          // A failed attempt may create this directory. Its existence must
          // not be confused with a publication predating the transaction.
          mkdirSync(catalog, { recursive: true });
          throw failure;
        },
        executeGraph: async ({ cases }) => {
          events.push("graph");
          assert.equal(cases, preparedCases);
          return recovered;
        }
      });
      if (variant === "recover") {
        assert.equal(await run, recovered);
        assert.deepEqual(events, ["ordinary", "prepare", "graph"]);
      } else if (variant === "success") {
        assert.equal(await run, ordinary);
        assert.deepEqual(events, ["ordinary"]);
      } else {
        await assert.rejects(run, error => error === failure);
        assert.deepEqual(events, ["ordinary"]);
      }
    } finally { rmSync(scratch, { recursive: true, force: true }); }
  }
});

import {
  adoptFrameValue,
  createFrameRecord,
  createRuntimeProbeHarness,
  serializeFrame
} from "../scripts/contract-probe-harness.mjs";
import {
  ArtifactResolutionError,
  resolvePackageArtifactClosure
} from "../scripts/artifact-resolution.mjs";
import {
  buildPublishedGraphExecutionRequest,
  CertificationRefusal,
  acquireRootCompilerSources,
  cascadeGraphNodeRefusals,
  graphCasesWithoutRefusedNodes,
  retainedCaseFloorRefusal,
  recoveryGraphBudgetRefusal,
  RECOVERY_GRAPH_CASE_BUDGET,
  certifyContract,
  isExactDependencyCompositionRefusal,
  isReusableDependencyRefusalAudit,
  nativeRefusalAttribution,
  locateExternalDependencyPackageRoot,
  mergeProposalDependencies,
  reexportImporterCensus,
  staticRuntimeDependencies,
  staticBindingDependencies,
  certificationImporterPathFor,
  parseCertifyArguments,
  partialProposalHasDependencyFrontier,
  preparedGraphForPartialProposal,
  declinedDependencyGraphCases,
  RetainedCasePreparationRefusal,
  recoveryGraphCases,
  certifyRecoverableCaseSelection,
  certifyIndependentCaseSelection,
  executeNativeOrGraphCertification,
  projectProposalCases,
  mapWithExactConcurrency,
  publishedGraphPreparationConcurrency,
  registryAcquisitionConcurrency,
  registryCacheRoot,
  reusableProposalInputs,
  runContractCertificationPipeline,
  validatedReusableDependencyRefusalAuditBytes,
  WITHHELD_CLOSURE_MARKER,
  withheldClosuresFromNativeOutput
} from "../scripts/certify-contract.mjs";
import { parseProbeArguments } from "../scripts/probe-contract.mjs";
import { parseReviewArguments } from "../scripts/review-contract.mjs";
import { parseVerifyArguments } from "../scripts/verify-contract.mjs";
import {
  ARTIFACT_CASE_CANDIDATE_LIMIT,
  ARTIFACT_APPLICABILITY,
  ARTIFACT_DISPOSITION,
  REFUSAL_CLASSES,
  artifactAnalysisBatchConcurrencyLimit,
  artifactApplicabilityForRefusal,
  artifactRefusalClass,
  artifactCaseDisposition,
  declaredApplicabilityClaims,
  finiteArtifactCandidates,
  finiteConditionPartitions,
  finiteEntrypoints,
  generatePackageContract,
  partitionArtifactAnalysisBatches,
  recommendedArtifactAnalysisBatchConcurrency,
  retainIndependentlyMergeableProposalBatches,
  retainIndependentlyMergeableProposals,
  withheldClaimsFromEmitterOutput,
  declinedClosuresFromEmitterOutput
} from "../scripts/generate-package-contract.mjs";

test("declaration binding recovery requests only exact refused reexports without granting authority", () => {
  const runtime = { axis: "runtime", kind: "import", specifier: "runtime", importerPath: "./index.js" };
  const declaration = { axis: "declarations", kind: "reexport", specifier: "types/setup", importerPath: "./index.d.ts" };
  const resolved = { externalDependencies: [runtime, declaration,
    { ...declaration, kind: "import", specifier: "type-import" },
    { ...declaration, specifier: "unrequested" }] };
  const coordinate = { entrypoint: "./setup", conditions: ["import"] };
  const refusal = { ...coordinate, conditions: [], reason: "accepted dependency types/setup has no exact declarations binding for export configure" };
  assert.deepEqual(staticBindingDependencies(resolved, coordinate, [refusal]), [runtime, declaration]);
  for (const altered of [
    { ...refusal, entrypoint: "." },
    { ...refusal, conditions: ["browser"] },
    { ...refusal, reason: refusal.reason.replace("declarations", "runtime") },
    { ...refusal, reason: refusal.reason.replace("types/setup", "type-import") },
    { ...refusal, reason: refusal.reason.replace("types/setup", "outside-census") }
  ]) assert.deepEqual(staticBindingDependencies(resolved, coordinate, [altered]), [runtime]);
  assert.deepEqual(staticBindingDependencies(resolved, coordinate), [runtime]);
  assert.deepEqual(staticRuntimeDependencies(resolved), [runtime]);
});

test("a withheld-closure record is read off the native transaction's stdout, and only when whole", () => {
  const record = {
    artifactCase: "artifact-case:abc",
    export: "noRecipe",
    domain: "creates",
    semanticClaimId: `claim:v1:sha256:${"0".repeat(64)}`,
    reason: "no recipe in corpus"
  };
  const stdout = [
    "policy-2 certification planning",
    `${WITHHELD_CLOSURE_MARKER}${JSON.stringify(record)}`,
    // A graph node carries its identity beside the record; it is kept as is.
    `${WITHHELD_CLOSURE_MARKER}${JSON.stringify({ ...record, export: "other", node: { package: "p", version: "1.0.0", digest: "sha256:1" } })}`,
    // Malformed or shapeless lines are not records, and neither is prose that
    // happens to mention the marker mid-line.
    `${WITHHELD_CLOSURE_MARKER}{not json`,
    `${WITHHELD_CLOSURE_MARKER}${JSON.stringify({ export: 1, domain: "creates", reason: "x" })}`,
    `note: ${WITHHELD_CLOSURE_MARKER}${JSON.stringify(record)}`
  ].join("\n");
  assert.deepEqual(withheldClosuresFromNativeOutput(stdout), [
    record,
    { ...record, export: "other", node: { package: "p", version: "1.0.0", digest: "sha256:1" } }
  ]);
  assert.deepEqual(withheldClosuresFromNativeOutput(""), []);
  assert.deepEqual(withheldClosuresFromNativeOutput(undefined), []);
});

test("a withheld-claim record is read only for its own target, and only when whole", () => {
  const marker = "solid-checker:withheld-owner-requirement=";
  const stdout = [
    `${marker}/scratch/a-proposal.json\tmountShape\teffect\tno domain carries it`,
    `${marker}/scratch/b-proposal.json\tother\tboundary\ta lowering fact`,
    // Another target of the same batch, a truncated line, and ordinary output
    // all have to be ignored: the marker is a contract, not a prose scan.
    `${marker}/scratch/a-proposal.json\tincomplete`,
    "generated unaccepted stable contract proposal for x@1.0.0"
  ].join("\n");
  assert.deepEqual(withheldClaimsFromEmitterOutput(stdout, "/scratch/a-proposal.json"), [
    { export: "mountShape", role: "effect", reason: "no domain carries it" }
  ]);
  assert.deepEqual(withheldClaimsFromEmitterOutput(stdout, "/scratch/b-proposal.json"), [
    { export: "other", role: "boundary", reason: "a lowering fact" }
  ]);
  assert.deepEqual(withheldClaimsFromEmitterOutput("", "/scratch/a-proposal.json"), []);
  assert.deepEqual(withheldClaimsFromEmitterOutput(undefined, "/scratch/a-proposal.json"), []);
});

test("a declined-closure record is read per target, relativized, and only when whole", () => {
  const marker = "solid-checker:declined-closure=";
  const stdout = [
    `${marker}/scratch/a-proposal.json\tdialectSilent\tcreates\tdialect-silent\tsolid-js\tcreateEffect\t/pkg/index.js:10:20\t\t\t`,
    `${marker}/scratch/a-proposal.json\tviaHelper\tcreates\trefusing-callee-fixpoint\t\t\t/pkg/index.js:40:52\t/pkg/index.js:30:60\t\t`,
    `${marker}/scratch/b-proposal.json\tother\tcreates\tunresolved-callee\t\t\t/pkg/other.js:1:9\t\tmember-property-unresolved\tread`,
    // The eight-column form an emitter predating the shape columns wrote: it
    // still parses, with both new fields empty rather than absent.
    `${marker}/scratch/c-proposal.json\tlegacy\tcreates\tunresolved-callee\t\t\t/pkg/legacy.js:2:8\t`,
    // Another target of the same batch, a line missing its kind, and ordinary
    // output all have to be ignored: the marker is a contract, not a prose
    // scan.
    `${marker}/scratch/a-proposal.json\tincomplete\tcreates`,
    "generated unaccepted stable contract proposal for x@1.0.0"
  ].join("\n");
  assert.deepEqual(declinedClosuresFromEmitterOutput(stdout, "/scratch/a-proposal.json", "/pkg"), [
    {
      export: "dialectSilent",
      domain: "creates",
      kind: "dialect-silent",
      package: "solid-js",
      callee: "createEffect",
      location: "<package-root>/index.js:10:20",
      declaration: "",
      shape: "",
      spelling: ""
    },
    {
      export: "viaHelper",
      domain: "creates",
      kind: "refusing-callee-fixpoint",
      package: "",
      callee: "",
      location: "<package-root>/index.js:40:52",
      declaration: "<package-root>/index.js:30:60",
      shape: "",
      spelling: ""
    }
  ]);
  // A second target of the same batch is answered on its own, and without a
  // package root the location is kept verbatim rather than truncated by guess.
  assert.deepEqual(declinedClosuresFromEmitterOutput(stdout, "/scratch/b-proposal.json"), [
    {
      export: "other",
      domain: "creates",
      kind: "unresolved-callee",
      package: "",
      callee: "",
      location: "/pkg/other.js:1:9",
      declaration: "",
      // The two appended columns: the shape of the callee expression, and the
      // one concrete string that shape observed.
      shape: "member-property-unresolved",
      spelling: "read"
    }
  ]);
  // An eight-column line stays readable: `kind` still says `unresolved-callee`,
  // and the shape columns are empty rather than missing.
  assert.deepEqual(declinedClosuresFromEmitterOutput(stdout, "/scratch/c-proposal.json"), [
    {
      export: "legacy",
      domain: "creates",
      kind: "unresolved-callee",
      package: "",
      callee: "",
      location: "/pkg/legacy.js:2:8",
      declaration: "",
      shape: "",
      spelling: ""
    }
  ]);
  assert.deepEqual(declinedClosuresFromEmitterOutput("", "/scratch/a-proposal.json"), []);
  assert.deepEqual(declinedClosuresFromEmitterOutput(undefined, "/scratch/a-proposal.json"), []);
});

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

test("a refusal's class comes from the error's structure, never from its prose", () => {
  // The CLI resolver's own code for "this case needs an accepted contract for
  // a dependency".
  assert.equal(
    artifactRefusalClass(
      new ArtifactResolutionError(
        "accepted-dependency-binding",
        "accepted dependency dependency has no exact runtime binding for export default"
      )
    ),
    REFUSAL_CLASSES.DependencyComposition
  );
  // The native emitter's own marker line, which exists precisely so this
  // decision need not read the sentence after it.
  assert.equal(
    artifactRefusalClass(
      new Error(
        "solid-checker:unresolved-dependency-module=@tanstack/pacer\n" +
          'emit package contract: cannot statically expand external export-all "@tanstack/pacer"'
      )
    ),
    REFUSAL_CLASSES.DependencyComposition
  );
  // Prose alone is not evidence: the same sentence without the marker line is
  // not a structured claim, and every other resolver code is a fact about the
  // publisher's own bytes.
  assert.equal(
    artifactRefusalClass(
      new Error('cannot statically expand external export-all "@tanstack/pacer"')
    ),
    REFUSAL_CLASSES.PublishedArtifact
  );
  assert.equal(
    artifactRefusalClass(new ArtifactResolutionError("declarations-not-found", "no .d.ts")),
    REFUSAL_CLASSES.PublishedArtifact
  );
  assert.equal(
    artifactRefusalClass(new Error("entry file has no runtime ESM exports")),
    REFUSAL_CLASSES.PublishedArtifact
  );
});

test("inapplicable artifact cases are decided from the export-map selection alone", () => {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-disposition-"));
  mkdirSync(join(root, "assets"), { recursive: true });
  writeFileSync(join(root, "index.js"), "export const value = 1;\n");
  writeFileSync(join(root, "assets", "widget.js"), "export const widget = 1;\n");
  writeFileSync(join(root, "assets", "widget.js.map"), "{}\n");
  writeFileSync(join(root, "assets", "styles.module.css"), ".root {}\n");
  writeFileSync(join(root, "assets", "addon.node"), "\0native\n");
  writeFileSync(join(root, "assets", "engine.wasm"), "\0asm\n");
  const manifest = {
    exports: {
      ".": "./index.js",
      "./assets/widget.js": "./assets/widget.js",
      "./assets/widget.js.map": "./assets/widget.js.map",
      "./assets/styles.module.css": "./assets/styles.module.css",
      "./private": { "vendor/source": "./src/private.ts", default: "./index.js" },
      "./browser-gap": { browser: "./missing-browser.js", default: "./index.js" },
      "./bun-gap": { bun: "./missing-bun.js", default: "./index.js" },
      "./solid-gap": { solid: "./missing-solid.jsx", default: "./index.js" },
      "./blocked": { "vendor/source": null, default: "./index.js" },
      "./extensionless": "./missing-target",
      "./native": "./assets/addon.node",
      "./engine": "./assets/engine.wasm"
    }
  };
  const disposition = (entrypoint, conditions = []) =>
    artifactCaseDisposition({ manifest, packageRoot: root, entrypoint, conditions });
  try {
    // A module entrypoint, and an asset entrypoint reached through the same
    // wildcard-shaped surface: only the asset is inapplicable.
    assert.equal(disposition("./assets/widget.js"), null);
    assert.deepEqual(disposition("./assets/widget.js.map"), {
      class: ARTIFACT_DISPOSITION.NonModuleTarget,
      reason: 'runtime target extension ".map" is not an executable module'
    });
    assert.deepEqual(disposition("./assets/styles.module.css"), {
      class: ARTIFACT_DISPOSITION.NonModuleTarget,
      reason: 'runtime target extension ".css" is not an executable module'
    });

    // A missing target behind a PRIVATE NAMESPACED condition is unpublished on
    // purpose; the same entrypoint under the empty partition resolves and stays
    // ordinary.
    assert.deepEqual(disposition("./private", ["vendor/source"]), {
      class: ARTIFACT_DISPOSITION.UnpublishedConditionalTarget,
      reason:
        'runtime target is unpublished behind private namespaced export condition(s) "vendor/source"'
    });
    assert.equal(disposition("./private"), null);

    // Standard conditions only: real consumers fail there, so it stays a
    // refusal. `.` under the empty/default partition can never reach a custom
    // condition at all, which is why its refusal always stands.
    assert.equal(disposition("./browser-gap", ["browser"]), null);
    assert.equal(disposition("."), null);

    // A BARE-NAME custom condition is not private: `bun` is activated
    // unconditionally by the Bun runtime, and `solid` by vite-plugin-solid and
    // solid-start, so a consumer really does reach these targets and a missing
    // one is a defective publish. Both stay refusals.
    assert.equal(disposition("./bun-gap", ["bun"]), null);
    assert.equal(disposition("./solid-gap", ["solid"]), null);

    // `.node` and `.wasm` are executable entrypoints this pipeline cannot read,
    // not inert resources. The closure already names them native-code and
    // opaque-wasm hazards, so they keep certify-or-refuse rather than asserting
    // nothing.
    assert.equal(disposition("./native"), null);
    assert.equal(disposition("./engine"), null);

    // Selections that refuse for their own reason keep those semantics.
    assert.equal(disposition("./blocked", ["vendor/source"]), null);
    assert.equal(disposition("./absent"), null);
    // An extensionless target is not answered by the module-extension rule.
    assert.equal(disposition("./extensionless"), null);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("the bundler-suffix fixture keeps a real control, pinned by both snapshots", () => {
  // `asset-query-import` proves a `?raw` specifier is opaque: no closure edge,
  // no proof candidate, every claim open. That claim is only meaningful against
  // a control where the SAME file, imported as a plain module, really is
  // proven. The control lives in `asset-query-import-control`, and this test
  // pins the two numbers against each other so a regression that flattened the
  // control -- one that stopped resolving the plain module import or stopped
  // producing candidates for it -- cannot be absorbed by a routine
  // `contract-corpus.mjs --update` while the `?raw` fixture's own snapshot sits
  // unchanged and its README's control claim quietly becomes a tautology.
  const fixtures = join(
    dirname(fileURLToPath(import.meta.url)),
    "../../../fixtures/package-contracts"
  );
  const plan = name =>
    JSON.parse(readFileSync(join(fixtures, name, "expected-proposal.json"), "utf8"));

  // 3 candidates and 7 open claims, since 2026-09-04: the generator proposes a
  // `creates` closure again, this time derived from its own walk of the
  // export's implementation rather than from the owner-requirement census
  // (docs/adr/0008-implementation-census-for-creates.md) -- a proposal the
  // certifier's implementation census then proves or refuses. Between
  // 2026-09-03 and then the count was 2 / 8: the owner-requirement-derived
  // `creates` had been withdrawn because an owner requirement is not a
  // `create`. The control still proves the plain module import resolves and
  // still produces candidates -- `reads`, `returns`, and now `creates` --
  // which is what this pin exists to protect.
  const control = plan("asset-query-import-control");
  assert.equal(control.closureCandidates.length, 3);
  assert.equal(control.unresolvedClaims.length, 7);

  const suffixed = plan("asset-query-import");
  assert.equal(suffixed.closureCandidates.length, 0);
  assert.equal(suffixed.unresolvedClaims.length, 20);
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

test("the dependency-graph lane is an explicit, valueless, default-off request", () => {
  const base = ["--integrity", "sha512-cGlubmVk"];
  // Off unless asked for: today's behavior is that a partial proposal is
  // certified as it stands, and the two lanes describe different case sets, so
  // switching by default would silently change which cases carry a receipt.
  assert.equal(parseCertifyArguments(base).dependencyGraphLane, false);
  assert.equal(
    parseCertifyArguments([...base, "--dependency-graph-lane"]).dependencyGraphLane,
    true
  );
  // Valueless, and it must not swallow the option that follows it.
  const options = parseCertifyArguments([
    ...base,
    "--dependency-graph-lane",
    "--entrypoint",
    "./web"
  ]);
  assert.equal(options.dependencyGraphLane, true);
  assert.deepEqual(options.entrypoints, ["./web"]);
});

test("entrypoint recovery is opt-in and retains generated cases beside the dependency frontier", () => {
  const base = ["--integrity", "sha512-cGlubmVk"];
  assert.equal(parseCertifyArguments(base).recoverEntrypoints, false);
  const options = parseCertifyArguments([...base, "--recover-entrypoints", "--entrypoint", "./m"]);
  assert.equal(options.recoverEntrypoints, true);
  assert.deepEqual(options.entrypoints, ["./m"]);
  const generated = { certificationInputs: [{ entrypoint: "./m", conditions: [] }] };
  const frontier = [".", "./v2"].map(entrypoint => ({
    entrypoint, conditions: [], stage: "artifact-case", class: "dependency-composition",
    applicability: "runtime-module", reason: "missing exact dependency binding"
  }));
  const unproved = { entrypoint: "./other", conditions: [], class: "published-artifact" };
  const result = recoveryGraphCases(generated, [...frontier, unproved]);
  assert.deepEqual(result.cases, ["./m", ".", "./v2"].map(entrypoint => ({ entrypoint, conditions: ["import"] })));
  assert.deepEqual(result.retainedCases, [{ entrypoint: "./m", conditions: ["import"] }]);
  assert.deepEqual(result.remainingRefusals, [unproved]);
  // The requests are rebuilt, without mutating or transplanting evidence.
  assert.deepEqual(generated.certificationInputs[0].conditions, []);
  assert.throws(() => recoveryGraphCases({ certificationInputs: [] }, frontier), /no generated artifact cases/);
  assert.throws(() => recoveryGraphCases({ certificationInputs: [{ entrypoint: "./m" }] }, frontier), /exact entrypoint/);
  assert.throws(() => recoveryGraphCases(generated, [
    ...frontier, { ...frontier[0], entrypoint: "./m", conditions: ["import"] }
  ]), /conflicting duplicate/);
  assert.throws(() => recoveryGraphCases(generated, [...frontier, frontier[0]]), /conflicting duplicate/);
  // Two genuine condition selections stay separate even on the same subpath.
  assert.equal(recoveryGraphCases(generated, [{ ...frontier[0], entrypoint: "./m", conditions: ["browser"] }]).cases.length, 2);
});

test("recovery publishes newly proved cases with every retained case and exact refusals", async () => {
  const cases = ["retained", "good", "bad"];
  const coordinates = cases.map(entrypoint => ({ entrypoint, conditions: ["import"] }));
  const recovery = { cases: coordinates, retainedCases: coordinates.slice(0, 1) };
  const attempts = [];
  const result = await certifyRecoverableCaseSelection({ cases, recovery, certify: async (selected, publish) => {
    attempts.push({ selected: [...selected], publish });
    if (selected.includes("bad")) throw new CertificationRefusal({ stage: "witness-acquisition", owner: "certifier", demandId: "exact-demand", family: "recursive-value-shape", reason: "unproved" });
    return { final: publish };
  }});
  assert.deepEqual(attempts, [
    { selected: cases, publish: true },
    { selected: ["retained"], publish: false },
    { selected: ["retained", "good"], publish: false },
    { selected: cases, publish: false },
    { selected: ["retained", "good"], publish: true }
  ]);
  assert.deepEqual(result, { final: true });
  assert.deepEqual(recovery.publishedCases, coordinates.slice(0, 2));
  assert.equal(recovery.caseRefusals[0].entrypoint, "bad");
  assert.equal(recovery.caseRefusals[0].demandId, "exact-demand");
});

test("independent recovery certifies fresh subsets and publishes exact accepted cases", async () => {
  const cases = [".", "./bad", "./good"].map(entrypoint => ({ entrypoint, conditions: ["import"] }));
  const recovery = {}, attempts = [];
  await certifyIndependentCaseSelection({ cases, recovery, existingPublication: false, certify: async (selected, publish) => {
    attempts.push({ cases: selected.map(x => x.entrypoint), publish });
    if (selected.some(x => x.entrypoint === "./bad")) throw new CertificationRefusal({
      stage: "witness-acquisition", owner: "certifier", demandId: "bad-demand", reason: "unproved"
    });
  }});
  assert.deepEqual(attempts, [
    { cases: [".", "./bad", "./good"], publish: true },
    { cases: ["."], publish: false },
    { cases: [".", "./bad"], publish: false },
    { cases: [".", "./good"], publish: false },
    { cases: [".", "./good"], publish: true }
  ]);
  assert.deepEqual(recovery.expectedCases, cases);
  assert.deepEqual(recovery.publishedCases, [cases[0], cases[2]]);
  assert.equal(recovery.caseRefusals[0].demandId, "bad-demand");
});

test("graph fallback independently certifies unaccepted cases and captures prior publication before attempts", async () => {
  for (const existedBefore of [false, true]) {
    const scratch = mkdtempSync(join(tmpdir(), "graph-fallback-selection-"));
    try {
      const catalog = join(scratch, "catalog");
      const marker = join(catalog, "existing-publication");
      if (existedBefore) {
        mkdirSync(catalog);
        writeFileSync(marker, "preserve me");
      }
      const coordinates = [".", "./bad", "./good"].map(entrypoint => ({ entrypoint, conditions: ["import"] }));
      const selection = { artifact: { path: "./index.js", sha256: "runtime" },
        declarations: { path: "./index.d.ts", sha256: "types" },
        resolution: { runtimeBranch: "/import", typesBranch: "/types" }, exports: { value: "summary" } };
      const document = { package: { name: "pkg", version: "1.0.0", integrity: "pin" },
        entrypoints: Object.fromEntries(coordinates.map(c => [c.entrypoint, { cases: [selection] }])),
        summaries: { summary: { shape: "plain", call: {} } } };
      const output = join(scratch, "proposal.json");
      writeFileSync(output, JSON.stringify(document));
      const inputs = coordinates.map(c => ({ ...c, resolution: {
        packageName: "pkg", packageVersion: "1.0.0", packageIntegrity: "pin", requestedEntrypoint: c.entrypoint,
        packageRoot: "/pkg", runtime: { path: "/pkg/index.js", digest: "sha256:runtime" },
        declarations: { path: "/pkg/index.d.ts", digest: "sha256:types" },
        runtimeTrace: { branch: "/import" }, declarationTrace: { branch: "/types" }
      } }));
      const graphOnly = { entrypoint: "./graph-only", conditions: ["import"] };
      const recovery = { cases: [...coordinates, graphOnly], retainedCases: coordinates };
      const graph = { preparedCases: recovery.cases, timing: { entrypointRecovery: recovery, reusedProposalForRecovery: true } };
      const failure = new CertificationRefusal({ stage: "witness-acquisition", owner: "certifier", demandId: "bad-demand", reason: "unproved" });
      const attempts = [];
      const run = executeNativeOrGraphCertification({
        options: { catalog, trustConfigurationOutput: join(scratch, "trust.json"), recoverEntrypoints: true },
        generated: { output, certificationInputs: inputs }, graph, scratch
      }, {
        executeGraph: async () => {
          // A failed attempt leaves a directory, but no accepted case set.
          mkdirSync(catalog, { recursive: true });
          throw failure;
        },
        executeNative: async ({ generated, options }) => {
          const selected = generated.certificationInputs.map(c => c.entrypoint);
          const projected = JSON.parse(readFileSync(generated.output, "utf8"));
          assert.deepEqual(Object.keys(projected.entrypoints), selected);
          assert.deepEqual(projected.summaries, document.summaries);
          attempts.push({ selected, publish: options.catalog === catalog });
          if (selected.includes("./bad")) throw failure;
          return { authority: "native-certification-complete" };
        }
      });
      if (existedBefore) {
        await assert.rejects(run, error => error === failure);
        assert.deepEqual(attempts, [{ selected: [".", "./bad", "./good"], publish: true }]);
        assert.equal(readFileSync(marker, "utf8"), "preserve me");
        assert.equal(recovery.publishedCases, undefined);
      } else {
        assert.deepEqual(await run, { authority: "native-certification-complete" });
        assert.deepEqual(attempts.at(-1), { selected: [".", "./good"], publish: true });
        assert.ok(attempts.some(a => !a.publish));
        assert.deepEqual(recovery.publishedCases, [coordinates[0], coordinates[2]]);
        assert.deepEqual(recovery.unpublishedCases, [coordinates[1], graphOnly]);
        assert.equal(recovery.caseRefusals[0].demandId, "bad-demand");
        assert.equal(recovery.graphRefusal.reason, "unproved");
        assert.equal(graph.timing.independentCaseRecovery.nativeCertificationTransactions, attempts.length);
      }
    } finally {
      rmSync(scratch, { recursive: true, force: true });
    }
  }
});

test("case projection preserves whole claims and requires exact artifact identities", () => {
  const selected = { artifact: { path: "./index.js", sha256: "runtime" },
    declarations: { path: "./index.d.ts", sha256: "types" },
    resolution: { runtimeBranch: "/import", typesBranch: "/types" }, exports: { value: "summary" } };
  const document = { package: { name: "pkg", version: "1.0.0", integrity: "pin" },
    entrypoints: { ".": { cases: [selected] }, "./other": { cases: [{ ...selected, exports: { value: "other" } }] } },
    summaries: { summary: { shape: "plain", call: {} }, other: { shape: "callable" } } };
  const input = { entrypoint: ".", resolution: { packageName: "pkg", packageVersion: "1.0.0", packageIntegrity: "pin",
    requestedEntrypoint: ".", packageRoot: "/pkg", runtime: { path: "/pkg/index.js", digest: "sha256:runtime" },
    declarations: { path: "/pkg/index.d.ts", digest: "sha256:types" }, runtimeTrace: { branch: "/import" },
    declarationTrace: { branch: "/types" } } };
  const projected = projectProposalCases(document, [input]);
  assert.deepEqual(Object.keys(projected.entrypoints), ["."]);
  assert.equal(projected.entrypoints["."].cases[0], selected);
  assert.equal(projected.summaries.summary, document.summaries.summary);
  assert.deepEqual(Object.keys(projected.summaries), ["summary"]);
  assert.equal(Object.keys(document.entrypoints).length, 2);
  for (const change of [
    x => x.resolution.packageIntegrity = "other",
    x => x.resolution.runtime.digest = "sha256:other",
    x => x.resolution.declarations.path = "/pkg/other.d.ts",
    x => x.resolution.runtimeTrace.branch = "/browser",
    x => x.resolution.declarationTrace.branch = "/other"
  ]) {
    const changed = structuredClone(input); change(changed);
    assert.throws(() => projectProposalCases(document, [changed]), /exact/);
  }
  assert.throws(() => projectProposalCases(document, [input, input]), /duplicate/);
  assert.throws(() => projectProposalCases(document, []), /empty/);
  const detailed = structuredClone(document);
  detailed.entrypoints["."].cases[0].exports.value = { summary: "summary", stability: "unknown" };
  assert.deepEqual(Object.keys(projectProposalCases(detailed, [input]).summaries), ["summary"]);
});

test("large recovery privately certifies and reads its retained floor before publishing graph additions", async () => {
  for (const existedBefore of [false, true]) {
    const scratch = mkdtempSync(join(tmpdir(), "verified-floor-workflow-"));
    try {
      const catalog = join(scratch, "catalog");
      if (existedBefore) { mkdirSync(catalog); writeFileSync(join(catalog, "existing"), "preserve"); }
      const coordinates = ["./kept", "./bad", "./graph"].map(entrypoint => ({ entrypoint, conditions: ["import"] }));
      const selection = { artifact: { path: "./index.js", sha256: "runtime", closureSha256: "closure" }, declarations: { path: "./index.d.ts", sha256: "types" }, resolution: { runtimeBranch: "/import", typesBranch: "/types" }, exports: { value: "summary" } };
      const document = { package: { name: "pkg", version: "1.0.0", integrity: "pin" }, entrypoints: Object.fromEntries(coordinates.slice(0, 2).map(c => [c.entrypoint, { cases: [selection] }])), summaries: { summary: { shape: "plain", call: {} } } };
      const output = join(scratch, "proposal.json"); writeFileSync(output, JSON.stringify(document));
      const inputs = coordinates.slice(0, 2).map(c => ({ entrypoint: c.entrypoint, conditions: [], resolution: {
        importer: "/project/importer.mjs", specifier: `pkg/${c.entrypoint.slice(2)}`,
        packageName: "pkg", packageVersion: "1.0.0", packageIntegrity: "pin", packageRoot: "/pkg", requestedEntrypoint: c.entrypoint,
        runtime: { path: "/pkg/index.js", digest: "sha256:runtime" }, declarations: { path: "/pkg/index.d.ts", digest: "sha256:types" }, runtimeTrace: { branch: "/import" }, declarationTrace: { branch: "/types" }
      } }));
      const recovery = { cases: coordinates, retainedCases: coordinates.slice(0, 2), retainedProposalRoots: true };
      const graph = { preparedCases: coordinates, timing: { entrypointRecovery: recovery } };
      const failure = new CertificationRefusal({ stage: "witness-acquisition", owner: "certifier", reason: "bad remains unproved" });
      const nativeAttempts = [], graphAttempts = [];
      const write = (path, value) => { const bytes = JSON.stringify(value); writeFileSync(path, bytes); return `sha256:${createHash("sha256").update(bytes).digest("hex")}`; };
      const run = executeNativeOrGraphCertification({ options: { catalog, trustConfigurationOutput: join(scratch, "trust.json"), recoverEntrypoints: true }, generated: { output, certificationInputs: inputs }, graph, scratch }, {
        executeGraph: async ({ cases, catalogRoot }) => {
          graphAttempts.push({ cases: cases.map(c => c.entrypoint), public: catalogRoot === catalog });
          if (cases.some(c => c.entrypoint === "./bad")) throw failure;
          return { authority: "native-certification-complete", catalogRoot };
        },
        executeNative: async ({ generated, options }) => {
          nativeAttempts.push({ selected: generated.certificationInputs.map(c => c.entrypoint), public: options.catalog === catalog });
          if (generated.certificationInputs.some(c => c.entrypoint === "./bad")) throw failure;
          mkdirSync(options.catalog, { recursive: true });
          const main = JSON.parse(readFileSync(generated.output, "utf8"));
          const documentDigest = write(join(options.catalog, "main.json"), main);
          const imported = generated.certificationInputs[0].resolution;
          const bindings = { importer: imported.importer, specifier: imported.specifier, resolvedImportRoot: "exact-root", semanticDigest: "exact-semantic" };
          const receiptDigest = write(join(options.catalog, "receipt.json"), { payload: { ...bindings, mainDigest: documentDigest } });
          write(join(options.catalog, "accepted-contracts.json"), { format: "solid-checker-accepted-contract-catalog", catalogVersion: 2, contracts: [{ document: "main.json", documentDigest, receipt: "receipt.json", receiptDigest, bindings, import: imported }] });
          return { authority: "native-certification-complete", catalogRoot: options.catalog };
        }
      });
      if (existedBefore) {
        await assert.rejects(run, error => error === failure);
        assert.equal(recovery.verifiedRetainedFloor, undefined);
        assert.deepEqual(nativeAttempts, [{ selected: ["./kept", "./bad"], public: true }]);
        assert.equal(readFileSync(join(catalog, "existing"), "utf8"), "preserve");
      } else {
        assert.equal((await run).catalogRoot, catalog);
        assert.ok(nativeAttempts.every(attempt => !attempt.public));
        assert.deepEqual(graphAttempts, [
          { cases: ["./kept", "./bad", "./graph"], public: true },
          { cases: ["./kept", "./bad"], public: false },
          { cases: ["./kept", "./graph"], public: true }
        ]);
        assert.deepEqual(recovery.publishedCases, [coordinates[0], coordinates[2]]);
        assert.deepEqual(recovery.caseRefusals.map(c => c.entrypoint), ["./bad"]);
        assert.equal(recovery.verifiedRetainedFloor.publication.cases.length, 1);
        assert.deepEqual(recovery.cases, coordinates);
      }
    } finally { rmSync(scratch, { recursive: true, force: true }); }
  }
});

test("independent recovery cannot shrink an existing publication or clear an empty result", async () => {
  const cases = [".", "./bad"].map(entrypoint => ({ entrypoint, conditions: [] }));
  const failure = new CertificationRefusal({ stage: "witness-acquisition", owner: "certifier", reason: "unproved" });
  for (const existingPublication of [true, false]) {
    const recovery = {}, attempts = [];
    await assert.rejects(certifyIndependentCaseSelection({ cases, recovery, existingPublication,
      certify: async (selected, publish) => { attempts.push(publish); throw failure; }
    }), error => error === failure);
    assert.deepEqual(attempts, existingPublication ? [true] : [true, false, false]);
    assert.equal(recovery.publishedCases, undefined);
  }
  let calls = 0;
  await assert.rejects(certifyIndependentCaseSelection({ cases: [cases[0], cases[0]], recovery: {},
    existingPublication: false, certify: async () => { calls++; }
  }), /conflicting duplicate/);
  assert.equal(calls, 0);
  await assert.rejects(certifyIndependentCaseSelection({ cases, recovery: {}, certify: async () => { calls++; } }), /publication-state/);
  const oversized = Array.from({ length: 1025 }, (_, index) => ({ entrypoint: `./case${index}`, conditions: [] }));
  await assert.rejects(certifyIndependentCaseSelection({ cases: oversized, recovery: {}, existingPublication: false,
    certify: async () => { calls++; throw failure; }
  }), error => error === failure);
  assert.equal(calls, 1);
});

test("independent recovery propagates non-proof and final-publication failures", async () => {
  const cases = [".", "./bad"].map(entrypoint => ({ entrypoint, conditions: [] }));
  const failure = new CertificationRefusal({ stage: "witness-acquisition", owner: "certifier", reason: "unproved" });
  const trust = new CertificationRefusal({ stage: "receipt-issuance", owner: "trust", reason: "missing issuer" });
  const recovery = {};
  await assert.rejects(certifyIndependentCaseSelection({ cases, recovery, existingPublication: false,
    certify: async (selected, publish) => {
      if (selected.length === 2) throw failure;
      if (publish) throw trust;
    }
  }), error => error === trust);
  assert.equal(recovery.publishedCases, undefined);
  let calls = 0;
  await assert.rejects(certifyIndependentCaseSelection({ cases, recovery: {}, existingPublication: false,
    certify: async () => { calls++; throw trust; }
  }), error => error === trust);
  assert.equal(calls, 1);
});

test("large recovery subdivides refused batches and freshly verifies the union", async () => {
  const cases = Array.from({ length: 64 }, (_, index) => ({ entrypoint: `./case${index}`, conditions: [] }));
  const bad = new Set([cases[0], cases[31], cases[63]]), attempts = [], recovery = {};
  const failure = new CertificationRefusal({ stage: "witness-acquisition", owner: "certifier", reason: "unproved" });
  await certifyIndependentCaseSelection({ cases, recovery, existingPublication: false,
    certify: async (selected, publish) => {
      attempts.push({ selected, publish });
      if (selected.some(item => bad.has(item))) throw failure;
    }
  });
  assert.equal(recovery.strategy, "binary-subdivision");
  assert.deepEqual(attempts.at(-1), { selected: cases.filter(item => !bad.has(item)), publish: true });
  assert.equal(recovery.publishedCases.length, 61);
  assert.deepEqual(recovery.caseRefusals.map(x => x.entrypoint), ["./case0", "./case31", "./case63"]);
  assert.ok(attempts.length <= 2 * cases.length);
  assert.ok(attempts.some(x => !x.publish && x.selected.length > 1));

  const conflicting = {};
  await assert.rejects(certifyIndependentCaseSelection({ cases, recovery: conflicting, existingPublication: false,
    certify: async (selected, publish) => { if (publish) throw failure; }
  }), error => error === failure);
  assert.equal(conflicting.publishedCases, undefined);

  const none = {};
  let calls = 0;
  await assert.rejects(certifyIndependentCaseSelection({ cases, recovery: none, existingPublication: false,
    certify: async () => { calls++; throw failure; }
  }), error => error === failure);
  assert.equal(none.caseRefusals.length, 64);
  assert.equal(calls, 127);
  assert.equal(none.publishedCases, undefined);
});

test("failed graph preparation waits for its other workers before returning", async () => {
  let release;
  const blocked = new Promise(resolve => { release = resolve; });
  let completed = false;
  let rejected = false;
  const result = mapWithExactConcurrency([0, 1], 2, async index => {
    if (index === 0) throw new Error("first worker failed");
    await blocked;
    completed = true;
  }).catch(error => { rejected = true; throw error; });
  await Promise.resolve();
  await Promise.resolve();
  assert.equal(rejected, false);
  release();
  await assert.rejects(result, /first worker failed/);
  assert.equal(completed, true);
});

test("recovery never drops a retained case or hides a non-proof failure", async () => {
  for (const proofFailure of [true, false]) {
    const cases = ["retained", "candidate"];
    const recovery = { cases, retainedCases: ["retained"] };
    let calls = 0;
    const failure = proofFailure
      ? new CertificationRefusal({ stage: "witness-acquisition", owner: "certifier", reason: "base unproved" })
      : new Error("publication failed");
    await assert.rejects(certifyRecoverableCaseSelection({ cases, recovery, certify: async () => { calls++; throw failure; } }), error => error === failure);
    assert.equal(calls, proofFailure ? 2 : 1);
    assert.equal(recovery.publishedCases, undefined);
  }
});

test("a refused retained case selects independently across the prepared set when nothing was published", async () => {
  // ADR 0070. The prepared set is the retained proposal cases plus the
  // dependency-composition frontier that the proposal never contained. One
  // unprovable retained case must not take the frontier down with it.
  const cases = ["retained-ok", "retained-bad", "frontier"];
  const recovery = { cases, retainedCases: ["retained-ok", "retained-bad"] };
  const attempts = [];
  const result = await certifyRecoverableCaseSelection({
    cases, recovery, existingPublication: false,
    certify: async (selected, publish) => {
      attempts.push({ selected: [...selected], publish });
      if (selected.includes("retained-bad")) {
        throw new CertificationRefusal({ stage: "witness-acquisition", owner: "certifier", reason: "retained-bad unproved" });
      }
      return { published: [...selected] };
    }
  });
  assert.deepEqual(result.published, ["retained-ok", "frontier"]);
  assert.deepEqual(recovery.publishedCases, ["retained-ok", "frontier"]);
  assert.equal(recovery.strategy, "independent-prepared-selection");
  assert.match(recovery.retainedBaselineRefusal, /retained-bad unproved/);
  assert.deepEqual(recovery.caseRefusals.map(refusal => refusal.stage), ["witness-acquisition"]);
  assert.deepEqual(recovery.caseRefusals.map(refusal => refusal.reason), ["retained-bad unproved"]);
  assert.equal(attempts.at(-1).publish, true);
  assert.deepEqual(attempts.at(-1).selected, ["retained-ok", "frontier"]);
});

test("prepared-set selection subdivides instead of spending a transaction per case", async () => {
  // Each trial re-certifies the whole prepared graph, so the transaction count
  // is the difference between finishing and hitting the certification budget.
  const cases = Array.from({ length: 24 }, (_, index) => `case-${index}`);
  const bad = "case-17";
  const recovery = { cases, retainedCases: cases.slice(0, 20) };
  let trials = 0;
  const result = await certifyRecoverableCaseSelection({
    cases, recovery, existingPublication: false,
    certify: async (selected, publish) => {
      trials++;
      if (selected.includes(bad)) {
        throw new CertificationRefusal({ stage: "witness-acquisition", owner: "certifier", reason: "one bad case" });
      }
      return { published: [...selected] };
    }
  });
  assert.deepEqual(result.published, cases.filter(item => item !== bad));
  assert.deepEqual(recovery.publishedCases, cases.filter(item => item !== bad));
  assert.equal(recovery.caseRefusals.length, 1);
  assert.equal(recovery.caseRefusals[0].reason, "one bad case");
  // Combined + baseline + subdivision + final publish. A prefix walk would
  // spend 24 trials on the subdivision alone.
  assert.ok(trials < 16, `expected a subdivision-shaped transaction count, spent ${trials}`);
});

test("an existing publication is never reduced by the prepared-set selection", async () => {
  const cases = ["retained-bad", "frontier"];
  const failure = new CertificationRefusal({ stage: "witness-acquisition", owner: "certifier", reason: "retained-bad unproved" });
  for (const existingPublication of [true, null, undefined]) {
    const recovery = { cases, retainedCases: ["retained-bad"] };
    await assert.rejects(certifyRecoverableCaseSelection({
      cases, recovery,
      ...(existingPublication === undefined ? {} : { existingPublication }),
      certify: async selected => {
        if (selected.includes("retained-bad")) throw failure;
        return {};
      }
    }), error => error === failure);
    assert.equal(recovery.publishedCases, undefined);
    assert.equal(recovery.strategy, undefined);
  }
});

test("a prepared set that proves nothing rethrows so the proposal fallback still runs", async () => {
  const cases = ["retained-bad", "frontier-bad"];
  const recovery = { cases, retainedCases: ["retained-bad"] };
  const failure = new CertificationRefusal({ stage: "witness-acquisition", owner: "certifier", reason: "nothing provable" });
  let fallbacks = 0;
  const result = await certifyRecoverableCaseSelection({
    cases, recovery, existingPublication: false,
    certify: async () => { throw failure; },
    fallback: async error => { assert.equal(error, failure); fallbacks++; return { authority: "proposal-fallback" }; }
  });
  assert.equal(fallbacks, 1);
  assert.equal(result.authority, "proposal-fallback");
  assert.equal(recovery.publishedCases, undefined);
});

test("recovery final combined publication must pass independently", async () => {
  const cases = ["retained", "candidate"];
  const recovery = { cases, retainedCases: ["retained"] };
  let publications = 0;
  await assert.rejects(certifyRecoverableCaseSelection({ cases, recovery, certify: async (_, publish) => {
    if (publish && ++publications === 1) throw new CertificationRefusal({ stage: "witness-acquisition", owner: "certifier", reason: "initial refusal" });
    if (publish) throw new Error("final conflict");
    return {};
  }}), /final conflict/);
  assert.equal(recovery.publishedCases, undefined);
});

test("a graph refusal can fall back only through fresh retained-proposal certification", async () => {
  const cases = ["retained", "candidate"];
  const recovery = { cases, retainedCases: ["retained"] };
  const failure = new CertificationRefusal({ stage: "witness-acquisition", owner: "certifier", reason: "retained graph cannot prove its dependency" });
  let fallbackCalls = 0;
  const result = await certifyRecoverableCaseSelection({ cases, recovery,
    certify: async () => { throw failure; }, fallback: async error => {
      assert.equal(error, failure); fallbackCalls++; return { authority: "fresh-retained-certification" };
    }
  });
  assert.equal(fallbackCalls, 1);
  assert.equal(result.authority, "fresh-retained-certification");
  await assert.rejects(certifyRecoverableCaseSelection({ cases, recovery,
    certify: async () => { throw failure; }, fallback: async () => { throw new Error("retained certification also failed"); }
  }), /retained certification also failed/);
});

test("recovery selection is bounded and successful combined certification needs no retries", async () => {
  for (const count of [3, 33]) {
    const cases = Array.from({ length: count }, (_, index) => String(index));
    let calls = 0;
    await certifyRecoverableCaseSelection({ cases, recovery: { cases, retainedCases: cases.slice(0, 1) }, certify: async (selected, publish) => {
      calls++; assert.deepEqual(selected, cases); assert.equal(publish, true); return {};
    }});
    assert.equal(calls, 1);
  }
});

// A graph state shaped exactly as `prepareState` leaves one, so the default
// reachability walk under test is the real one.
function graphNodeFixture(key, dependencies = []) {
  const state = {
    node: {
      key,
      packageName: key,
      packageVersion: "1.0.0",
      entrypoint: ".",
      conditions: ["import"],
      dependencies: dependencies.map(dependency => ({
        specifier: dependency.node.key,
        node: dependency.node.key
      }))
    },
    directDependencies: dependencies.map(dependency => ({
      viaSpecifier: dependency.node.key,
      state: dependency
    }))
  };
  return state;
}

test("a refused graph node refuses exactly the artifact cases whose graph reaches it", () => {
  const broken = graphNodeFixture("node-builtin-owner");
  const shared = graphNodeFixture("shared");
  const brokenRoot = graphNodeFixture("root-a", [broken, shared]);
  const intactRoot = graphNodeFixture("root-b", [shared]);
  const byKey = new Map(
    [broken, shared, brokenRoot, intactRoot].map(state => [state.node.key, state])
  );
  const nodeRefusals = new Map([
    ["node-builtin-owner", { stage: "graph-preparation", reason: "node:async_hooks is not a package receipt" }]
  ]);
  const prepared = [
    { root: brokenRoot, artifactCase: { entrypoint: "./server", conditions: ["import", "node"] } },
    { root: intactRoot, artifactCase: { entrypoint: ".", conditions: ["import"] } }
  ];
  const { cases, refusals } = graphCasesWithoutRefusedNodes({ prepared, byKey, nodeRefusals });
  assert.equal(cases.length, 1);
  assert.equal(cases[0].artifactCase.entrypoint, ".");
  // The surviving case keeps its whole graph, including the node it shares
  // with the refused one.
  assert.deepEqual(cases[0].nodes.map(state => state.node.key).sort(), ["root-b", "shared"]);
  assert.equal(refusals.length, 1);
  assert.equal(refusals[0].entrypoint, "./server");
  assert.deepEqual(refusals[0].conditions, ["import", "node"]);
  assert.equal(refusals[0].stage, "graph-preparation");
  assert.match(refusals[0].reason, /graph node node-builtin-owner@1\.0\.0 \. \[import\] refused: node:async_hooks/);
});

test("a refused graph node refuses every node generated against its contract", () => {
  const broken = graphNodeFixture("broken");
  const middle = graphNodeFixture("middle", [broken]);
  const root = graphNodeFixture("root", [middle]);
  const unrelated = graphNodeFixture("unrelated");
  const nodeRefusals = cascadeGraphNodeRefusals(
    [broken, middle, root, unrelated],
    new Map([["broken", { stage: "graph-generation", reason: "closure module was not found" }]])
  );
  assert.deepEqual([...nodeRefusals.keys()].sort(), ["broken", "middle", "root"]);
  assert.equal(nodeRefusals.get("middle").stage, "graph-generation");
  assert.match(nodeRefusals.get("middle").reason, /dependency broken refused: closure module was not found/);
  assert.match(nodeRefusals.get("root").reason, /dependency middle refused: dependency broken refused:/);
  assert.equal(nodeRefusals.has("unrelated"), false);
});

test("a graph that cannot prepare a retained case is abandoned rather than published smaller", () => {
  const retained = [
    { entrypoint: ".", conditions: ["import"] },
    { entrypoint: "./http", conditions: ["import"] }
  ];
  assert.equal(retainedCaseFloorRefusal(retained, new Set(retained), []), null);
  // Only the frontier dropping is not a floor breach: those cases refused at
  // generation, so the proposal never covered them.
  assert.equal(
    retainedCaseFloorRefusal(retained, new Set(retained), [
      { entrypoint: "./config", conditions: ["import"], stage: "graph-preparation", reason: "crossws is not installed" }
    ]),
    null
  );
  const message = retainedCaseFloorRefusal(
    retained,
    new Set([retained[0]]),
    [{ entrypoint: "./http", conditions: ["import"], stage: "graph-preparation", reason: "crossws is not installed" }]
  );
  assert.match(message, /would drop 1 retained artifact case\(s\)/);
  assert.match(message, /starting at \.\/http \[import\]: crossws is not installed/);
});

test("a recovery set above the graph budget is refused before any preparation work", () => {
  assert.equal(recoveryGraphBudgetRefusal(RECOVERY_GRAPH_CASE_BUDGET), null);
  assert.equal(recoveryGraphBudgetRefusal(1), null);
  const message = recoveryGraphBudgetRefusal(RECOVERY_GRAPH_CASE_BUDGET + 1);
  assert.match(message, new RegExp(`prepares ${RECOVERY_GRAPH_CASE_BUDGET + 1} artifact cases`));
  assert.match(message, new RegExp(`above the ${RECOVERY_GRAPH_CASE_BUDGET}-case graph budget`));
  // The measured boundary: 24 prepared cases certify, 59 exceeded the runner's
  // memory ceiling and cost the row its whole publication.
  assert.equal(recoveryGraphBudgetRefusal(24), null);
  assert.notEqual(recoveryGraphBudgetRefusal(59), null);
});

test("a refused node no surviving case reaches leaves every case intact", () => {
  const orphan = graphNodeFixture("orphan");
  const root = graphNodeFixture("root");
  const byKey = new Map([orphan, root].map(state => [state.node.key, state]));
  const { cases, refusals } = graphCasesWithoutRefusedNodes({
    prepared: [{ root, artifactCase: { entrypoint: ".", conditions: ["import"] } }],
    byKey,
    nodeRefusals: new Map([["orphan", { stage: "graph-acquisition", reason: "archive digest mismatch" }]])
  });
  assert.equal(refusals.length, 0);
  assert.equal(cases.length, 1);
});

test("an unpreparable graph still reports the reason and takes no lane", async () => {
  const root = mkdtempSync(join(tmpdir(), "recovery-preparation-"));
  const output = join(root, "proposal.json");
  const generated = { certificationInputs: [{ entrypoint: "./retained", conditions: [] }] };
  const refusals = ["./good", "./bad"].map(entrypoint => ({ entrypoint, conditions: [],
    stage: "artifact-case", class: "dependency-composition", applicability: "runtime-module" }));
  writeFileSync(`${output}.refusals.json`, JSON.stringify({ refusals }));
  try {
    // Preparation isolates a broken node to the cases that reach it, so a
    // throw from it now means no case survived at all. There is no second
    // preparation attempt to make: the caller's proposal is the answer.
    let attempts = 0;
    const result = await preparedGraphForPartialProposal(
      { output, scratch: root, generated, options: { recoverEntrypoints: true } },
      { prepare: async () => { attempts++; throw new Error("published dependency graph prepared no artifact case: virtual module unavailable"); } }
    );
    assert.equal(attempts, 1);
    assert.equal(result.graph, null);
    assert.equal(result.trace.partialProposalFrontier, "unprepared");
    assert.match(result.trace.reason, /prepared no artifact case: virtual module unavailable/);
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test("a partial proposal has a dependency frontier only when a refusal is a dependency composition", () => {
  const binding = {
    reason: "accepted dependency @solidjs/signals has no exact runtime binding for export $PROXY"
  };
  const unresolvedModule = {
    reason:
      "solid-checker:unresolved-dependency-module=@tanstack/pacer\n" +
      'emit package contract: cannot statically expand external export-all "@tanstack/pacer"'
  };
  // Facts about the publisher's own bytes. No dependency catalog moves them, so
  // the graph lane has nothing to prepare for them and the emitted proposal
  // stays the best available answer.
  const publisherDefects = [
    { reason: "emit package contract: entry file <root>/dist/solid.js has no runtime ESM exports" },
    { reason: "no declaration target exists for <root>/dist/solid.cjs" },
    {
      reason:
        "contract emission batch target 45 names entry file <root>/types/jsx.d.ts as its own " +
        "fact source; its suffix makes it a TypeScript declaration file"
    }
  ];
  assert.equal(partialProposalHasDependencyFrontier([binding]), true);
  assert.equal(partialProposalHasDependencyFrontier([unresolvedModule]), true);
  assert.equal(
    partialProposalHasDependencyFrontier([...publisherDefects, binding]),
    true
  );
  assert.equal(partialProposalHasDependencyFrontier(publisherDefects), false);
  // The structured class decides whenever a row carries one; the prose above is
  // the legacy fallback for a census written before the field existed. A row
  // the generator classified as a fact about the publisher's own bytes is not
  // reclassified by a reason that happens to quote a dependency phrase, and a
  // classified dependency row routes without its reason being read at all.
  assert.equal(
    partialProposalHasDependencyFrontier([
      { class: "published-artifact", reason: "accepted dependency x has no exact runtime binding" }
    ]),
    false
  );
  assert.equal(
    partialProposalHasDependencyFrontier([
      { class: "dependency-composition", reason: "unresolved-dependency-module" }
    ]),
    true
  );
  assert.equal(
    partialProposalHasDependencyFrontier([{ class: "resource-limit", reason: "" }]),
    false
  );
  // Nothing to route on is not a frontier.
  assert.equal(partialProposalHasDependencyFrontier([]), false);
  assert.equal(partialProposalHasDependencyFrontier(null), false);
  assert.equal(partialProposalHasDependencyFrontier(undefined), false);
  assert.equal(partialProposalHasDependencyFrontier("accepted dependency"), false);
});

test("a declined dependency frontier names exact cases, and nothing it cannot spell", () => {
  // A malformed coordinate is not an acquisition request. Every one of these
  // would otherwise become a case the graph tries to acquire, and a case
  // acquired on a guessed coordinate is the failure the whole lane is built
  // to avoid -- so each is dropped, and the positive row proves the drop is
  // about the defect and not about the filter rejecting everything.
  assert.deepEqual(declinedDependencyGraphCases(null), []);
  assert.deepEqual(declinedDependencyGraphCases("declined"), []);
  assert.deepEqual(
    declinedDependencyGraphCases([
      { kind: "unaccepted-external-dependency", conditions: [] },
      { kind: "unaccepted-external-dependency", entrypoint: ".", conditions: null },
      { kind: "unaccepted-external-dependency", entrypoint: ".", conditions: "import" },
      { kind: "unaccepted-external-dependency", entrypoint: ".", conditions: [7] },
      { kind: "unresolved-callee", entrypoint: "./refused", conditions: [] },
      { kind: "unaccepted-external-dependency", entrypoint: "./ok", conditions: [] }
    ]),
    [{ entrypoint: "./ok", conditions: ["import"] }]
  );
});

test("the partial-proposal graph lane falls back, and says so, without ever swallowing a refusal", async () => {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-partial-lane-"));
  const output = join(root, "solid-reactivity.json");
  const census = refusals => JSON.stringify({
    format: "solid-checker-contract-proposal-refusals",
    refusalVersion: 1,
    package: { name: "fixture", version: "1.0.0" },
    refusals,
    inapplicable: []
  });
  const frontier = {
    entrypoint: ".",
    conditions: [],
    stage: "artifact-case",
    class: "dependency-composition",
    applicability: "runtime-module",
    reason: "accepted dependency dependency has no exact runtime binding for export default"
  };
  const never = () => {
    throw new Error("preparation must not be attempted");
  };
  try {
    // (a) No census at all, and a census that is not JSON: nothing is known
    // about a frontier, so there is nothing to prepare and nothing to trace.
    // The caller certifies the partial proposal exactly as without the flag.
    assert.deepEqual(
      await preparedGraphForPartialProposal({ output }, { prepare: never }),
      { graph: null, trace: null }
    );
    writeFileSync(`${output}.refusals.json`, "{ not json");
    assert.deepEqual(
      await preparedGraphForPartialProposal({ output }, { prepare: never }),
      { graph: null, trace: null }
    );
    // A census whose refusals are all publisher defects is the same
    // non-request: this lane answers dependency composition only.
    writeFileSync(
      `${output}.refusals.json`,
      census([{ ...frontier, class: "published-artifact", reason: "no runtime ESM exports" }])
    );
    assert.deepEqual(
      await preparedGraphForPartialProposal({ output }, { prepare: never }),
      { graph: null, trace: null }
    );

    // A frontier recorded as closure *declines* rather than refused cases --
    // the shape `@solid-primitives/memo` presents, and the shape whose
    // silence was § 26's no-op. The declines here carry no exact coordinate
    // pair (a census written before those fields existed), so there is no
    // acquisition request to make and the frontier is named instead. That
    // naming is the floor: the audit must always be able to tell this apart
    // from a row that never wanted the lane.
    const declinedCensus = declined => JSON.stringify({
      format: "solid-checker-contract-proposal-refusals",
      refusalVersion: 1,
      package: { name: "fixture", version: "1.0.0" },
      refusals: [],
      inapplicable: [],
      declinedClosures: declined
    });
    writeFileSync(
      `${output}.refusals.json`,
      declinedCensus([
        { stage: "closure-proposal", export: "a", domain: "reads",
          kind: "unaccepted-external-dependency", package: "@scope/dep" },
        { stage: "closure-proposal", export: "b", domain: "creates",
          kind: "unaccepted-external-dependency", package: "@scope/dep" },
        { stage: "closure-proposal", export: "c", domain: "reads",
          kind: "dialect-silent", package: "solid-js" }
      ])
    );
    assert.deepEqual(
      await preparedGraphForPartialProposal(
        { output },
        { prepare: never, prepareCases: never }
      ),
      {
        graph: null,
        trace: {
          partialProposalFrontier: "declined-only",
          reason:
            "the dependency frontier is recorded as closure declines, not as refused artifact cases",
          declinedDependencyRecords: 2,
          declinedDependencySpecifiers: ["@scope/dep"],
          declinedDependencySpecifiersTotal: 1
        }
      }
    );

    // The same frontier with the coordinates the generator actually writes.
    // Now it is an acquisition request: the exact `(entrypoint, conditions)`
    // pairs the declines name, deduplicated, `import` folded in the way every
    // other request here folds it, and the non-dependency decline's case
    // excluded -- `./other` must not be acquired because something unrelated
    // to a dependency declined there.
    const composedCases = [];
    const composed = { timing: { rootCases: 2, canonicalNodes: 5 } };
    writeFileSync(
      `${output}.refusals.json`,
      declinedCensus([
        { stage: "closure-proposal", entrypoint: ".", conditions: ["import"],
          export: "a", domain: "reads",
          kind: "unaccepted-external-dependency", package: "@scope/dep" },
        { stage: "closure-proposal", entrypoint: ".", conditions: [],
          export: "b", domain: "creates",
          kind: "unaccepted-external-dependency", package: "@scope/dep" },
        { stage: "closure-proposal", entrypoint: "./sub", conditions: ["node"],
          export: "c", domain: "reads",
          kind: "unaccepted-external-dependency", package: "@scope/other" },
        { stage: "closure-proposal", entrypoint: "./other", conditions: ["import"],
          export: "d", domain: "reads",
          kind: "dialect-silent", package: "solid-js" }
      ])
    );
    assert.deepEqual(
      await preparedGraphForPartialProposal(
        { output, scratch: root },
        {
          prepare: never,
          prepareCases: async ({ dependencyCases }) => {
            composedCases.push(dependencyCases);
            return composed;
          }
        }
      ),
      { graph: composed, trace: null }
    );
    assert.deepEqual(composedCases, [[
      { entrypoint: ".", conditions: ["import"] },
      { entrypoint: "./sub", conditions: ["import", "node"] }
    ]]);
    assert.deepEqual(composed.timing.declinedDependencyFrontier, {
      declinedDependencyRecords: 3,
      declinedDependencySpecifiers: ["@scope/dep", "@scope/other"],
      declinedDependencySpecifiersTotal: 2,
      composedArtifactCases: 2
    });

    // Composition failing is a fallback to the partial proposal, exactly as a
    // refusal-driven preparation failing is -- and it says which specifier it
    // was reaching for, so the row does not read like one that was never
    // shaped to compose.
    assert.deepEqual(
      await preparedGraphForPartialProposal(
        { output, scratch: root },
        {
          prepare: never,
          prepareCases: async () => {
            throw new Error("registry acquisition failed for @scope/dep@1.0.0");
          }
        }
      ),
      {
        graph: null,
        trace: {
          partialProposalFrontier: "declined-only",
          reason:
            "the dependency frontier is recorded as closure declines, not as refused artifact cases",
          declinedDependencyRecords: 3,
          declinedDependencySpecifiers: ["@scope/dep", "@scope/other"],
          declinedDependencySpecifiersTotal: 2,
          composedArtifactCases: 2,
          preparationRefusal: "registry acquisition failed for @scope/dep@1.0.0"
        }
      }
    );

    // The control: declines that name no dependency stay silent, so the trace
    // above is about the dependency kind and not about declines existing.
    writeFileSync(
      `${output}.refusals.json`,
      declinedCensus([
        { stage: "closure-proposal", entrypoint: ".", conditions: ["import"],
          export: "c", domain: "reads",
          kind: "dialect-silent", package: "solid-js" }
      ])
    );
    assert.deepEqual(
      await preparedGraphForPartialProposal(
        { output },
        { prepare: never, prepareCases: never }
      ),
      { graph: null, trace: null }
    );

    // A real frontier: preparation is attempted, and its result is the lane.
    writeFileSync(`${output}.refusals.json`, census([frontier]));
    const prepared = { timing: { rootCases: 1, canonicalNodes: 3 } };
    assert.deepEqual(
      await preparedGraphForPartialProposal({ output }, { prepare: async () => prepared }),
      { graph: prepared, trace: null }
    );

    // (b) Preparation failing is still a fallback to the partial proposal --
    // the caller has a valid answer a throw would discard -- but the attempt
    // must leave a trace, or a requested lane that never happened reads
    // exactly like a row that never asked for one.
    assert.deepEqual(
      await preparedGraphForPartialProposal(
        { output },
        {
          prepare: async () => {
            throw new Error("registry acquisition failed for dependency@1.0.0");
          }
        }
      ),
      {
        graph: null,
        trace: {
          partialProposalFrontier: "unprepared",
          reason: "registry acquisition failed for dependency@1.0.0"
        }
      }
    );

    for (const mode of ["recover", "unrequested", "retry-fails", "generic-error"]) {
      const attempts = [];
      const recovered = await preparedGraphForPartialProposal(
        { output, scratch: root, options: { recoverEntrypoints: mode !== "unrequested" }, generated: {} },
        { prepare: async input => {
          attempts.push(input);
          if (attempts.length === 1) throw mode === "generic-error"
            ? new Error("retained preparation failed")
            : new RetainedCasePreparationRefusal("retained preparation failed");
          assert.equal(input.retainGeneratedCases, true);
          assert.notEqual(input.scratch, root);
          if (mode === "retry-fails") throw new Error("retained source acquisition failed");
          return { timing: {} };
        } }
      );
      assert.equal(attempts.length, ["recover", "retry-fails"].includes(mode) ? 2 : 1);
      if (mode === "recover") {
        assert.equal(recovered.graph.timing.retainedPreparationFallback.reason, "retained preparation failed");
      } else {
        assert.equal(recovered.graph, null);
        assert.equal(recovered.trace.reason, "retained preparation failed");
        if (mode === "retry-fails") assert.equal(recovered.trace.retainedPreparationRefusal, "retained source acquisition failed");
      }
    }

    // (c) A refusal is not a graph fact. A missing issuer or trust
    // configuration is a request error, and certifying the partial proposal
    // instead would answer a broken request with a receipt.
    await assert.rejects(
      preparedGraphForPartialProposal(
        { output },
        {
          prepare: async () => {
            throw new CertificationRefusal({
              stage: "receiptIssuance",
              owner: "configured-issuer",
              reason: "no issuer configuration"
            });
          }
        }
      ),
      error => {
        assert.equal(error.name, "CertificationRefusal");
        assert.equal(error.reason, "no issuer configuration");
        return true;
      }
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
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

test("only a genuinely absent dynamic optional peer is inapplicable", () => {
  const importer = "/project/packages/app/dist/index.js";
  const missing = () => {
    throw new ArtifactResolutionError("package-not-found", "missing");
  };
  const optionalDynamic = {
    kind: "dynamic",
    specifier: "@solidjs/router",
    optionalPeer: true,
    dynamicImport: true
  };

  assert.equal(
    locateExternalDependencyPackageRoot(importer, optionalDynamic, {
      locatePackage: missing,
      pathExists: () => false
    }),
    null
  );
  for (const dependency of [
    { ...optionalDynamic, kind: "import" },
    { ...optionalDynamic, dynamicImport: false },
    { ...optionalDynamic, optionalPeer: false }
  ]) {
    assert.throws(
      () => locateExternalDependencyPackageRoot(importer, dependency, {
        locatePackage: missing,
        pathExists: () => false
      }),
      error => error instanceof ArtifactResolutionError && error.code === "package-not-found"
    );
  }
  for (const resolutionError of [
    new Error("generic resolution failure"),
    new ArtifactResolutionError("invalid-target", "wrong refusal family")
  ]) {
    assert.throws(
      () => locateExternalDependencyPackageRoot(importer, optionalDynamic, {
        locatePackage: () => { throw resolutionError; },
        pathExists: () => false
      }),
      error => error === resolutionError,
      "only an exact package-not-found refusal can prove absence"
    );
  }
  assert.throws(
    () => locateExternalDependencyPackageRoot(importer, optionalDynamic, {
      locatePackage: missing,
      pathExists: candidate => candidate === "/project/node_modules/@solidjs/router"
    }),
    error => error instanceof ArtifactResolutionError && error.code === "package-not-found",
    "a present but broken package directory remains a refusal"
  );
  const inaccessible = Object.assign(new Error("permission denied"), { code: "EACCES" });
  assert.throws(
    () => locateExternalDependencyPackageRoot(importer, optionalDynamic, {
      locatePackage: missing,
      pathExists: () => { throw inaccessible; }
    }),
    error => error === inaccessible,
    "filesystem I/O failure is not proof of absence"
  );
  assert.equal(
    locateExternalDependencyPackageRoot(importer, optionalDynamic, {
      locatePackage: () => "/project/node_modules/@solidjs/router",
      pathExists: () => false
    }),
    "/project/node_modules/@solidjs/router",
    "an installed optional peer follows ordinary authentication"
  );
});

test("the static runtime importer census includes imports and re-exports at every occurrence", () => {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-reexport-census-"));
  try {
    const packageRoot = join(root, "node_modules", "consumer");
    mkdirSync(join(packageRoot, "dist", "core"), { recursive: true });
    writeFileSync(
      join(packageRoot, "package.json"),
      `${JSON.stringify({
        name: "consumer",
        version: "1.0.0",
        type: "module",
        dependencies: { "shared-dependency": "1.0.0" },
        exports: {
          ".": { types: "./dist/index.d.ts", import: "./dist/index.js" }
        }
      })}\n`
    );
    // One module imports and one re-exports the same specifier. `./dist/core/nested.js`
    // sorts before `./dist/index.js`, so a first-occurrence census names the
    // nested module and drops the entry module -- exactly the shape that made
    // `motion-solidjs@0.6.0`'s entry-module bridge resolve against nothing.
    writeFileSync(
      join(packageRoot, "dist", "index.js"),
      'import { SHARED as source } from "shared-dependency"; export const SHARED = source + "!";\n' +
        'export { NESTED } from "./core/nested.js";\n'
    );
    writeFileSync(
      join(packageRoot, "dist", "index.d.ts"),
      'export { SHARED } from "shared-dependency";\n' +
        'export { NESTED } from "./core/nested.js";\n'
    );
    writeFileSync(
      join(packageRoot, "dist", "core", "nested.js"),
      'export { OTHER as NESTED } from "shared-dependency";\n'
    );
    writeFileSync(
      join(packageRoot, "dist", "core", "nested.d.ts"),
      'export { OTHER as NESTED } from "shared-dependency";\n'
    );
    const dependencyRoot = join(root, "node_modules", "shared-dependency");
    mkdirSync(dependencyRoot, { recursive: true });
    writeFileSync(
      join(dependencyRoot, "package.json"),
      `${JSON.stringify({
        name: "shared-dependency",
        version: "1.0.0",
        type: "module",
        exports: { ".": { types: "./index.d.ts", import: "./index.js" } }
      })}\n`
    );
    writeFileSync(
      join(dependencyRoot, "index.js"),
      'export const SHARED = "shared";\nexport const OTHER = "other";\n'
    );
    writeFileSync(
      join(dependencyRoot, "index.d.ts"),
      'export declare const SHARED: "shared";\nexport declare const OTHER: "other";\n'
    );

    const resolved = resolvePackageArtifactClosure({
      importer: join(root, "app.mjs"),
      specifier: "consumer",
      packageRoot,
      conditions: ["import"],
      resolutionKind: "import",
      integrity: "sha512-consumer"
    });
    const edges = staticRuntimeDependencies(resolved);
    assert.deepEqual(edges.map(edge => edge.kind), ["reexport", "import"]);
    assert(resolved.externalDependencies.some(edge => edge.axis === "declarations"));
    assert(edges.every(edge => edge.axis === "runtime"));
    assert.deepEqual(staticRuntimeDependencies({ externalDependencies: [
      { axis: "runtime", kind: "dynamic", specifier: "late" },
      { axis: "declarations", kind: "import", specifier: "types" }
    ] }), [], "dynamic and declaration edges do not gain semantic authority");
    assert.deepEqual(
      edges.map(edge => edge.importerPath),
      ["./dist/core/nested.js", "./dist/index.js"],
      "the nested module really is the first occurrence in canonical order"
    );

    const entryImporter = join(packageRoot, "dist", "index.js");
    const nestedImporter = join(packageRoot, "dist", "core", "nested.js");
    const census = reexportImporterCensus(packageRoot, edges);
    assert.deepEqual(
      [...(census.get("shared-dependency") ?? [])].sort(),
      [nestedImporter, entryImporter].sort(),
      "the census carries both importing and re-exporting modules of the package"
    );

    // The emitted catalog carries both, with the node's own importer being the
    // first occurrence exactly as `prepareState` records it.
    const proposal = join(root, "dependency-proposal.json");
    writeFileSync(proposal, '{"format":"solid-reactivity-contract"}\n');
    const merged = mergeProposalDependencies(
      [{
        viaSpecifier: "shared-dependency",
        node: { importer: nestedImporter, packageName: "shared-dependency" },
        planning: {
          proposal,
          resolution: {
            importer: nestedImporter,
            specifier: "shared-dependency",
            exports: {}
          }
        },
        demandPlan: {
          selectedArtifactCase: "artifact-case:shared",
          candidateSemanticDigest: `sha256:${"0".repeat(64)}`
        },
        reexportImporters: [...census.get("shared-dependency")].sort()
      }],
      join(root, "catalog")
    );
    assert.deepEqual(
      JSON.parse(readFileSync(merged.catalog, "utf8")).contracts.map(
        contract => contract.import.importer
      ),
      [nestedImporter, entryImporter].sort(),
      "the entry module must be able to ask the catalog about this dependency"
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("the private graph catalog names every module that re-exports a dependency", () => {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-graph-catalog-"));
  try {
    const packageRoot = join(root, "node_modules", "consumer");
    const proposal = join(root, "dependency-proposal.json");
    writeFileSync(proposal, '{"format":"solid-reactivity-contract"}\n');
    const entryImporter = join(packageRoot, "dist", "index.js");
    const helperImporter = join(packageRoot, "dist", "core", "helper.js");
    const dependency = {
      viaSpecifier: "shared-dependency",
      node: { importer: helperImporter, packageName: "shared-dependency" },
      planning: {
        proposal,
        resolution: {
          importer: helperImporter,
          specifier: "shared-dependency",
          exports: { SHARED: { runtime: {}, declarations: {} } }
        }
      },
      demandPlan: {
        selectedArtifactCase: "artifact-case:shared",
        candidateSemanticDigest: `sha256:${"0".repeat(64)}`
      },
      // The alphabetically first re-exporting module is the helper, but the
      // artifact case's entry module re-exports the same specifier and is what
      // emission asks the catalog about.
      reexportImporters: [entryImporter, helperImporter].sort()
    };

    const merged = mergeProposalDependencies([dependency], join(root, "catalog"));
    const catalog = JSON.parse(readFileSync(merged.catalog, "utf8"));
    assert.deepEqual(
      catalog.contracts.map(contract => contract.import.importer),
      [helperImporter, entryImporter].sort(),
      "every re-exporting module of the consumer names the same accepted contract"
    );
    assert.equal(
      new Set(catalog.contracts.map(contract => contract.document)).size,
      1,
      "the entries share one document object rather than duplicating bytes"
    );
    for (const contract of catalog.contracts) {
      assert.equal(contract.import.specifier, "shared-dependency");
      assert.deepEqual(contract.import.exports, {
        SHARED: { runtime: {}, declarations: {} }
      });
    }
    assert.deepEqual(Object.keys(merged.proposalDependencies), ["shared-dependency"]);

    assert.throws(
      () => mergeProposalDependencies(
        [{ ...dependency, reexportImporters: [entryImporter] }],
        join(root, "catalog-mismatch")
      ),
      /names an importer outside its occurrence census/,
      "the node's own resolution must be one of the occurrences"
    );

    const single = mergeProposalDependencies(
      [{ ...dependency, reexportImporters: [] }],
      join(root, "catalog-single")
    );
    assert.deepEqual(
      JSON.parse(readFileSync(single.catalog, "utf8")).contracts.map(
        contract => contract.import.importer
      ),
      [helperImporter],
      "with no occurrence census the node's own importer is the only entry"
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
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

test("both graph execution shapes carry the configured pinned probe paths", () => {
  const root = {
    node: { key: "root", bunLockPath: "/project/bun.lock", lockLocator: "root" },
    planning: { schemaVersion: 1, proposal: "/scratch/root.json", resolution: {} }
  };
  const configured = {
    probeHarnessRoot: "/repository",
    probeNodeExecutable: "/pinned/node",
    probeRecipeCorpus: "/recipes"
  };
  for (const count of [1, 2]) {
    const inputs = {
      cases: Array.from({ length: count }, () => ({ root, nodes: [root] })),
      typefactsExecutable: "/bin/typefacts",
      issuerConfiguration: "/config/issuer.json",
      catalogRoot: "/catalog",
      trustConfigurationOutput: "/config/trust.json"
    };
    const absent = buildPublishedGraphExecutionRequest(inputs);
    assert.equal("probeRecipeCorpus" in absent, false);
    const armed = buildPublishedGraphExecutionRequest({ ...inputs, ...configured });
    for (const [key, value] of Object.entries(configured)) assert.equal(armed[key], value);
    assert.equal("sandboxPolicy" in armed, false, "the adapter cannot declare isolation authority");
  }
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

test("a native semantic refusal attributes the demand and family the audit records", () => {
  // The exact stderr the native certifier writes for an unsupported operation
  // input, wrapped by the two stages it passes through. Before the refusal
  // named its demand, every row of this class produced the same sentence and
  // the sidecar recorded `demandId: null, family: null` for all of them.
  const unsupported =
    "solid-checker-rust: policy-2 proof finalization failed: " +
    "Type Facts certification failed during live export-value verification: " +
    `Type Facts demand sha256:${"3".repeat(64)} is unsupported: operation input ` +
    "artifact-case:097ee468:createMarker:operation:callback-0[0] is reactive/accessor, and the " +
    "implementation census binds only parameter-rooted operation inputs " +
    "(family=recursive-value-shape)";
  assert.deepEqual(nativeRefusalAttribution(unsupported), {
    demandId: `sha256:${"3".repeat(64)}`,
    family: "recursive-value-shape"
  });

  // The locally-open family carries its name in its own position.
  assert.deepEqual(
    nativeRefusalAttribution(
      `Type Facts demand sha256:${"a".repeat(64)} is locally open: argument-binding ` +
        "(artifact-case:331dfa49:createReaction): callback parameter has no exact " +
        "direct-call or resolved-argument flow"
    ),
    { demandId: `sha256:${"a".repeat(64)}`, family: "argument-binding" }
  );

  // Nothing is guessed. A reason this cannot parse stays unattributed rather
  // than being attributed wrongly.
  assert.deepEqual(nativeRefusalAttribution("native checker exited 1"), {
    demandId: null,
    family: null
  });
  assert.deepEqual(nativeRefusalAttribution(undefined), { demandId: null, family: null });

  // These are exactly the two fields the refusal audit copies into
  // `refusal.demandId` and `refusal.family`, so populating them here is what
  // makes the sidecar attributable.
  const refusal = new CertificationRefusal({
    stage: "witness-acquisition",
    owner: "certifier",
    reason: unsupported,
    ...nativeRefusalAttribution(unsupported)
  });
  assert.equal(refusal.demandId, `sha256:${"3".repeat(64)}`);
  assert.equal(refusal.family, "recursive-value-shape");
  assert.match(refusal.message, /witness-acquisition refused for demand sha256:3{64}/);
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
  const metadataRequests = [];
  const fetch_ = async (url, options) => {
    metadataRequests.push({ url, options });
    return {
      ok: true,
      status: 200,
      arrayBuffer: async () => metadata.buffer
    };
  };
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
    assert.deepEqual(metadataRequests, [{
      url: "https://registry.npmjs.org/example",
      options: { headers: { accept: "application/vnd.npm.install-v1+json" } }
    }]);
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
  const events = harness.events();
  assert.equal(events.length, 2);
  assert.deepEqual([events[0].sequence, events[1].sequence], [0, 1]);
  assert.equal(harness.drainedMicrotasks(), 2);
  assert.equal(harness.drainedMacrotasks(), 1);
  assert.equal(flushed, 1);
  // The events container is a frame list rather than an `Array`, because an
  // array's prototype is one more place an inherited `toJSON` can sit and the
  // report path must reach no prototype at all.
  assert.equal(Array.isArray(events), false);
  assert.equal(Object.getPrototypeOf(events), null);
});

test("a frame is serialized without consulting toJSON or any prototype", () => {
  const harness = createRuntimeProbeHarness({ drain: [] });
  harness.emit({ marker: "undeclared-alternative", kind: "callback", ordinal: 0 });
  const frame = createFrameRecord();
  frame.session = "session-1";
  frame.environment = adoptFrameValue({ os: "macos", conditions: ["import", "node"] });
  frame.outcome = createFrameRecord();
  frame.outcome.kind = "completed";
  frame.outcome.events = harness.events();
  const expected =
    '{"session":"session-1","environment":{"os":"macos","conditions":["import","node"]},' +
    '"outcome":{"kind":"completed","events":[{"marker":"undeclared-alternative",' +
    '"kind":"callback","ordinal":0,"sequence":0}]}}';
  assert.equal(serializeFrame(frame), expected);

  // The attack a captured `JSON.stringify` cannot answer: the algorithm
  // performs `Get(value, "toJSON")` on every object it visits, so a package
  // installing one on `Object.prototype` was handed the worker's own run frame
  // and could return a laundered copy. The frame serializer consults neither
  // `toJSON` nor a prototype chain, so the same patch — visibly diverting the
  // ordinary path in the same breath — does not reach it.
  const inherited = Object.getOwnPropertyDescriptor(Object.prototype, "toJSON");
  try {
    Object.defineProperty(Object.prototype, "toJSON", {
      configurable: true,
      value: () => ({ laundered: true })
    });
    assert.equal(JSON.stringify({ session: "session-1" }), '{"laundered":true}');
    assert.equal(serializeFrame(frame), expected);
  } finally {
    delete Object.prototype.toJSON;
    if (inherited) Object.defineProperty(Object.prototype, "toJSON", inherited);
  }

  // An ordinary object never reaches the wire: it is refused rather than
  // described, so one reaching a frame by accident fails the launch.
  assert.throws(() => serializeFrame({ session: "session-1" }), /null-prototype/);
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
    // The "." entrypoint under the empty/default partition reaches its missing
    // target through standard conditions only. No disposition rule applies: the
    // row's refusal stands and the inapplicable census stays empty.
    assert.deepEqual(audit.inapplicable, []);
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

// A minimal installed tree for the root-path declaration-only source walk:
// `root-package`'s typings import `alpha` and `beta`, both installed side by
// side, and only the ones the caller lists reach the Bun lockfile.
function writeRootSourceInstall(project, { lockedNames, integrityOf = name => `sha512-${name}` }) {
  const write = (path, body) => {
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, body);
  };
  const declarationOnly = name => {
    write(
      join(project, "node_modules", name, "package.json"),
      `{"name":"${name}","version":"1.0.0","exports":{".":{"types":"./types/index.d.ts","import":"./dist/index.js"}}}\n`
    );
    write(join(project, "node_modules", name, "types/index.d.ts"), `export type T = () => void;\n`);
    write(join(project, "node_modules", name, "dist/index.js"), `export {};\n`);
  };
  write(
    join(project, "node_modules/root-package/package.json"),
    `{"name":"root-package","version":"1.0.0","exports":{".":{"types":"./types/index.d.ts","import":"./dist/index.js"}}}\n`
  );
  write(
    join(project, "node_modules/root-package/types/index.d.ts"),
    `import type { T as A } from "alpha";\nimport type { T as B } from "beta";\nexport declare const value: A | B;\n`
  );
  write(join(project, "node_modules/root-package/dist/index.js"), `export const value = () => {};\n`);
  declarationOnly("alpha");
  declarationOnly("beta");
  const packages = Object.fromEntries(
    lockedNames.map(name => [name, [`${name}@1.0.0`, "", {}, integrityOf(name)]])
  );
  write(join(project, "bun.lock"), `${JSON.stringify({ lockfileVersion: 2, packages }, null, 2)}\n`);
}

function rootSourceGenerated(project) {
  return {
    certificationInputs: [
      {
        entrypoint: ".",
        conditions: ["import"],
        resolution: {
          specifier: "root-package",
          importer: join(project, "solid-checker-certification-importer.ts"),
          packageRoot: join(project, "node_modules/root-package")
        }
      }
    ]
  };
}

// Serves any package the registry stub was told about; every other name fails
// acquisition the way an unreachable registry would.
function registryStub(served, requests = []) {
  const origin = "https://registry.npmjs.org/";
  return async url => {
    requests.push(url);
    // `<origin>/<name>` for a packument, `<origin>/<name>/-/<file>.tgz` for the
    // archive; both address the same package name.
    const path = decodeURIComponent(url.slice(origin.length));
    const name = path.split("/-/")[0];
    const record = served[name];
    if (!record) return { ok: false, status: 404, statusText: "Not Found" };
    if (url.endsWith(".tgz")) {
      return { ok: true, status: 200, arrayBuffer: async () => record.archive };
    }
    return {
      ok: true,
      status: 200,
      arrayBuffer: async () =>
        new TextEncoder().encode(
          JSON.stringify({
            versions: {
              "1.0.0": {
                name,
                version: "1.0.0",
                dist: {
                  integrity: record.integrity ?? `sha512-${name}`,
                  tarball: `https://registry.npmjs.org/${name}/-/${name}-1.0.0.tgz`
                }
              }
            }
          })
        ).buffer
    };
  };
}

test("root certification names only the declaration-only packages the lockfile selects", async () => {
  const project = mkdtempSync(join(tmpdir(), "solid-checker-root-sources-"));
  try {
    writeRootSourceInstall(project, { lockedNames: ["alpha", "beta"] });
    const scratch = join(project, "scratch");
    mkdirSync(scratch, { recursive: true });
    const archive = new TextEncoder().encode("not a real tarball").buffer;
    const [emitted] = await acquireRootCompilerSources({
      options: {
        packageRoot: join(project, "node_modules/root-package"),
        registryOrigin: "https://registry.npmjs.org",
        integrity: "sha512-root-package"
      },
      generated: rootSourceGenerated(project),
      scratch,
      fetch_: registryStub({ alpha: { archive }, beta: { archive } })
    });
    assert.deepEqual(
      emitted.map(source => source.packageName).sort(),
      ["alpha", "beta"],
      "both lock-selected declaration-only packages are named"
    );
    for (const source of emitted) {
      assert.equal(source.registryOrigin, "https://registry.npmjs.org");
      assert.ok(existsSync(source.archive), "an emitted source names acquired archive bytes");
      assert.ok(existsSync(source.registryMetadata));
      assert.equal(
        source.installedPackageRoot,
        join(project, "node_modules", source.packageName)
      );
    }
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("repeated root source acquisition preserves identities without reusing scratch files", async () => {
  const project = mkdtempSync(join(tmpdir(), "solid-checker-root-source-retries-"));
  try {
    writeRootSourceInstall(project, { lockedNames: ["alpha", "beta"] });
    const scratch = join(project, "scratch");
    mkdirSync(scratch);
    const archive = new TextEncoder().encode("first archive").buffer;
    const args = {
      options: { packageRoot: join(project, "node_modules/root-package"),
        registryOrigin: "https://registry.npmjs.org", integrity: "sha512-root-package" },
      generated: rootSourceGenerated(project), scratch,
      fetch_: registryStub({ alpha: { archive }, beta: { archive } })
    };
    const [first] = await acquireRootCompilerSources(args);
    const [second] = await acquireRootCompilerSources(args);
    const identity = source => [source.packageName, source.packageVersion,
      source.lockfile, source.lockLocator, source.installedPackageRoot];
    assert.deepEqual(first.map(source => source.packageName), ["alpha", "beta"]);
    assert.deepEqual(second.map(identity), first.map(identity));
    for (let index = 0; index < first.length; index++) {
      assert.notEqual(second[index].archive, first[index].archive);
      assert.equal(readFileSync(first[index].archive, "utf8"), "first archive");
    }
    const [third] = await acquireRootCompilerSources({ ...args,
      fetch_: registryStub({ alpha: { archive: new TextEncoder().encode("new archive").buffer } }) });
    assert.deepEqual(third.map(source => source.packageName), ["alpha"],
      "a genuine acquisition failure still withholds the package on a later attempt");
    assert.equal(readFileSync(first[0].archive, "utf8"), "first archive");
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("root certification withholds a whole package name it could not name or acquire", async () => {
  const project = mkdtempSync(join(tmpdir(), "solid-checker-root-sources-"));
  try {
    // `beta` is installed but absent from the lockfile, so it can never be
    // authenticated. Naming only `alpha` would be fine; the point of this test
    // is that the withheld name never appears.
    writeRootSourceInstall(project, { lockedNames: ["alpha"] });
    const scratch = join(project, "scratch");
    mkdirSync(scratch, { recursive: true });
    const archive = new TextEncoder().encode("not a real tarball").buffer;
    const [emitted] = await acquireRootCompilerSources({
      options: {
        packageRoot: join(project, "node_modules/root-package"),
        registryOrigin: "https://registry.npmjs.org",
        integrity: "sha512-root-package"
      },
      generated: rootSourceGenerated(project),
      scratch,
      fetch_: registryStub({ alpha: { archive } })
    });
    assert.deepEqual(
      emitted.map(source => source.packageName),
      ["alpha"],
      "a package the lockfile does not select is never named"
    );
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("root certification withholds a name whose published bytes it could not acquire", async () => {
  const project = mkdtempSync(join(tmpdir(), "solid-checker-root-sources-"));
  try {
    writeRootSourceInstall(project, { lockedNames: ["alpha", "beta"] });
    const scratch = join(project, "scratch");
    mkdirSync(scratch, { recursive: true });
    const archive = new TextEncoder().encode("not a real tarball").buffer;
    // The registry serves `alpha` and refuses `beta`. An unserved name must be
    // withheld rather than silently omitted from acquisition but kept in the
    // emitted set.
    const [emitted] = await acquireRootCompilerSources({
      options: {
        packageRoot: join(project, "node_modules/root-package"),
        registryOrigin: "https://registry.npmjs.org",
        integrity: "sha512-root-package"
      },
      generated: rootSourceGenerated(project),
      scratch,
      fetch_: registryStub({ alpha: { archive } })
    });
    assert.deepEqual(
      emitted.map(source => source.packageName),
      ["alpha"],
      "a package whose archive cannot be acquired is withheld"
    );
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

// The three cases of the `dependency-target-not-exported` policy. Each writes
// the same minimal install and differs only in what `alpha` exports and what
// the root's runtime module imports from it.
function writeSubpathInstall(project, {
  alphaSubpath = null,
  alphaExports = null,
  alphaDropExports = false,
  alphaFiles = {},
  rootImport = null,
  rootTypeImport = null
}) {
  writeRootSourceInstall(project, { lockedNames: ["alpha", "beta"] });
  const manifestPath = join(project, "node_modules/alpha/package.json");
  const alpha = JSON.parse(readFileSync(manifestPath, "utf8"));
  if (alphaSubpath) {
    alpha.exports["./web"] = alphaSubpath;
    writeFileSync(join(project, "node_modules/alpha/types/web.d.ts"), "export type W = () => void;\n");
    writeFileSync(join(project, "node_modules/alpha/dist/web.js"), "export {};\n");
  }
  if (alphaExports) alpha.exports = alphaExports;
  if (alphaDropExports) {
    delete alpha.exports;
    alpha.main = "index.js";
    alpha.types = "./types/index.d.ts";
    writeFileSync(join(project, "node_modules/alpha/index.js"), "export {};\n");
  }
  writeFileSync(manifestPath, `${JSON.stringify(alpha)}\n`);
  for (const [relativePath, body] of Object.entries(alphaFiles)) {
    const target = join(project, "node_modules/alpha", relativePath);
    mkdirSync(dirname(target), { recursive: true });
    writeFileSync(target, body);
  }
  if (rootImport) {
    writeFileSync(
      join(project, "node_modules/root-package/dist/index.js"),
      `import ${JSON.stringify(rootImport)};\nexport const value = () => {};\n`
    );
  }
  if (rootTypeImport) {
    writeFileSync(
      join(project, "node_modules/root-package/types/index.d.ts"),
      `import type { T as A } from ${JSON.stringify(rootTypeImport)};\nexport declare const value: A;\n`
    );
  }
}

// Runs the root source walk and returns either the emitted source names or the
// refusal, so a case's disposition is one assertion either way.
async function rootSources(project, generated = null) {
  const scratch = join(project, "scratch");
  mkdirSync(scratch, { recursive: true });
  const archive = new TextEncoder().encode("not a real tarball").buffer;
  return acquireRootCompilerSources({
    options: {
      packageRoot: join(project, "node_modules/root-package"),
      registryOrigin: "https://registry.npmjs.org",
      integrity: "sha512-root-package"
    },
    generated: generated ?? rootSourceGenerated(project),
    scratch,
    fetch_: registryStub({ alpha: { archive }, beta: { archive } })
  }).then(
    ([emitted]) => ({ names: emitted.map(source => source.packageName).sort() }),
    error => ({ refusal: error })
  );
}

test("a dependency shipping no exports field never answers not-exported for a subpath", async () => {
  const project = mkdtempSync(join(tmpdir(), "solid-checker-root-sources-"));
  try {
    // The `picomatch@2.3.2` shape: no `exports` field at all, and an
    // extensionless subpath whose real file is `lib/utils.js`. Node applies
    // PACKAGE_EXPORTS_RESOLVE only when `exports` is present; without it the
    // subpath is legacy path resolution, so the package excludes nothing and
    // there is nothing to refuse.
    writeSubpathInstall(project, {
      alphaDropExports: true,
      alphaFiles: { "lib/utils.js": "export {};\n" },
      rootImport: "alpha/lib/utils"
    });
    const result = await rootSources(project);
    assert.equal(result.refusal, undefined, `unexpected refusal: ${result.refusal?.reason}`);
    assert.deepEqual(result.names, ["alpha", "beta"]);
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("a dependency shipping no exports field resolves an exact .js subpath", async () => {
  const project = mkdtempSync(join(tmpdir(), "solid-checker-root-sources-"));
  try {
    // The `fetch-blob@3.2.0` shape: no `exports`, and the request already
    // carries its extension. `fetch-blob/from.js` is a published file.
    writeSubpathInstall(project, {
      alphaDropExports: true,
      alphaFiles: { "from.js": "export {};\n" },
      rootImport: "alpha/from.js"
    });
    const result = await rootSources(project);
    assert.equal(result.refusal, undefined, `unexpected refusal: ${result.refusal?.reason}`);
    assert.deepEqual(result.names, ["alpha", "beta"]);
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("a type-only import of a non-exported subpath never refuses the case", async () => {
  const project = mkdtempSync(join(tmpdir(), "solid-checker-root-sources-"));
  try {
    // The `jiti/lib/types` shape: an `exports` map that really does exclude
    // the subpath, reached only from a declaration file. A type import is
    // erased before anything resolves it, so the case's runtime never asks.
    writeSubpathInstall(project, { rootTypeImport: "alpha/lib/types" });
    const result = await rootSources(project);
    assert.equal(result.refusal, undefined, `unexpected refusal: ${result.refusal?.reason}`);
    assert.ok(result.names.includes("alpha"), "the dependency is still named");
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("a runtime import of a subpath a real exports map excludes refuses the case", async () => {
  const project = mkdtempSync(join(tmpdir(), "solid-checker-root-sources-"));
  try {
    // The `solid-js` 2.0 shape, exactly: an `exports` map whose keys are `.`,
    // `./refresh`, `./types/*` and `./package.json`, and a runtime import of
    // `./web` — the path Solid 1.x published and 2.0 retired. `./types/*` is a
    // pattern, and it does not match `./web`.
    writeSubpathInstall(project, {
      alphaExports: {
        ".": { types: "./types/index.d.ts", import: "./dist/index.js" },
        "./refresh": "./dist/refresh.js",
        "./types/*": "./types/*",
        "./package.json": "./package.json"
      },
      alphaFiles: { "dist/refresh.js": "export {};\n" },
      rootImport: "alpha/web"
    });
    const result = await rootSources(project);
    assert.ok(result.refusal, "an excluded runtime subpath must refuse the case");
    assert.equal(result.refusal.stage, "artifact-case");
    assert.match(
      result.refusal.reason,
      /dependency-target-not-exported: alpha\/web is not exported by alpha@1\.0\.0 under conditions \[import\]/
    );
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("an exports pattern that matches the subpath is a match, not an exclusion", async () => {
  const project = mkdtempSync(join(tmpdir(), "solid-checker-root-sources-"));
  try {
    // The wildcard control for the premise above: a `./*` key matches every
    // subpath, so no map containing one can answer `not-exported`.
    writeSubpathInstall(project, {
      alphaExports: {
        ".": { types: "./types/index.d.ts", import: "./dist/index.js" },
        "./*": "./dist/*"
      },
      alphaFiles: { "dist/web.js": "export {};\n" },
      rootImport: "alpha/web.js"
    });
    const result = await rootSources(project);
    assert.equal(result.refusal, undefined, `unexpected refusal: ${result.refusal?.reason}`);
    assert.ok(result.names.includes("alpha"));
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("an artifact case importing a subpath its dependency does not export is refused", async () => {
  const project = mkdtempSync(join(tmpdir(), "solid-checker-root-sources-"));
  try {
    // `alpha` exports `.` and nothing else, and the root's runtime module
    // imports `alpha/web` — the shape `@solid-primitives/favicon`'s compiled
    // output has against Solid 2, which dropped the `./web` subpath. The
    // dependency is installed, its manifest is the published one and the
    // lockfile selects it, so no witness program can make this import
    // resolve: the case is refused, and refused as a case rather than by
    // unnaming `alpha`.
    writeSubpathInstall(project, { rootImport: "alpha/web" });
    const scratch = join(project, "scratch");
    mkdirSync(scratch, { recursive: true });
    const archive = new TextEncoder().encode("not a real tarball").buffer;
    const refusal = await acquireRootCompilerSources({
      options: {
        packageRoot: join(project, "node_modules/root-package"),
        registryOrigin: "https://registry.npmjs.org",
        integrity: "sha512-root-package"
      },
      generated: rootSourceGenerated(project),
      scratch,
      fetch_: registryStub({ alpha: { archive }, beta: { archive } })
    }).then(
      () => null,
      error => error
    );
    assert.ok(refusal, "the case must be refused, not certified with the module missing");
    assert.equal(refusal.name, "CertificationRefusal");
    assert.equal(refusal.stage, "artifact-case");
    assert.equal(refusal.owner, "certifier");
    assert.match(refusal.reason, /^artifact case \. \[import\] imports dependency-target-not-exported: /);
    assert.match(refusal.reason, /alpha\/web is not exported by alpha@1\.0\.0 under conditions \[import\]/);
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("a sibling case importing a subpath its dependency does export is not refused", async () => {
  const project = mkdtempSync(join(tmpdir(), "solid-checker-root-sources-"));
  try {
    writeSubpathInstall(project, {
      alphaSubpath: { types: "./types/web.d.ts", import: "./dist/web.js" },
      rootImport: "alpha/web"
    });
    const scratch = join(project, "scratch");
    mkdirSync(scratch, { recursive: true });
    const archive = new TextEncoder().encode("not a real tarball").buffer;
    const [emitted] = await acquireRootCompilerSources({
      options: {
        packageRoot: join(project, "node_modules/root-package"),
        registryOrigin: "https://registry.npmjs.org",
        integrity: "sha512-root-package"
      },
      generated: rootSourceGenerated(project),
      scratch,
      fetch_: registryStub({ alpha: { archive }, beta: { archive } })
    });
    assert.deepEqual(
      emitted.map(source => source.packageName).sort(),
      ["alpha", "beta"],
      "an exported subpath refuses nothing and still names every source"
    );
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("a subpath exported only under a condition this run selects is not refused", async () => {
  const project = mkdtempSync(join(tmpdir(), "solid-checker-root-sources-"));
  try {
    // The subpath exists only behind `solid`. The run selects it, so the
    // import resolves and there is nothing to refuse — the negative control
    // for the policy above, which must key on the exports map's answer under
    // the run's own conditions and not on the subpath's spelling.
    writeSubpathInstall(project, {
      alphaSubpath: { solid: { types: "./types/web.d.ts", import: "./dist/web.js" } },
      rootImport: "alpha/web"
    });
    const scratch = join(project, "scratch");
    mkdirSync(scratch, { recursive: true });
    const archive = new TextEncoder().encode("not a real tarball").buffer;
    const generated = rootSourceGenerated(project);
    generated.certificationInputs[0].conditions = ["import", "solid"];
    const [emitted] = await acquireRootCompilerSources({
      options: {
        packageRoot: join(project, "node_modules/root-package"),
        registryOrigin: "https://registry.npmjs.org",
        integrity: "sha512-root-package"
      },
      generated,
      scratch,
      fetch_: registryStub({ alpha: { archive }, beta: { archive } })
    });
    assert.deepEqual(
      emitted.map(source => source.packageName).sort(),
      ["alpha", "beta"],
      "a condition-gated subpath the run selects resolves like any other"
    );
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("a resolution failure that is not a missing export still names the located package", async () => {
  const project = mkdtempSync(join(tmpdir(), "solid-checker-root-sources-"));
  try {
    // The batch-1 defect, in the shape that survives the policy above: the
    // subpath IS exported, but only behind a condition this run does not
    // select. That is `conditions-unmatched`, deliberately still the
    // unresolved-dependency frontier — the located package's declarations are
    // supplied and its name is not withheld, which is what collapsed
    // `Component<Props>` to `any` before.
    writeSubpathInstall(project, {
      alphaSubpath: { "alpha/private-condition": "./dist/web.js" },
      rootImport: "alpha/web"
    });
    const scratch = join(project, "scratch");
    mkdirSync(scratch, { recursive: true });
    const archive = new TextEncoder().encode("not a real tarball").buffer;
    const [emitted] = await acquireRootCompilerSources({
      options: {
        packageRoot: join(project, "node_modules/root-package"),
        registryOrigin: "https://registry.npmjs.org",
        integrity: "sha512-root-package"
      },
      generated: rootSourceGenerated(project),
      scratch,
      fetch_: registryStub({ alpha: { archive }, beta: { archive } })
    });
    assert.deepEqual(
      emitted.map(source => source.packageName).sort(),
      ["alpha", "beta"],
      "the package behind an unresolvable-but-declared subpath is still named"
    );
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("a project with no Bun lockfile names no declaration-only source at all", async () => {
  const project = mkdtempSync(join(tmpdir(), "solid-checker-root-sources-"));
  try {
    writeRootSourceInstall(project, { lockedNames: ["alpha", "beta"] });
    rmSync(join(project, "bun.lock"));
    const scratch = join(project, "scratch");
    mkdirSync(scratch, { recursive: true });
    const emitted = await acquireRootCompilerSources({
      options: {
        packageRoot: join(project, "node_modules/root-package"),
        registryOrigin: "https://registry.npmjs.org",
        integrity: "sha512-root-package"
      },
      generated: rootSourceGenerated(project),
      scratch,
      fetch_: async () => {
        throw new Error("no lockfile means no acquisition may be attempted");
      }
    });
    assert.deepEqual(emitted, [[]]);
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

// Two entrypoints of one package that reach two *different* copies of one
// dependency name: `.` reaches the hoisted `alpha`, `./sub` reaches a nested
// `alpha` under `mid` that the lockfile does not select.
//
// This is the substitution shape. Materializing the hoisted copy because one
// entrypoint could name it, while the copy that actually governs the other
// entrypoint is missing, does not leave `alpha` unresolved — bundler resolution
// walks up and answers from the wrong version. The name has to go entirely.
function writeShadowedSourceInstall(project) {
  const write = (path, body) => {
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, body);
  };
  const declarationOnly = (root, name, version, body) => {
    write(
      join(root, "package.json"),
      `{"name":"${name}","version":"${version}","exports":{".":{"types":"./types/index.d.ts","import":"./dist/index.js"}}}\n`
    );
    write(join(root, "types/index.d.ts"), body);
    write(join(root, "dist/index.js"), `export {};\n`);
  };
  const rootPackage = join(project, "node_modules/root-package");
  write(
    join(rootPackage, "package.json"),
    `{"name":"root-package","version":"1.0.0","exports":{".":{"types":"./types/index.d.ts","import":"./dist/index.js"},"./sub":{"types":"./sub/types/index.d.ts","import":"./sub/dist/index.js"}}}\n`
  );
  write(
    join(rootPackage, "types/index.d.ts"),
    `import type { T } from "alpha";\nexport declare const value: T;\n`
  );
  write(join(rootPackage, "dist/index.js"), `export const value = () => {};\n`);
  write(
    join(rootPackage, "sub/types/index.d.ts"),
    `import type { T } from "mid";\nexport declare const other: T;\n`
  );
  write(join(rootPackage, "sub/dist/index.js"), `export const other = () => {};\n`);

  declarationOnly(
    join(project, "node_modules/alpha"),
    "alpha",
    "1.0.0",
    `export type T = () => void;\n`
  );
  declarationOnly(
    join(project, "node_modules/mid"),
    "mid",
    "1.0.0",
    `import type { T as A } from "alpha";\nexport type T = A;\n`
  );
  declarationOnly(
    join(project, "node_modules/mid/node_modules/alpha"),
    "alpha",
    "2.0.0",
    `export type T = { notCallable: true };\n`
  );

  // Only the hoisted alpha@1.0.0 and mid@1.0.0 are selectable; the nested
  // alpha@2.0.0 that governs `mid` is not.
  write(
    join(project, "bun.lock"),
    `${JSON.stringify(
      {
        lockfileVersion: 2,
        packages: {
          alpha: ["alpha@1.0.0", "", {}, "sha512-alpha"],
          mid: ["mid@1.0.0", "", {}, "sha512-mid"]
        }
      },
      null,
      2
    )}\n`
  );
  return rootPackage;
}

test("a package name one entrypoint could not authenticate is withheld from every entrypoint", async () => {
  const project = mkdtempSync(join(tmpdir(), "solid-checker-root-sources-"));
  try {
    const rootPackage = writeShadowedSourceInstall(project);
    const scratch = join(project, "scratch");
    mkdirSync(scratch, { recursive: true });
    const archive = new TextEncoder().encode("not a real tarball").buffer;
    const emitted = await acquireRootCompilerSources({
      options: {
        packageRoot: rootPackage,
        registryOrigin: "https://registry.npmjs.org",
        integrity: "sha512-root-package"
      },
      generated: {
        certificationInputs: [
          {
            entrypoint: ".",
            conditions: ["import"],
            resolution: {
              specifier: "root-package",
              importer: join(project, "solid-checker-certification-importer.ts"),
              packageRoot: rootPackage
            }
          },
          {
            entrypoint: "./sub",
            conditions: ["import"],
            resolution: {
              specifier: "root-package/sub",
              importer: join(project, "solid-checker-certification-importer.ts"),
              packageRoot: rootPackage
            }
          }
        ]
      },
      scratch,
      fetch_: registryStub({ alpha: { archive }, mid: { archive } })
    });
    const names = emitted.flat().map(source => source.packageName);
    assert.ok(
      !names.includes("alpha"),
      `the hoisted alpha must not stand in for the nested copy; got ${JSON.stringify(names)}`
    );
    // `mid` still travels: one unnameable grandchild poisons its own name, not
    // the whole collected subtree.
    assert.deepEqual([...new Set(names)], ["mid"]);
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

// ---------------------------------------------------------------------------
// Registry acquisition cache. An entry is addressed by exact (origin, name,
// version, integrity) and is only ever *used* when its archive still hashes to
// that integrity and its packument still names it — the fresh path's checks.
// ---------------------------------------------------------------------------

function sriOf(bytes) {
  return `sha512-${createHash("sha512").update(new Uint8Array(bytes)).digest("base64")}`;
}

async function withRegistryCache(cacheRoot, body) {
  const previous = process.env.SOLID_CHECKER_REGISTRY_CACHE;
  process.env.SOLID_CHECKER_REGISTRY_CACHE = cacheRoot;
  try {
    return await body();
  } finally {
    if (previous === undefined) delete process.env.SOLID_CHECKER_REGISTRY_CACHE;
    else process.env.SOLID_CHECKER_REGISTRY_CACHE = previous;
  }
}

function cacheEntries(cacheRoot) {
  const entries = [];
  const format = join(cacheRoot, "v1");
  if (!existsSync(format)) return entries;
  for (const shard of readdirSync(format)) {
    for (const entry of readdirSync(join(format, shard))) {
      if (!entry.includes(".staging-")) entries.push(join(format, shard, entry));
    }
  }
  return entries.sort();
}

async function acquireAlphaBeta(project, scratchName, fetch_) {
  const scratch = join(project, scratchName);
  mkdirSync(scratch, { recursive: true });
  const [emitted] = await acquireRootCompilerSources({
    options: {
      packageRoot: join(project, "node_modules/root-package"),
      registryOrigin: "https://registry.npmjs.org",
      integrity: "sha512-root-package"
    },
    generated: rootSourceGenerated(project),
    scratch,
    fetch_
  });
  return emitted.sort((left, right) => left.packageName.localeCompare(right.packageName));
}

test("registryCacheRoot and registryAcquisitionConcurrency read their environment exactly", () => {
  assert.equal(registryCacheRoot({}), null);
  assert.equal(registryCacheRoot({ SOLID_CHECKER_REGISTRY_CACHE: "" }), null);
  assert.equal(
    registryCacheRoot({ SOLID_CHECKER_REGISTRY_CACHE: "/tmp/registry-cache" }),
    "/tmp/registry-cache"
  );
  assert.equal(registryAcquisitionConcurrency({}), 8);
  assert.equal(registryAcquisitionConcurrency({ SOLID_CHECKER_REGISTRY_CONCURRENCY: "" }), 8);
  assert.equal(registryAcquisitionConcurrency({ SOLID_CHECKER_REGISTRY_CONCURRENCY: "3" }), 3);
  for (const raw of ["0", "-1", "1.5", "many"]) {
    assert.throws(
      () => registryAcquisitionConcurrency({ SOLID_CHECKER_REGISTRY_CONCURRENCY: raw }),
      /positive integer/
    );
  }
});

test("a registry acquisition that authenticates is reused without a network round trip", async () => {
  const project = mkdtempSync(join(tmpdir(), "solid-checker-registry-cache-"));
  const cacheRoot = join(project, "registry-cache");
  try {
    const archives = {
      alpha: new TextEncoder().encode("alpha archive bytes").buffer,
      beta: new TextEncoder().encode("beta archive bytes").buffer
    };
    const integrityOf = name => sriOf(archives[name]);
    writeRootSourceInstall(project, { lockedNames: ["alpha", "beta"], integrityOf });
    const served = {
      alpha: { archive: archives.alpha, integrity: integrityOf("alpha") },
      beta: { archive: archives.beta, integrity: integrityOf("beta") }
    };

    await withRegistryCache(cacheRoot, async () => {
      const coldRequests = [];
      const cold = await acquireAlphaBeta(project, "scratch-cold", registryStub(served, coldRequests));
      assert.deepEqual(cold.map(source => source.packageName), ["alpha", "beta"]);
      assert.equal(coldRequests.length, 4, "a cold cache fetches packument and archive per source");
      assert.equal(cacheEntries(cacheRoot).length, 2, "both authenticated acquisitions are kept");

      const warmRequests = [];
      const warm = await acquireAlphaBeta(project, "scratch-warm", registryStub(served, warmRequests));
      assert.deepEqual(warmRequests, [], "a warm cache performs no registry request");
      assert.deepEqual(warm.map(source => source.packageName), ["alpha", "beta"]);
      for (const [index, source] of warm.entries()) {
        assert.ok(
          source.archive.startsWith(cacheRoot),
          "a warm acquisition names the validated cache bytes in place instead of copying them"
        );
        assert.deepEqual(readFileSync(source.archive), readFileSync(cold[index].archive));
        assert.deepEqual(
          readFileSync(source.registryMetadata),
          readFileSync(cold[index].registryMetadata)
        );
      }
    });
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("a cache entry whose archive no longer hashes to its integrity is discarded, not used", async () => {
  const project = mkdtempSync(join(tmpdir(), "solid-checker-registry-cache-"));
  const cacheRoot = join(project, "registry-cache");
  try {
    const archive = new TextEncoder().encode("alpha archive bytes").buffer;
    const integrity = sriOf(archive);
    writeRootSourceInstall(project, { lockedNames: ["alpha"], integrityOf: () => integrity });
    const served = { alpha: { archive, integrity } };

    await withRegistryCache(cacheRoot, async () => {
      await acquireAlphaBeta(project, "scratch-cold", registryStub(served));
      const [entry] = cacheEntries(cacheRoot);
      assert.ok(entry, "the authenticated acquisition was cached");
      writeFileSync(join(entry, "package.tgz"), "tampered archive bytes");

      const requests = [];
      const [source] = await acquireAlphaBeta(project, "scratch-warm", registryStub(served, requests));
      assert.equal(requests.length, 2, "the tampered entry forces a fresh acquisition");
      assert.equal(sriOf(readFileSync(source.archive)), integrity, "the emitted archive is the registry's");
      const [replaced] = cacheEntries(cacheRoot);
      assert.equal(replaced, entry, "the entry is re-addressed identically");
      assert.equal(sriOf(readFileSync(join(entry, "package.tgz"))), integrity, "and refilled with authenticated bytes");
    });
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("an acquisition whose archive does not match the pinned integrity is never cached", async () => {
  const project = mkdtempSync(join(tmpdir(), "solid-checker-registry-cache-"));
  const cacheRoot = join(project, "registry-cache");
  try {
    // The stub's default `sha512-<name>` integrity is not the hash of the
    // bytes it serves: Rust will refuse these, so remembering them would only
    // replay a refusal from disk.
    writeRootSourceInstall(project, { lockedNames: ["alpha"] });
    const archive = new TextEncoder().encode("not a real tarball").buffer;
    await withRegistryCache(cacheRoot, async () => {
      const [source] = await acquireAlphaBeta(project, "scratch", registryStub({ alpha: { archive } }));
      assert.equal(source.packageName, "alpha", "the acquisition itself still flows to Rust unchanged");
      assert.deepEqual(cacheEntries(cacheRoot), []);
    });
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("without SOLID_CHECKER_REGISTRY_CACHE every acquisition is fetched fresh", async () => {
  const project = mkdtempSync(join(tmpdir(), "solid-checker-registry-cache-"));
  try {
    const archive = new TextEncoder().encode("alpha archive bytes").buffer;
    const integrity = sriOf(archive);
    writeRootSourceInstall(project, { lockedNames: ["alpha"], integrityOf: () => integrity });
    const served = { alpha: { archive, integrity } };
    await withRegistryCache("", async () => {
      const first = [];
      await acquireAlphaBeta(project, "scratch-1", registryStub(served, first));
      const second = [];
      await acquireAlphaBeta(project, "scratch-2", registryStub(served, second));
      assert.equal(first.length, 2);
      assert.equal(second.length, 2);
    });
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

// ---------------------------------------------------------------------------
// Proposal hand-over. A proposal `contract generate` emitted may stand in for
// the one certification would regenerate only when everything the generation
// was parameterized by matches and the bytes still carry the recorded digests.
// ---------------------------------------------------------------------------

function sha256Of(bytes) {
  return `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
}

function writeReusableProposalProject() {
  const project = mkdtempSync(join(tmpdir(), "solid-checker-proposal-reuse-"));
  const write = (path, body) => {
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, body);
  };
  const packageRoot = join(project, "node_modules/root-package");
  write(
    join(packageRoot, "package.json"),
    `{"name":"root-package","version":"1.0.0","exports":{".":{"types":"./types/index.d.ts","import":"./dist/index.js"}}}\n`
  );
  write(join(packageRoot, "types/index.d.ts"), "export declare const value: () => void;\n");
  write(join(packageRoot, "dist/index.js"), "export const value = () => {};\n");
  const importer = certificationImporterPathFor({
    packageRoot,
    catalog: join(project, "out/root.json.accepted-catalog")
  });
  const documentBytes = Buffer.from('{"format":"stable","package":{"name":"root-package"}}\n');
  const planBytes = Buffer.from('{"format":"plan"}\n');
  const inputs = {
    format: "solid-checker-contract-certification-inputs",
    inputsVersion: 1,
    package: { name: "root-package", version: "1.0.0" },
    integrity: "sha512-root",
    packageRoot,
    certificationImporter: importer,
    entrypoints: [],
    conditions: [],
    document: { path: join(project, "out/root.json"), sha256: sha256Of(documentBytes) },
    plan: { path: join(project, "out/root.json.proposal.json"), sha256: sha256Of(planBytes) },
    certificationInputs: [
      {
        entrypoint: ".",
        conditions: [],
        resolution: { specifier: "root-package", importer, packageRoot }
      }
    ]
  };
  const manifest = JSON.parse(readFileSync(join(packageRoot, "package.json"), "utf8"));
  const current = {
    manifest,
    packageRoot,
    integrity: "sha512-root",
    certificationImporter: importer,
    entrypoints: [],
    conditions: []
  };
  return { project, packageRoot, importer, documentBytes, planBytes, inputs, current };
}

test("certificationImporterPathFor is deterministic in the package root and catalog", () => {
  const project = mkdtempSync(join(tmpdir(), "solid-checker-importer-path-"));
  try {
    const packageRoot = join(project, "node_modules/pkg");
    mkdirSync(packageRoot, { recursive: true });
    const first = certificationImporterPathFor({ packageRoot, catalog: join(project, "a.catalog") });
    const again = certificationImporterPathFor({ packageRoot, catalog: join(project, "a.catalog") });
    const other = certificationImporterPathFor({ packageRoot, catalog: join(project, "b.catalog") });
    assert.equal(first, again);
    assert.notEqual(first, other, "the catalog is part of the importer identity");
    assert.equal(dirname(first), realpathSync(dirname(packageRoot)), "the importer sits beside the package root");
    assert.match(first.split("/").pop(), /^\.solid-checker-certification-[0-9a-f]{64}\.mjs$/);
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("a matching emitted proposal is admitted for reuse", () => {
  const { project, documentBytes, planBytes, inputs, current } = writeReusableProposalProject();
  try {
    const admitted = reusableProposalInputs({ inputs, documentBytes, planBytes, ...current });
    assert.ok(admitted, "identity, digests and census all match");
    assert.equal(admitted.certificationInputs.length, 1);
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("an emitted proposal is refused for reuse on any parameter or byte mismatch", () => {
  const { project, packageRoot, importer, documentBytes, planBytes, inputs, current } =
    writeReusableProposalProject();
  try {
    const attempt = (override, name) => {
      const result = reusableProposalInputs({
        inputs: { ...inputs, ...override.inputs },
        documentBytes: override.documentBytes ?? documentBytes,
        planBytes: override.planBytes ?? planBytes,
        ...current,
        ...override.current
      });
      assert.equal(result, null, name);
    };
    attempt({ documentBytes: Buffer.from("edited\n") }, "document bytes drifted");
    attempt({ planBytes: Buffer.from("edited\n") }, "plan bytes drifted");
    attempt({ current: { integrity: "sha512-other" } }, "integrity differs");
    attempt({ inputs: { package: { name: "root-package", version: "1.0.1" } } }, "version differs");
    attempt({ current: { entrypoints: ["./extra"] } }, "entrypoint census differs");
    attempt({ current: { conditions: ["solid"] } }, "conditions differ");
    attempt(
      { current: { certificationImporter: join(dirname(importer), ".solid-checker-certification-ffff.mjs") } },
      "importer differs"
    );
    attempt({ inputs: { packageRoot: join(dirname(packageRoot), "other-package") } }, "package root differs");
    attempt({ inputs: { inputsVersion: 2 } }, "unknown sidecar version");
    attempt({ inputs: { certificationInputs: [] } }, "no emitted case");
    attempt(
      {
        inputs: {
          certificationInputs: [
            { entrypoint: "./absent", conditions: [], resolution: { specifier: "root-package/absent", importer, packageRoot } }
          ]
        }
      },
      "a case the current census does not emit"
    );
    attempt(
      {
        inputs: {
          certificationInputs: [
            { entrypoint: ".", conditions: [], resolution: { specifier: "root-package", importer: join(project, "elsewhere.mjs"), packageRoot } }
          ]
        }
      },
      "a resolution from another importer"
    );
    attempt(
      {
        inputs: {
          certificationInputs: [
            { entrypoint: ".", conditions: [], resolution: { specifier: "root-package", importer, packageRoot: packageRoot.replace("/node_modules/", "/node_modules/./") } }
          ]
        }
      },
      "a resolution computed under another spelling of the package root"
    );
    // The census claims nothing here, so any declared claim is invented: a
    // proposal must not be reusable while it asserts an omission the current
    // census never made.
    attempt(
      {
        inputs: {
          inapplicableCases: [
            {
              entrypoint: "./types/index.d.ts",
              conditions: [],
              class: "non-emitting-module-target",
              reason: "runtime target emits no JavaScript"
            }
          ]
        }
      },
      "a declared applicability claim the current census does not make"
    );
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("a reused proposal must declare exactly the applicability claims the census makes", () => {
  const project = mkdtempSync(join(tmpdir(), "solid-checker-proposal-reuse-claims-"));
  const write = (path, body) => {
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, body);
  };
  try {
    const packageRoot = join(project, "node_modules/root-package");
    write(
      join(packageRoot, "package.json"),
      `{"name":"root-package","version":"1.0.0","exports":{".":"./dist/index.js","./types/kinds.d.ts":"./types/kinds.d.ts"}}\n`
    );
    write(join(packageRoot, "dist/index.js"), "export const value = () => {};\n");
    write(join(packageRoot, "types/kinds.d.ts"), "export type Kind = 1;\n");
    const importer = certificationImporterPathFor({
      packageRoot,
      catalog: join(project, "out/root.json.accepted-catalog")
    });
    const documentBytes = Buffer.from('{"format":"stable","package":{"name":"root-package"}}\n');
    const planBytes = Buffer.from('{"format":"plan"}\n');
    const claim = {
      entrypoint: "./types/kinds.d.ts",
      conditions: [],
      class: "non-emitting-module-target",
      reason:
        "runtime target emits no JavaScript (erasable-statements): 1 module-level statement(s)"
    };
    const inputs = {
      format: "solid-checker-contract-certification-inputs",
      inputsVersion: 1,
      package: { name: "root-package", version: "1.0.0" },
      integrity: "sha512-root",
      packageRoot,
      certificationImporter: importer,
      entrypoints: [],
      conditions: [],
      document: { path: join(project, "out/root.json"), sha256: sha256Of(documentBytes) },
      plan: { path: join(project, "out/root.json.proposal.json"), sha256: sha256Of(planBytes) },
      certificationInputs: [
        {
          entrypoint: ".",
          conditions: [],
          resolution: { specifier: "root-package", importer, packageRoot }
        }
      ],
      inapplicableCases: [claim]
    };
    const manifest = JSON.parse(readFileSync(join(packageRoot, "package.json"), "utf8"));
    const current = {
      manifest,
      packageRoot,
      integrity: "sha512-root",
      certificationImporter: importer,
      entrypoints: [],
      conditions: []
    };
    const attempt = override =>
      reusableProposalInputs({
        inputs: { ...inputs, ...override },
        documentBytes,
        planBytes,
        ...current
      });

    const admitted = attempt({});
    assert.ok(admitted, "the declared claim matches the recomputed census");
    assert.deepEqual(admitted.inapplicableCases, [claim]);

    // A sidecar written before the field existed, or one that dropped the
    // claim, would let the omitted case reach certification unproved.
    assert.equal(attempt({ inapplicableCases: undefined }), null, "no claim census at all");
    assert.equal(attempt({ inapplicableCases: [] }), null, "an emptied claim census");
    assert.equal(
      attempt({ inapplicableCases: [{ ...claim, entrypoint: "." }] }),
      null,
      "a claim over another case"
    );
    assert.equal(
      attempt({ inapplicableCases: [claim, claim] }),
      null,
      "a duplicated claim"
    );
  } finally {
    rmSync(project, { recursive: true, force: true });
  }
});

test("only a content-premise disposition travels to certification as a claim", () => {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-claims-"));
  mkdirSync(join(root, "types"), { recursive: true });
  writeFileSync(join(root, "index.js"), "export const value = 1;\n");
  writeFileSync(join(root, "effects.js"), 'import { a } from "./index.js";\na();\n');
  writeFileSync(join(root, "empty.js"), "");
  writeFileSync(join(root, "tokens.json"), "{}\n");
  writeFileSync(join(root, "types/kinds.ts"), "export type Kind = 1;\n");
  writeFileSync(join(root, "types/kinds.d.ts"), "export type Kind = 1;\n");
  writeFileSync(
    join(root, "types/ambient.d.ts"),
    "export declare function createRenderer(): void;\n"
  );
  // The same barrel bytes under both suffixes, and a `.d.ts` carrying an
  // implementation that its suffix cannot vouch for.
  writeFileSync(join(root, "types/barrel.ts"), 'export * from "./kinds.js";\n');
  writeFileSync(join(root, "types/barrel.d.ts"), 'export * from "./kinds.js";\n');
  writeFileSync(join(root, "types/implemented.d.ts"), "declare const value = 1;\n");
  const manifest = {
    exports: {
      ".": "./index.js",
      "./effects": "./effects.js",
      "./empty": "./empty.js",
      "./tokens.json": "./tokens.json",
      "./types/kinds.ts": "./types/kinds.ts",
      "./types/kinds.d.ts": "./types/kinds.d.ts",
      "./types/ambient.d.ts": "./types/ambient.d.ts",
      "./types/barrel.ts": "./types/barrel.ts",
      "./types/barrel.d.ts": "./types/barrel.d.ts",
      "./types/implemented.d.ts": "./types/implemented.d.ts",
      "./private": { "vendor/source": "./src/private.ts", default: "./index.js" }
    }
  };
  const disposition = (entrypoint, conditions = []) =>
    artifactCaseDisposition({ manifest, packageRoot: root, entrypoint, conditions });
  try {
    // A type-only module and an ambient declaration file both carry the
    // applicability claim certification must prove, and the recorded reason
    // names which premise answered — the member's suffix chooses it.
    for (const [entrypoint, arm] of [
      ["./types/kinds.ts", "erasable-statements"],
      ["./types/kinds.d.ts", "declaration-file"],
      ["./types/ambient.d.ts", "declaration-file"]
    ]) {
      assert.deepEqual(
        disposition(entrypoint),
        {
          class: ARTIFACT_DISPOSITION.NonEmittingModuleTarget,
          applicability: ARTIFACT_APPLICABILITY.TypeOnlyExport,
          reason: `runtime target emits no JavaScript (${arm}): 1 module-level statement(s)`
        },
        entrypoint
      );
    }
    // The premise that admits the suffix is strictly narrower on the shapes
    // that make a `.d.ts` claim false, and strictly wider on the re-export
    // forms a declaration file also erases.
    assert.equal(disposition("./types/barrel.ts"), null);
    assert.deepEqual(disposition("./types/barrel.d.ts"), {
      class: ARTIFACT_DISPOSITION.NonEmittingModuleTarget,
      applicability: ARTIFACT_APPLICABILITY.TypeOnlyExport,
      reason: "runtime target emits no JavaScript (declaration-file): 1 module-level statement(s)"
    });
    assert.equal(disposition("./types/implemented.d.ts"), null);
    // A real module, a side-effect-only module, and a member with no
    // statements at all all keep certify-or-refuse.
    assert.equal(disposition("."), null);
    assert.equal(disposition("./effects"), null);
    assert.equal(disposition("./empty"), null);

    // The export-map dispositions are unchanged and carry no claim: Rust
    // replays the export map and the member list for every case anyway.
    assert.deepEqual(disposition("./tokens.json"), {
      class: ARTIFACT_DISPOSITION.NonModuleTarget,
      reason: 'runtime target extension ".json" is not an executable module'
    });
    const rows = [
      { entrypoint: "./types/kinds.ts", conditions: [], ...disposition("./types/kinds.ts") },
      { entrypoint: "./tokens.json", conditions: [], ...disposition("./tokens.json") },
      {
        entrypoint: "./private",
        conditions: ["vendor/source"],
        ...disposition("./private", ["vendor/source"])
      }
    ];
    assert.deepEqual(declaredApplicabilityClaims(rows), [
      {
        entrypoint: "./types/kinds.ts",
        conditions: [],
        class: ARTIFACT_DISPOSITION.NonEmittingModuleTarget,
        reason:
          "runtime target emits no JavaScript (erasable-statements): 1 module-level statement(s)"
      }
    ]);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
