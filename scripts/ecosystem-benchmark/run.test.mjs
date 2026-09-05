import assert from "node:assert/strict";
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "vitest";

import { buildInstallArguments } from "./lib/install.mjs";
import {
  hasUsableDenominator,
  isCompleteCoverage,
  isMeasuredCoverage,
  readCertifiedCoverage
} from "./lib/certified-coverage.mjs";
import { formatCoverage } from "./lib/report.mjs";
import {
  certificationLaneOf,
  certificationLaneRequest,
  checkRequiredBinaries,
  countDeclaredEntrypoints,
  decideExitCode,
  defaultReportPaths,
  probeOutcome,
  recommendedCertificationInnerConcurrency,
  recommendedCertificationConcurrency,
  recommendedConcurrency,
  resolveProbeIdFilter,
  resolveRegistryCache,
  solidRuntimeCompletion,
  unknownExplicitProbeIds,
  runBenchmark,
  runScope,
  startProgressHeartbeat
} from "./run.mjs";

test("a Solid 2 probe pinned to solid-js alone is completed with the same-version @solidjs/web", () => {
  const solidReleases = {
    "solid-js": { v1: ["1.9.14"], v2: ["2.0.0-rc.0", "2.0.0-rc.3"] },
    "@solidjs/web": { v1: [], v2: ["2.0.0-rc.0", "2.0.0-rc.3"] }
  };
  const row = { solidTarget: "solid2" };
  // `@tanstack/solid-query@6.0.0-rc.0`: peers `solid-js` only, imports `@solidjs/web`.
  assert.deepEqual(
    solidRuntimeCompletion(row, { solid: { "solid-js": "2.0.0-rc.3" } }, solidReleases),
    { "@solidjs/web": "2.0.0-rc.3" }
  );
  // Already pinned: nothing to add.
  assert.deepEqual(
    solidRuntimeCompletion(
      row,
      { solid: { "solid-js": "2.0.0-rc.3", "@solidjs/web": "2.0.0-rc.3" } },
      solidReleases
    ),
    {}
  );
  // A version the pinned release catalog never saw is not substituted.
  assert.deepEqual(
    solidRuntimeCompletion(row, { solid: { "solid-js": "2.0.0-rc.9" } }, solidReleases),
    {}
  );
  // No catalog, no completion.
  assert.deepEqual(solidRuntimeCompletion(row, { solid: { "solid-js": "2.0.0-rc.3" } }, null), {});
  // Solid 1 ships `solid-js/web` inside `solid-js`; never completed.
  assert.deepEqual(
    solidRuntimeCompletion({ solidTarget: "solid1" }, { solid: { "solid-js": "1.9.14" } }, solidReleases),
    {}
  );
  // A probe with no solid-js pin (`@solidjs/diagnostics`) has nothing to pair with.
  assert.deepEqual(solidRuntimeCompletion(row, { solid: {} }, solidReleases), {});
});

// ---------------------------------------------------------------------------
// Fixtures: a tiny manifest with a handful of probes, enough to exercise
// ordering, filtering, and failure handling without any real npm/network/CLI
// involvement.
// ---------------------------------------------------------------------------

function makeRow({ family = "solid-primitives", status = "official", pkg, version, solidTarget = "solid1", probes }) {
  return {
    family,
    status,
    package: pkg,
    solidTarget,
    version,
    distTags: ["latest"],
    integrity: `sha512-${pkg}-${version}`,
    deprecated: null,
    dependencies: { "solid-js": "^1.0.0" },
    peerDependencies: {},
    optionalDependencies: {},
    compatibleSolidVersions: { "solid-js": ["1.9.14"] },
    unparsedRanges: [],
    probes
  };
}

// Package names are deliberately alphabetical (alpha < bravo < charlie <
// delta) so their insertion order already matches `collectProbeTasks`'s
// deterministic (family, package, solidTarget) sort — the ordering tests
// below rely on manifest order equalling this array's order.
function fourProbeManifest() {
  return {
    schemaVersion: 1,
    rows: [
      makeRow({
        pkg: "@solid-primitives/alpha",
        version: "1.0.0",
        probes: [{ id: "@solid-primitives/alpha@1.0.0|solid1|only", kind: "only", channel: "stable", solid: { "solid-js": "1.9.14" } }]
      }),
      makeRow({
        pkg: "@solid-primitives/bravo",
        version: "1.0.0",
        probes: [{ id: "@solid-primitives/bravo@1.0.0|solid1|only", kind: "only", channel: "stable", solid: { "solid-js": "1.9.14" } }]
      }),
      makeRow({
        pkg: "@solid-primitives/charlie",
        version: "1.0.0",
        probes: [{ id: "@solid-primitives/charlie@1.0.0|solid1|only", kind: "only", channel: "stable", solid: { "solid-js": "1.9.14" } }]
      }),
      makeRow({
        pkg: "@solid-primitives/delta",
        version: "1.0.0",
        probes: [{ id: "@solid-primitives/delta@1.0.0|solid1|only", kind: "only", channel: "stable", solid: { "solid-js": "1.9.14" } }]
      })
    ],
    exclusions: [],
    supplemental: [],
    limitations: []
  };
}

// A hook set where every step succeeds immediately, for tests that only care
// about one specific behavior (ordering, cleanup, filtering) and want the
// rest of the pipeline to be a no-op.
function successHooks({ mkProjectCalls = [], installCalls = [], generateCalls = [], cleanupCalls = [] } = {}) {
  return {
    now: () => 0,
    mkProject: async args => {
      mkProjectCalls.push(args);
      return { projectDir: `/tmp/project-${mkProjectCalls.length}`, outputDir: `/tmp/out-${mkProjectCalls.length}` };
    },
    installPackages: async args => {
      installCalls.push(args);
      return {
        status: 0,
        stdout: "added 2 packages",
        stderr: "",
        timedOut: false,
        installedVersions: Object.fromEntries(Object.entries(args.expected).map(([name, want]) => [name, want.version])),
        integrity: Object.fromEntries(Object.entries(args.expected).map(([name, want]) => [name, want.integrity]))
      };
    },
    generateContract: async args => {
      generateCalls.push(args);
      return {
        status: 0,
        stdout: "generated pkg@1.0.0 contract with 1 entrypoints at /tmp/out.json; review plan /tmp/plan.json (3 checklist items)",
        stderr: "",
        timedOut: false
      };
    },
    cleanup: async args => {
      cleanupCalls.push(args);
    }
  };
}

test("install arguments used by the real hook are Bun-safe and quiet", () => {
  // This pins the exact same contract lib/install.mjs already guarantees,
  // but at the seam run.mjs actually calls through, so a future refactor
  // that stops routing installs through buildInstallArguments would fail
  // here even if install.mjs's own tests still passed in isolation.
  const args = buildInstallArguments({ specs: ["solid-js@1.9.14", "left-pad@1.0.0"] });
  assert.ok(args.includes("--ignore-scripts"), "must include --ignore-scripts");
  assert.ok(args.includes("--no-progress"), "must include --no-progress");
  assert.ok(!args.includes("--no-package-lock"), "must retain Bun's lockfile evidence");
});

test("the contract output path passed to generateContract is never inside a node_modules directory", async () => {
  const generateCalls = [];
  const hooks = successHooks({ generateCalls });
  await runBenchmark({ manifest: fourProbeManifest(), hooks });

  assert.ok(generateCalls.length > 0);
  for (const call of generateCalls) {
    assert.ok(
      !call.outputPath.split(/[\\/]/).includes("node_modules"),
      `output path must not contain a node_modules segment: ${call.outputPath}`
    );
  }
});

test("probe entrypoint scopes reach contract generation", async () => {
  const manifest = fourProbeManifest();
  manifest.rows[0].probes[0].entrypoints = ["./solid"];
  const generateCalls = [];
  await runBenchmark({ manifest, hooks: successHooks({ generateCalls }) });
  assert.deepEqual(generateCalls[0].entrypoints, ["./solid"]);
});

test("the manifest's exact registry integrity reaches contract generation", async () => {
  const manifest = fourProbeManifest();
  const generateCalls = [];
  await runBenchmark({ manifest, hooks: successHooks({ generateCalls }) });
  assert.equal(generateCalls[0].integrity, manifest.rows[0].integrity);
});

test("certification receives the proposal refusal audit from the retained generation project", async () => {
  const manifest = { ...fourProbeManifest(), rows: [fourProbeManifest().rows[0]] };
  const temporary = mkdtempSync(join(tmpdir(), "solid-checker-refusal-forwarding-"));
  const hooks = successHooks();
  let generatedAuditPath = "";
  let certifiedAuditPath = "";
  hooks.mkProject = async () => {
    const projectDir = join(temporary, "project");
    const outputDir = join(temporary, "output");
    mkdirSync(projectDir, { recursive: true });
    mkdirSync(outputDir, { recursive: true });
    return { projectDir, outputDir };
  };
  hooks.generateContract = async ({ outputPath }) => {
    generatedAuditPath = `${outputPath}.refusals.json`;
    writeFileSync(generatedAuditPath, JSON.stringify({
      format: "solid-checker-contract-proposal-refusals",
      refusalVersion: 1,
      package: { name: "@solid-primitives/alpha", version: "1.0.0" },
      refusals: []
    }));
    return {
      status: 0,
      stdout: `generated pkg@1.0.0 contract with 1 entrypoints at ${outputPath}; review plan /tmp/plan.json (3 checklist items)`,
      stderr: "",
      timedOut: false
    };
  };
  hooks.attemptCertification = async ({ proposalRefusalAudit }) => {
    certifiedAuditPath = proposalRefusalAudit;
    assert.equal(existsSync(proposalRefusalAudit), true);
    return { status: 0, stdout: "", stderr: "", timedOut: false };
  };
  try {
    await runBenchmark({
      manifest,
      hooks,
      options: { concurrency: 1, certificationConcurrency: 1, attemptCertification: true }
    });
    assert.equal(certifiedAuditPath, generatedAuditPath);
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
});

test("complete proposals retain an exact policy-2 certification refusal when attempts are enabled", async () => {
  const manifest = { ...fourProbeManifest(), rows: [fourProbeManifest().rows[0]] };
  const temporary = mkdtempSync(join(tmpdir(), "solid-checker-certification-attempt-"));
  const hooks = successHooks();
  hooks.mkProject = async () => {
    const projectDir = join(temporary, "project");
    const outputDir = join(temporary, "output");
    mkdirSync(projectDir, { recursive: true });
    mkdirSync(outputDir, { recursive: true });
    return { projectDir, outputDir };
  };
  hooks.attemptCertification = async ({ auditPath }) => {
    writeFileSync(
      auditPath,
      JSON.stringify({
        authoritative: false,
        replayable: false,
        status: "refused",
        stage: "witness-acquisition",
        refusal: {
          owner: "type-facts",
          demandId: "sha256:missing",
          family: "selected-signature",
          reason: "the automatic type-facts witness adapter is unavailable"
        },
        refusals: [{ demandId: "sha256:missing" }],
        demandPlans: [{
          demands: [
            { family: "package-identity", satisfiedByArtifactSnapshot: true },
            { family: "selected-signature", satisfiedByArtifactSnapshot: false }
          ]
        }],
        stageDurationsMs: { artifactAcquisition: 1, demandPlanning: 2 },
        graphPreparation: {
          rootCases: 18,
          canonicalNodes: 120,
          proposalGenerations: 120,
          graphNodeReferences: 240,
          nativeCertificationTransactions: 1,
          typeFactsCaseSetBatches: 1
        }
      })
    );
    return { status: 1, stdout: "", stderr: "refused", timedOut: false };
  };
  try {
    const [result] = await runBenchmark({
      manifest,
      hooks,
      options: { concurrency: 1, attemptCertification: true }
    });
    assert.deepEqual(result.certificationAttempt, {
      attempted: true,
      status: "refused",
      // The audit carries a `rootCases` graph preparation and no reuse marker,
      // so the lane it records is the published graph. This row's generation
      // was a `success`, which is never routed, hence the reuse *request*.
      lane: "published-graph",
      laneRequested: "reused-proposal",
      // A refused attempt published no catalog.
      coverage: null,
      stage: "witness-acquisition",
      owner: "type-facts",
      demandId: "sha256:missing",
      family: "selected-signature",
      reason: "the automatic type-facts witness adapter is unavailable",
      refusalCount: 1,
      memoryExceeded: false,
      durationMs: 0,
      stageDurationsMs: { artifactAcquisition: 1, demandPlanning: 2 },
      graphPreparation: {
        rootCases: 18,
        canonicalNodes: 120,
        proposalGenerations: 120,
        graphNodeReferences: 240,
        nativeCertificationTransactions: 1,
        typeFactsCaseSetBatches: 1
      },
      demandCountsByFamily: { "package-identity": 1, "selected-signature": 1 },
      artifactSatisfiedDemandsByFamily: { "package-identity": 1 },
      refusalCountsByFamily: {},
      refusalCountsByOwner: {},
      // The audit fixture names no withheld closure candidate.
      withheldClosures: 0,
      withheldClosureReasons: { noRecipe: 0, censusRefused: 0, vetoIncomplete: 0, dependencyWithheld: 0, other: 0 }
    });
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
});

test("a watchdog-killed certification is a machine-queryable resource failure, not only prose", async () => {
  const manifest = { ...fourProbeManifest(), rows: [fourProbeManifest().rows[0]] };
  const temporary = mkdtempSync(join(tmpdir(), "solid-checker-certification-memory-"));
  const hooks = successHooks();
  hooks.mkProject = async () => {
    const projectDir = join(temporary, "project");
    const outputDir = join(temporary, "output");
    mkdirSync(projectDir, { recursive: true });
    mkdirSync(outputDir, { recursive: true });
    return { projectDir, outputDir };
  };
  // Exactly what the real hook resolves when `superviseChildMemory` SIGKILLs
  // the tree: a non-zero status, the marker appended to stderr, and the flag.
  // No audit file is written, because the child never reached its own exit.
  hooks.attemptCertification = async () => ({
    status: null,
    stdout: "",
    stderr:
      "\n[solid-checker-ecosystem-benchmark: probe process tree exceeded the 4096 MiB memory ceiling and was killed]",
    timedOut: false,
    memoryExceeded: true
  });
  try {
    const [result] = await runBenchmark({
      manifest,
      hooks,
      options: { concurrency: 1, attemptCertification: true }
    });
    assert.equal(result.certificationAttempt.status, "infrastructure-failure");
    assert.equal(result.certificationAttempt.memoryExceeded, true);
    assert.match(result.certificationAttempt.reason, /memory ceiling/);
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
});

test("a certification that exits on its own is not reported as a resource failure", async () => {
  const manifest = { ...fourProbeManifest(), rows: [fourProbeManifest().rows[0]] };
  const hooks = successHooks();
  // A hook result that names no flag at all: the row must read `false`, never
  // `undefined`, so "was this a memory kill?" is always answerable.
  hooks.attemptCertification = async () => ({
    status: 0,
    stdout: "",
    stderr: "",
    timedOut: false
  });
  const [result] = await runBenchmark({
    manifest,
    hooks,
    options: { concurrency: 1, attemptCertification: true }
  });
  assert.equal(result.certificationAttempt.status, "certified");
  assert.equal(result.certificationAttempt.memoryExceeded, false);
});

test("the shared pool drains certification during long generation without exceeding either cap", async () => {
  const manifest = fourProbeManifest();
  const events = [];
  let generatedStarted = 0;
  let generatedFinished = 0;
  let activeWork = 0;
  let maximumActiveWork = 0;
  let activeCertifications = 0;
  let maximumActiveCertifications = 0;
  let releaseLongGeneration;
  const mkProjectCalls = [];
  const installCalls = [];
  const cleanupCalls = [];
  const generatedPackageRoots = [];
  const certifiedPackageRoots = [];
  const longGeneration = new Promise(resolve => {
    releaseLongGeneration = resolve;
  });
  const hooks = successHooks({ mkProjectCalls, installCalls, cleanupCalls });
  hooks.generateContract = async ({ packageRoot }) => {
    generatedPackageRoots.push(packageRoot);
    events.push(`generate:${packageRoot}`);
    generatedStarted += 1;
    activeWork += 1;
    maximumActiveWork = Math.max(maximumActiveWork, activeWork);
    if (packageRoot.includes("alpha")) await longGeneration;
    activeWork -= 1;
    generatedFinished += 1;
    return {
      status: 0,
      stdout: "generated pkg@1.0.0 contract with 1 entrypoints at /tmp/out.json; review plan /tmp/plan.json (3 checklist items)",
      stderr: "",
      timedOut: false
    };
  };
  hooks.attemptCertification = async ({ packageRoot }) => {
    certifiedPackageRoots.push(packageRoot);
    assert.ok(generatedStarted >= 2, "certification follows a completed generation");
    if (generatedFinished < 4) releaseLongGeneration();
    activeWork += 1;
    maximumActiveWork = Math.max(maximumActiveWork, activeWork);
    activeCertifications += 1;
    maximumActiveCertifications = Math.max(
      maximumActiveCertifications,
      activeCertifications
    );
    events.push(`certify:${packageRoot}`);
    await new Promise(resolveDelay => setTimeout(resolveDelay, 5));
    activeCertifications -= 1;
    activeWork -= 1;
    return { status: 0, stdout: "", stderr: "", timedOut: false };
  };

  const results = await runBenchmark({
    manifest,
    hooks,
    options: {
      concurrency: 2,
      certificationConcurrency: 1,
      attemptCertification: true,
      scheduleCosts: { "@solid-primitives/alpha@1.0.0|solid1|only": 100 }
    }
  });

  assert.equal(maximumActiveCertifications, 1);
  assert.ok(maximumActiveWork <= 2, `shared worker pool reached ${maximumActiveWork}`);
  assert.equal(events.filter(event => event.startsWith("generate:")).length, 4);
  assert.equal(events.filter(event => event.startsWith("certify:")).length, 4);
  assert.equal(mkProjectCalls.length, 4, "certification reuses each generation project");
  assert.equal(installCalls.length, 4, "certification reuses each verified install");
  assert.equal(cleanupCalls.length, 4, "each transferred project lease is released once");
  assert.deepEqual(
    [...certifiedPackageRoots].sort(),
    [...generatedPackageRoots].sort(),
    "fresh certification reads the exact verified generation project"
  );
  assert.ok(
    events.findIndex(event => event.startsWith("certify:")) > 0,
    "certification should run after generation work was claimed"
  );
  assert.ok(results.every(result => result.certificationAttempt.status === "certified"));
});

test("dedicated certification slots never displace the generation width", async () => {
  const manifest = fourProbeManifest();
  let activeGenerations = 0;
  let maximumActiveGenerations = 0;
  let activeAtFirstCertification = null;
  let generationStarts = 0;
  let releaseFirstGenerationWave;
  let releaseRefilledGeneration;
  let releaseBlockedGenerations;
  const firstGenerationWave = new Promise(resolve => {
    releaseFirstGenerationWave = resolve;
  });
  const blockedGenerations = new Promise(resolve => {
    releaseBlockedGenerations = resolve;
  });
  const refilledGeneration = new Promise(resolve => {
    releaseRefilledGeneration = resolve;
  });
  const hooks = successHooks();
  hooks.generateContract = async ({ packageRoot }) => {
    generationStarts += 1;
    activeGenerations += 1;
    maximumActiveGenerations = Math.max(maximumActiveGenerations, activeGenerations);
    if (activeGenerations === 2) releaseFirstGenerationWave();
    if (generationStarts === 3) releaseRefilledGeneration();
    await firstGenerationWave;
    if (!packageRoot.includes("alpha")) await blockedGenerations;
    activeGenerations -= 1;
    return {
      status: 0,
      stdout: "generated pkg@1.0.0 contract with 1 entrypoints at /tmp/out.json; review plan /tmp/plan.json (3 checklist items)",
      stderr: "",
      timedOut: false
    };
  };
  hooks.attemptCertification = async () => {
    if (activeAtFirstCertification === null) {
      await refilledGeneration;
      activeAtFirstCertification = activeGenerations;
      releaseBlockedGenerations();
    }
    return { status: 0, stdout: "", stderr: "", timedOut: false };
  };

  const results = await runBenchmark({
    manifest,
    hooks,
    options: {
      concurrency: 2,
      certificationConcurrency: 4,
      attemptCertification: true
    }
  });

  assert.equal(maximumActiveGenerations, 2);
  assert.equal(
    activeAtFirstCertification,
    2,
    "certification must use a dedicated slot after generation is refilled"
  );
  assert.ok(results.every(result => result.certificationAttempt.status === "certified"));
});

test("an unexpected generation result releases its transferred project lease", async () => {
  const manifest = { ...fourProbeManifest(), rows: [fourProbeManifest().rows[0]] };
  const cleanupCalls = [];
  const hooks = successHooks({ cleanupCalls });
  hooks.installPackages = async () => undefined;

  await assert.rejects(
    runBenchmark({ manifest, hooks, options: { concurrency: 1 } }),
    /installedVersions/
  );
  assert.equal(cleanupCalls.length, 1, "a rejected generation must not orphan its project");
});

test("an unexpected certification result still releases the reused project", async () => {
  const manifest = { ...fourProbeManifest(), rows: [fourProbeManifest().rows[0]] };
  const cleanupCalls = [];
  const hooks = successHooks({ cleanupCalls });
  hooks.attemptCertification = async () => undefined;

  await assert.rejects(
    runBenchmark({
      manifest,
      hooks,
      options: {
        concurrency: 1,
        certificationConcurrency: 1,
        attemptCertification: true
      }
    }),
    /status/
  );
  assert.equal(cleanupCalls.length, 1, "a rejected certification must not orphan its project");
});

test("certification expands past the install-safe generation width after proposal work drains", async () => {
  const manifest = fourProbeManifest();
  let activeCertifications = 0;
  let maximumActiveCertifications = 0;
  const hooks = successHooks();
  hooks.attemptCertification = async () => {
    activeCertifications += 1;
    maximumActiveCertifications = Math.max(
      maximumActiveCertifications,
      activeCertifications
    );
    await new Promise(resolveDelay => setTimeout(resolveDelay, 10));
    activeCertifications -= 1;
    return { status: 0, stdout: "", stderr: "", timedOut: false };
  };

  const results = await runBenchmark({
    manifest,
    hooks,
    options: {
      concurrency: 2,
      certificationConcurrency: 4,
      attemptCertification: true
    }
  });

  assert.ok(
    maximumActiveCertifications > 2,
    `certification drain stayed at generation width ${maximumActiveCertifications}`
  );
  assert.ok(maximumActiveCertifications <= 4);
  assert.ok(results.every(result => result.certificationAttempt.status === "certified"));
});

test("a timeout during generation produces a timeout result and the run continues", async () => {
  const manifest = fourProbeManifest();
  const cleanupCalls = [];
  const hooks = successHooks({ cleanupCalls });
  hooks.generateContract = async ({ packageRoot }) => {
    if (packageRoot.includes("bravo")) {
      return { status: null, stdout: "", stderr: "", timedOut: true };
    }
    return {
      status: 0,
      stdout: "generated pkg@1.0.0 contract with 1 entrypoints at /tmp/out.json; review plan /tmp/plan.json (3 checklist items)",
      stderr: "",
      timedOut: false
    };
  };
  // mkProject must report which package this probe is for so the fake
  // generateContract above can single probe #2 out; reuse successHooks'
  // mkProject but tag the projectDir with the package name.
  let counter = 0;
  hooks.mkProject = async ({ row }) => {
    counter += 1;
    return { projectDir: `/tmp/project-${row.package.split("/").pop()}`, outputDir: `/tmp/out-${counter}` };
  };

  const results = await runBenchmark({ manifest, hooks });

  assert.equal(results.length, 4);
  const timedOutProbe = results.find(r => r.package === "@solid-primitives/bravo");
  assert.equal(timedOutProbe.class, "timeout");
  assert.equal(timedOutProbe.timedOut, true);
  assert.equal(timedOutProbe.outcome, "failure");
  // Every other probe still ran and succeeded.
  const others = results.filter(r => r.package !== "@solid-primitives/bravo");
  assert.equal(others.length, 3);
  for (const other of others) assert.equal(other.outcome, "success");
});

test("a generation that refused entrypoints is partial-success, not success", async () => {
  // The generator exits 0 and writes a real contract, so nothing about the
  // process status distinguishes this from a complete run -- only the note it
  // prints does. A contract describing 1 of 3 entrypoints must never be
  // filed under the same outcome as one describing all of them.
  const manifest = { ...fourProbeManifest(), rows: [fourProbeManifest().rows[0]] };
  const temporary = mkdtempSync(join(tmpdir(), "solid-checker-benchmark-run-"));
  const hooks = successHooks();
  hooks.mkProject = async () => ({
    projectDir: join(temporary, "project"),
    outputDir: join(temporary, "out")
  });
  hooks.generateContract = async ({ outputPath }) => {
    mkdirSync(dirname(outputPath), { recursive: true });
    writeFileSync(
      outputPath,
      JSON.stringify({
        schemaVersion: 1,
        summaries: { value: { kind: "value" } },
        entrypoints: { ".": { exports: { value: ["thing"] } } }
      })
    );
    return {
      status: 0,
      stdout:
        `generated pkg@1.0.0 contract with 1 entrypoints at ${outputPath}; ` +
        "2 entrypoint(s) refused and omitted; review plan /tmp/plan.md (7 checklist items)",
      stderr: "",
      timedOut: false
    };
  };

  try {
    const [result] = await runBenchmark({ manifest, hooks, options: { concurrency: 1 } });

    assert.equal(result.class, "partial-success");
    assert.equal(result.outcome, "partial-success");
    assert.equal(result.refusedEntrypoints, 2);
    // The contract it did write is still measured.
    assert.equal(result.generatedEntrypoints, 1);
    assert.equal(result.checklistItems, 7);
    assert.equal(result.exitStatus, 0);
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
});

test("a full generation refusal retains its structured refusal census", async () => {
  const manifest = { ...fourProbeManifest(), rows: [fourProbeManifest().rows[0]] };
  const temporary = mkdtempSync(join(tmpdir(), "solid-checker-benchmark-refusal-audit-"));
  const hooks = successHooks();
  hooks.mkProject = async () => {
    const projectDir = join(temporary, "project");
    const outputDir = join(temporary, "output");
    mkdirSync(projectDir, { recursive: true });
    mkdirSync(outputDir, { recursive: true });
    return { projectDir, outputDir };
  };
  hooks.generateContract = async ({ outputPath }) => {
    writeFileSync(
      `${outputPath}.refusals.json`,
      JSON.stringify({
        format: "solid-checker-contract-proposal-refusals",
        refusalVersion: 1,
        package: { name: "@solid-primitives/alpha", version: "1.0.0" },
        refusals: [
          { entrypoint: ".", conditions: [], stage: "artifact-case", reason: "first" },
          { entrypoint: "./sub", conditions: [], stage: "proposal-merge", reason: "second" }
        ]
      })
    );
    return {
      status: 2,
      stdout: "",
      stderr: "solid-checker: no certifiable artifact case; 2 case(s) refused; first refusal: .: first",
      timedOut: false
    };
  };
  try {
    const [result] = await runBenchmark({ manifest, hooks, options: { concurrency: 1 } });
    assert.equal(result.outcome, "failure");
    assert.equal(result.contractContent, null);
    assert.equal(result.refusedArtifactCases, 2);
    assert.deepEqual(result.artifactCaseRefusals.map(refusal => refusal.stage), [
      "artifact-case",
      "proposal-merge"
    ]);
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
});

test("probeOutcome files a complete contract, a partial one, and a failure separately", () => {
  assert.equal(probeOutcome("success"), "success");
  assert.equal(probeOutcome("partial-success"), "partial-success");
  assert.equal(probeOutcome("cjs-only-entrypoint"), "failure");
  assert.equal(probeOutcome("unclassified"), "failure");
});

test("runBenchmark records install and generation durations separately", async () => {
  const manifest = { ...fourProbeManifest(), rows: [fourProbeManifest().rows[0]] };
  const hooks = successHooks();
  const timestamps = [0, 10, 30, 40, 90, 100];
  hooks.now = () => timestamps.shift();

  const [result] = await runBenchmark({ manifest, hooks, options: { concurrency: 1 } });

  assert.equal(result.installDurationMs, 20);
  assert.equal(result.generationDurationMs, 50);
  assert.equal(result.durationMs, 100);
});

test("probes that throw never abort the run: all results are present and later probes still run", async () => {
  const manifest = fourProbeManifest();
  const hooks = successHooks();
  hooks.installPackages = async ({ expected }) => {
    const pkg = Object.keys(expected).find(name => name.startsWith("@solid-primitives"));
    if (pkg === "@solid-primitives/bravo" || pkg === "@solid-primitives/charlie") {
      throw new Error(`simulated install crash for ${pkg}`);
    }
    return {
      status: 0,
      stdout: "added 2 packages",
      stderr: "",
      timedOut: false,
      installedVersions: Object.fromEntries(Object.entries(expected).map(([name, want]) => [name, want.version])),
      integrity: Object.fromEntries(Object.entries(expected).map(([name, want]) => [name, want.integrity]))
    };
  };

  const results = await runBenchmark({ manifest, hooks, options: { concurrency: 1 } });

  assert.equal(results.length, 4, "all four probes must produce a result");
  const byPackage = Object.fromEntries(results.map(r => [r.package, r]));
  assert.equal(byPackage["@solid-primitives/alpha"].outcome, "success");
  assert.equal(byPackage["@solid-primitives/bravo"].outcome, "failure");
  assert.equal(byPackage["@solid-primitives/charlie"].outcome, "failure");
  // Probe four ran despite two and three throwing.
  assert.equal(byPackage["@solid-primitives/delta"].outcome, "success");
});

test("results are returned in deterministic manifest order even when hooks resolve out of order", async () => {
  const manifest = fourProbeManifest();
  const hooks = successHooks();
  // Stagger generateContract so the LAST probe (by manifest order) finishes
  // FIRST, and the FIRST probe finishes LAST — completion order is the
  // exact inverse of manifest order.
  const delays = { alpha: 40, bravo: 30, charlie: 20, delta: 10 };
  hooks.generateContract = async ({ packageRoot }) => {
    const key = Object.keys(delays).find(name => packageRoot.includes(name));
    await new Promise(resolveDelay => setTimeout(resolveDelay, delays[key]));
    return {
      status: 0,
      stdout: "generated pkg@1.0.0 contract with 1 entrypoints at /tmp/out.json; review plan /tmp/plan.json (3 checklist items)",
      stderr: "",
      timedOut: false
    };
  };

  const results = await runBenchmark({ manifest, hooks, options: { concurrency: 4 } });

  assert.deepEqual(
    results.map(r => r.package),
    ["@solid-primitives/alpha", "@solid-primitives/bravo", "@solid-primitives/charlie", "@solid-primitives/delta"],
    "results must follow manifest order regardless of which hook resolved first"
  );
});

test("historical cost hints schedule long probes first without changing report order", async () => {
  const manifest = fourProbeManifest();
  const installCalls = [];
  const results = await runBenchmark({
    manifest,
    hooks: successHooks({ installCalls }),
    options: {
      concurrency: 1,
      scheduleCosts: {
        "@solid-primitives/alpha@1.0.0|solid1|only": 1,
        "@solid-primitives/bravo@1.0.0|solid1|only": 100,
        "@solid-primitives/charlie@1.0.0|solid1|only": 2,
        "@solid-primitives/delta@1.0.0|solid1|only": 50
      }
    }
  });

  assert.deepEqual(
    installCalls.map(call =>
      Object.keys(call.expected).find(name => name.startsWith("@solid-primitives"))
    ),
    [
      "@solid-primitives/bravo",
      "@solid-primitives/delta",
      "@solid-primitives/charlie",
      "@solid-primitives/alpha"
    ]
  );
  assert.deepEqual(
    results.map(result => result.package),
    [
      "@solid-primitives/alpha",
      "@solid-primitives/bravo",
      "@solid-primitives/charlie",
      "@solid-primitives/delta"
    ]
  );
});

test("cleanup is called once per probe, including for a probe that failed, and is skipped entirely with keepTemp", async () => {
  const manifest = fourProbeManifest();

  const cleanupCallsDefault = [];
  const hooksDefault = successHooks({ cleanupCalls: cleanupCallsDefault });
  hooksDefault.installPackages = async ({ expected }) => {
    const pkg = Object.keys(expected).find(name => name.startsWith("@solid-primitives"));
    if (pkg === "@solid-primitives/bravo") {
      return { status: 1, stdout: "", stderr: "npm ERR! ETARGET", timedOut: false, installedVersions: {}, integrity: {} };
    }
    return {
      status: 0,
      stdout: "added 2 packages",
      stderr: "",
      timedOut: false,
      installedVersions: Object.fromEntries(Object.entries(expected).map(([name, want]) => [name, want.version])),
      integrity: Object.fromEntries(Object.entries(expected).map(([name, want]) => [name, want.integrity]))
    };
  };

  await runBenchmark({ manifest, hooks: hooksDefault });
  assert.equal(cleanupCallsDefault.length, 4, "cleanup must run once per probe regardless of outcome");

  const cleanupCallsKeepTemp = [];
  const hooksKeepTemp = successHooks({ cleanupCalls: cleanupCallsKeepTemp });
  await runBenchmark({ manifest, hooks: hooksKeepTemp, options: { keepTemp: true } });
  assert.equal(cleanupCallsKeepTemp.length, 0, "cleanup must never run when keepTemp is set");
});

test("a version mismatch on one probe yields an install-failure/integrity-failure result without runBenchmark rejecting", async () => {
  const manifest = fourProbeManifest();
  const hooks = successHooks();
  hooks.installPackages = async ({ expected }) => {
    const pkg = Object.keys(expected).find(name => name.startsWith("@solid-primitives"));
    if (pkg === "@solid-primitives/charlie") {
      // Reports success exit status, but the installed version does not
      // match what the manifest pinned — the install step itself was
      // "successful" from npm's point of view, only the verification fails.
      return {
        status: 0,
        stdout: "added 2 packages",
        stderr: "",
        timedOut: false,
        installedVersions: { [pkg]: "0.9.0", "solid-js": "1.9.14" },
        integrity: { [pkg]: expected[pkg].integrity, "solid-js": null }
      };
    }
    return {
      status: 0,
      stdout: "added 2 packages",
      stderr: "",
      timedOut: false,
      installedVersions: Object.fromEntries(Object.entries(expected).map(([name, want]) => [name, want.version])),
      integrity: Object.fromEntries(Object.entries(expected).map(([name, want]) => [name, want.integrity]))
    };
  };

  const results = await runBenchmark({ manifest, hooks });
  assert.equal(results.length, 4);
  const mismatched = results.find(r => r.package === "@solid-primitives/charlie");
  assert.equal(mismatched.class, "install-failure");
  assert.equal(mismatched.outcome, "failure");
  assert.equal(mismatched.integrityVerified, false);
  // The rest of the run still completed.
  assert.equal(results.filter(r => r.outcome === "success").length, 3);
});

test("an integrity mismatch classifies distinctly as integrity-failure", async () => {
  const manifest = { ...fourProbeManifest(), rows: [fourProbeManifest().rows[0]] };
  const hooks = successHooks();
  hooks.installPackages = async ({ expected }) => {
    const pkg = Object.keys(expected).find(name => name.startsWith("@solid-primitives"));
    return {
      status: 0,
      stdout: "added 2 packages",
      stderr: "",
      timedOut: false,
      installedVersions: { [pkg]: expected[pkg].version, "solid-js": "1.9.14" },
      integrity: { [pkg]: "sha512-tampered==", "solid-js": null }
    };
  };

  const results = await runBenchmark({ manifest, hooks });
  assert.equal(results.length, 1);
  assert.equal(results[0].class, "integrity-failure");
  assert.equal(results[0].outcome, "failure");
});

test("resolveProbeIdFilter returns null (meaning: run everything) when no filter is requested", () => {
  const manifest = fourProbeManifest();
  assert.equal(resolveProbeIdFilter({ manifest }), null);
});

test("recommendedConcurrency bounds Bun install and outer proposal contention", () => {
  assert.equal(recommendedConcurrency(1), 1);
  assert.equal(recommendedConcurrency(4), 4);
  assert.equal(recommendedConcurrency(8), 8);
  assert.equal(recommendedConcurrency(12), 8);
  assert.equal(recommendedConcurrency(14), 8);
  assert.equal(recommendedConcurrency(32), 8);
  assert.equal(recommendedConcurrency(Number.NaN), 4);
});

test("recommendedCertificationConcurrency fills the bounded drain pool within memory", () => {
  const gib = 1024 * 1024 * 1024;
  const plenty = 1024 * gib;
  // The drain runs six slots wider than the core count, capped at twenty: a
  // certification child mostly waits on filesystem metadata once registry
  // bytes are cached, so cores-bounded width left the host under-used
  // (measured 185-190 s at 14 slots against 176-178 s at 20 on the 14-core
  // authority host, identical outcomes; 24 was no faster than 20).
  assert.equal(recommendedCertificationConcurrency(1, plenty), 7);
  assert.equal(recommendedCertificationConcurrency(8, plenty), 14);
  assert.equal(recommendedCertificationConcurrency(12, plenty), 18);
  assert.equal(recommendedCertificationConcurrency(14, plenty), 20);
  assert.equal(recommendedCertificationConcurrency(32, plenty), 20);
  assert.equal(recommendedCertificationConcurrency(Number.NaN, plenty), 2);
  // The drain width reserves one memory share per slot. The share is 2 GiB,
  // ~2.7x the worst process-tree peak measured across the heavy tail after the
  // resolver stopped retaining one `ts.Program` per module (762 MiB, down from
  // 30.5 GB for the worst probe), so a 48 GB host now runs the full
  // cores-bounded width instead of the six slots an 8 GiB share allowed.
  assert.equal(recommendedCertificationConcurrency(14, 48 * gib), 20);
  assert.equal(recommendedCertificationConcurrency(14, 16 * gib), 8);
  // The share still bounds a small machine below its core count, and the floor
  // keeps two slots on a host too small for even one share.
  assert.equal(recommendedCertificationConcurrency(14, 8 * gib), 4);
  assert.equal(recommendedCertificationConcurrency(14, 1 * gib), 2);
  // Memory never lifts the width above the oversubscribed core bound, and an
  // unknown size stays at the conservative floor rather than the cores-only
  // width.
  assert.equal(recommendedCertificationConcurrency(4, plenty), 10);
  assert.equal(recommendedCertificationConcurrency(14, Number.NaN), 2);
});

test("recommendedCertificationInnerConcurrency preserves a host-wide native bound", () => {
  assert.equal(recommendedCertificationInnerConcurrency(1, 14), 8);
  assert.equal(recommendedCertificationInnerConcurrency(2, 14), 7);
  assert.equal(recommendedCertificationInnerConcurrency(6, 14), 2);
  assert.equal(recommendedCertificationInnerConcurrency(12, 14), 1);
  assert.equal(recommendedCertificationInnerConcurrency(32, 14), 1);
  assert.equal(recommendedCertificationInnerConcurrency(0, 14), 1);
  assert.equal(recommendedCertificationInnerConcurrency(12, Number.NaN), 1);
});

test("the CLI progress heartbeat bounds silent runs without changing benchmark results", () => {
  const lines = [];
  let scheduled = null;
  let cleared = null;
  const timer = Symbol("progress timer");
  const stop = startProgressHeartbeat({
    intervalMs: 30_000,
    writeLine: line => lines.push(line),
    schedule: (callback, delay) => {
      scheduled = { callback, delay };
      return timer;
    },
    cancel: value => {
      cleared = value;
    }
  });

  assert.equal(scheduled.delay, 30_000);
  scheduled.callback();
  scheduled.callback();
  assert.deepEqual(lines, [
    "solid-checker-ecosystem-benchmark: still running (30s heartbeat; reports follow all probes)",
    "solid-checker-ecosystem-benchmark: still running (60s heartbeat; reports follow all probes)"
  ]);

  stop();
  assert.equal(cleared, timer);
});

test("resolveProbeIdFilter narrows to a sentinel subset intersected with family/solid filters", () => {
  const manifest = fourProbeManifest();
  const ids = resolveProbeIdFilter({
    manifest,
    sentinelIds: ["@solid-primitives/alpha@1.0.0|solid1|only", "@solid-primitives/bravo@1.0.0|solid1|only"]
  });
  assert.deepEqual(ids.sort(), ["@solid-primitives/alpha@1.0.0|solid1|only", "@solid-primitives/bravo@1.0.0|solid1|only"]);
});

test("resolveProbeIdFilter accepts exact repeatable probe ids", () => {
  const manifest = fourProbeManifest();
  const ids = resolveProbeIdFilter({
    manifest,
    explicitProbeIds: ["@solid-primitives/bravo@1.0.0|solid1|only"]
  });
  assert.deepEqual(ids, ["@solid-primitives/bravo@1.0.0|solid1|only"]);
});

test("unknownExplicitProbeIds refuses a misspelled exact probe instead of permitting an empty measurement", () => {
  const manifest = fourProbeManifest();
  assert.deepEqual(
    unknownExplicitProbeIds(manifest, [
      "@solid-primitives/bravo@1.0.0|solid1|only",
      "@solid-primitives/brav0@1.0.0|solid1|only",
      "@solid-primitives/brav0@1.0.0|solid1|only"
    ]),
    ["@solid-primitives/brav0@1.0.0|solid1|only"]
  );
});

test("runBenchmark honors an explicit probeIds filter", async () => {
  const manifest = fourProbeManifest();
  const hooks = successHooks();
  const results = await runBenchmark({
    manifest,
    probeIds: ["@solid-primitives/bravo@1.0.0|solid1|only"],
    hooks
  });
  assert.equal(results.length, 1);
  assert.equal(results[0].package, "@solid-primitives/bravo");
});

// ---------------------------------------------------------------------------
// Benchmark-vs-threshold exit behavior: tested directly against the exit
// code decision function rather than by spawning the process.
// ---------------------------------------------------------------------------

test("decideExitCode: with failures and no --thresholds, the run reports infrastructure success (exit 0)", () => {
  assert.equal(decideExitCode({ thresholdsRequested: false, evaluation: null }), 0);
});

test("decideExitCode: with --thresholds and evaluateThresholds reporting ok, exit 0", () => {
  assert.equal(decideExitCode({ thresholdsRequested: true, evaluation: { ok: true, failures: [] } }), 0);
});

test("decideExitCode: with --thresholds and evaluateThresholds reporting a regression, exit 1", () => {
  assert.equal(
    decideExitCode({ thresholdsRequested: true, evaluation: { ok: false, failures: [{ metric: "solid1.successRate" }] } }),
    1
  );
});

// ---------------------------------------------------------------------------
// Missing binaries: the exact missing path must be named.
// ---------------------------------------------------------------------------

test("checkRequiredBinaries reports the exact missing native binary path", () => {
  const result = checkRequiredBinaries({
    SOLID_CHECKER_NATIVE_BIN: "/nonexistent/solid-checker-rust",
    SOLID_TYPEFACTS_BIN: "/nonexistent/solid-typefacts"
  });
  assert.equal(result.ok, false);
  assert.ok(result.problems.some(problem => problem.includes("/nonexistent/solid-checker-rust")));
  assert.ok(result.problems.some(problem => problem.includes("/nonexistent/solid-typefacts")));
});

test("checkRequiredBinaries reports both env vars missing when unset", () => {
  const result = checkRequiredBinaries({});
  assert.equal(result.ok, false);
  assert.ok(result.problems.some(problem => problem.includes("SOLID_CHECKER_NATIVE_BIN is not set")));
  assert.ok(result.problems.some(problem => problem.includes("SOLID_TYPEFACTS_BIN is not set")));
});

test("resolveRegistryCache prefers the flag, then the environment, then the repository default", () => {
  const fallback = "/repo/rust/target/registry-cache";
  assert.equal(resolveRegistryCache({ env: {}, fallback }), fallback);
  assert.equal(
    resolveRegistryCache({ env: { SOLID_CHECKER_REGISTRY_CACHE: "" }, fallback }),
    fallback,
    "an empty variable is not a cache location"
  );
  assert.equal(
    resolveRegistryCache({ env: { SOLID_CHECKER_REGISTRY_CACHE: "/elsewhere/cache" }, fallback }),
    "/elsewhere/cache"
  );
  assert.equal(
    resolveRegistryCache({
      option: "/flag/cache",
      env: { SOLID_CHECKER_REGISTRY_CACHE: "/elsewhere/cache" },
      fallback
    }),
    "/flag/cache"
  );
  assert.equal(
    resolveRegistryCache({
      option: "/flag/cache",
      disabled: true,
      env: { SOLID_CHECKER_REGISTRY_CACHE: "/elsewhere/cache" },
      fallback
    }),
    null,
    "--no-registry-cache wins over every source"
  );
});

test("checkRequiredBinaries reports ok when both paths exist", () => {
  // This file itself is a real, existing path, so it stands in for both
  // binaries without needing an actual compiled checker in this test.
  const result = checkRequiredBinaries({
    SOLID_CHECKER_NATIVE_BIN: import.meta.url.replace("file://", ""),
    SOLID_TYPEFACTS_BIN: import.meta.url.replace("file://", "")
  });
  assert.equal(result.ok, true);
  assert.deepEqual(result.problems, []);
});

// ---------------------------------------------------------------------------
// Small pure-helper coverage.
// ---------------------------------------------------------------------------

test("countDeclaredEntrypoints counts subpaths, treats a single string/conditions object as one, and wildcards as one", () => {
  // The count is one *pattern* per key, and `wildcard` records that the count
  // is a pattern count -- the only place that is visible.
  const count = exportsField => countDeclaredEntrypoints(exportsField).count;
  assert.equal(count("./index.js"), 1);
  assert.equal(count({ import: "./index.mjs", require: "./index.cjs" }), 1);
  assert.equal(count({ ".": "./index.js", "./util": "./util.js" }), 2);
  assert.equal(count({ ".": "./index.js", "./*": "./dist/*.js" }), 2);
  assert.equal(count(null), 0);
  assert.equal(count(undefined), 0);
  assert.equal(countDeclaredEntrypoints({ ".": "./index.js", "./*": "./dist/*.js" }).wildcard, true);
  assert.equal(countDeclaredEntrypoints({ ".": "./index.js" }).wildcard, false);
  assert.equal(countDeclaredEntrypoints("./index.js").wildcard, false);
  assert.equal(countDeclaredEntrypoints(null).wildcard, false);
});

test("only an unfiltered run defaults to the canonical report path", () => {
  // A subset overwriting benchmarks/ecosystem/report.json is how a full-corpus
  // artifact gets silently replaced by a 23-probe one. The scope owns the name.
  const full = defaultReportPaths(runScope({}), "/reports");
  assert.equal(full.json, "/reports/report.json");
  assert.equal(full.markdown, "/reports/report.md");

  const sentinel = defaultReportPaths(runScope({ sentinel: true }), "/reports");
  assert.equal(sentinel.json, "/reports/report-sentinel.json");
  assert.equal(sentinel.markdown, "/reports/report-sentinel.md");

  const family = defaultReportPaths(
    runScope({ families: ["kobalte"], solidTargets: ["1"] }),
    "/reports"
  );
  assert.equal(family.json, "/reports/report-family-kobalte-solid1.json");
});

test("a scope slug is order-independent so the same filters always name one file", () => {
  assert.equal(
    runScope({ families: ["tanstack", "corvu"], solidTargets: ["2", "1"] }).slug,
    runScope({ families: ["corvu", "tanstack"], solidTargets: ["1", "2"] }).slug
  );
  assert.equal(
    runScope({ probeIds: ["probe-b", "probe-a"] }).slug,
    runScope({ probeIds: ["probe-a", "probe-b"] }).slug
  );
});

test("runScope records which filters produced a run", () => {
  const scope = runScope({ sentinel: true, families: ["kobalte"] });
  assert.equal(scope.kind, "filtered");
  assert.equal(scope.sentinel, true);
  assert.deepEqual(scope.families, ["kobalte"]);
  assert.equal(runScope({}).kind, "full");
});

// ---------------------------------------------------------------------------
// Lane routing: which proposal lane a probe asks certification for, and which
// one the audit says produced the proposal.
// ---------------------------------------------------------------------------

const DEPENDENCY_BINDING_REFUSAL = {
  entrypoint: ".",
  conditions: [],
  stage: "artifact-case",
  applicability: "runtime-module",
  reason:
    "accepted dependency @solidjs/signals has no exact runtime binding for export $PROXY"
};

const UNRESOLVED_DEPENDENCY_MODULE_REFUSAL = {
  entrypoint: ".",
  conditions: [],
  stage: "artifact-case",
  applicability: "runtime-module",
  reason:
    "solid-checker:unresolved-dependency-module=@tanstack/pacer\n" +
    "solid-checker-rust: emit package contract: cannot statically expand external " +
    'export-all "@tanstack/pacer" from <package-root>/dist/index.js'
};

// A refusal about the publisher's own bytes. No dependency catalog moves it, so
// routing it to the graph lane could only lose the cases that did generate.
const PUBLISHER_DEFECT_REFUSAL = {
  entrypoint: "./jsx-runtime",
  conditions: [],
  stage: "artifact-case",
  applicability: "runtime-module",
  reason:
    "solid-checker-rust: emit package contract: entry file <package-root>/dist/solid.js " +
    "has no runtime ESM exports"
};

test("a complete proposal is never routed away from reuse", () => {
  assert.deepEqual(certificationLaneRequest({ class: "success" }), {
    lane: "reused-proposal"
  });
  // Even carrying a dependency-binding refusal row, which a `success` cannot,
  // the class alone decides: a complete proposal describes every applicable
  // case and there is no frontier to want.
  assert.deepEqual(
    certificationLaneRequest({
      class: "success",
      artifactCaseRefusals: [DEPENDENCY_BINDING_REFUSAL]
    }),
    { lane: "reused-proposal" }
  );
});

test("a partial proposal with a dependency-binding refusal is routed to the published graph", () => {
  for (const refusal of [
    DEPENDENCY_BINDING_REFUSAL,
    UNRESOLVED_DEPENDENCY_MODULE_REFUSAL
  ]) {
    assert.deepEqual(
      certificationLaneRequest({
        class: "partial-success",
        artifactCaseRefusals: [PUBLISHER_DEFECT_REFUSAL, refusal]
      }),
      { lane: "published-graph" },
      refusal.reason
    );
  }
});

test("a partial proposal whose refusals are publisher defects keeps its reuse", () => {
  assert.deepEqual(
    certificationLaneRequest({
      class: "partial-success",
      artifactCaseRefusals: [PUBLISHER_DEFECT_REFUSAL, PUBLISHER_DEFECT_REFUSAL]
    }),
    { lane: "reused-proposal" }
  );
  // An absent or empty census is not a dependency frontier either: routing on
  // "not known to be a publisher defect" would send rows to a lane with nothing
  // to prepare.
  for (const refusals of [null, undefined, []]) {
    assert.deepEqual(
      certificationLaneRequest({ class: "partial-success", artifactCaseRefusals: refusals }),
      { lane: "reused-proposal" }
    );
  }
});

test("the lane a row reports is the one the audit recorded, not the one requested", () => {
  assert.equal(certificationLaneOf({ graphPreparation: { reusedProposal: true } }), "reused-proposal");
  assert.equal(
    certificationLaneOf({ graphPreparation: { rootCases: 3, canonicalNodes: 6 } }),
    "published-graph"
  );
  // A graph preparation that also carries the partial-frontier marker is still
  // the graph lane: the marker says how it was reached, not what it is.
  assert.equal(
    certificationLaneOf({
      graphPreparation: { rootCases: 7, partialProposalFrontier: true }
    }),
    "published-graph"
  );
  // Neither marker: certification generated the proposal in its own scratch.
  assert.equal(certificationLaneOf({ graphPreparation: null }), "generated-proposal");
  assert.equal(certificationLaneOf({}), "generated-proposal");
  // A requested graph lane that could not be prepared is exactly that case: no
  // graph produced the proposal. The trace says the lane was attempted, and
  // the lane still reports what actually happened rather than the request.
  assert.equal(
    certificationLaneOf({
      graphPreparation: {
        partialProposalFrontier: "unprepared",
        reason: "registry acquisition failed"
      }
    }),
    "generated-proposal"
  );
  // No audit at all -- an infrastructure failure before certify wrote one.
  assert.equal(certificationLaneOf(null), null);
});

test("routing is off unless the run asks for it", async () => {
  const manifest = { ...fourProbeManifest(), rows: [fourProbeManifest().rows[0]] };
  const temporary = mkdtempSync(join(tmpdir(), "solid-checker-lane-default-"));
  const hooks = successHooks();
  hooks.mkProject = async () => {
    const projectDir = join(temporary, "project");
    const outputDir = join(temporary, "output");
    mkdirSync(projectDir, { recursive: true });
    mkdirSync(outputDir, { recursive: true });
    return { projectDir, outputDir };
  };
  let emittedProposal = "";
  hooks.generateContract = async ({ outputPath }) => {
    emittedProposal = outputPath;
    writeFileSync(`${outputPath}.refusals.json`, JSON.stringify({
      format: "solid-checker-contract-proposal-refusals",
      refusalVersion: 1,
      package: { name: "@solid-primitives/alpha", version: "1.0.0" },
      refusals: [DEPENDENCY_BINDING_REFUSAL],
      inapplicable: []
    }));
    writeFileSync(`${outputPath}.certification-inputs.json`, "{}");
    return {
      status: 0,
      stdout:
        `generated unaccepted stable contract proposal for @solid-primitives/alpha@1.0.0 at ${outputPath}` +
        "; 1 artifact case(s) refused and omitted; proof verification must issue its receipt",
      stderr: "",
      timedOut: false
    };
  };
  hooks.planDependencies = () => ({
    schemaVersion: 1,
    rootIdentity: { package: "@solid-primitives/alpha", version: "1.0.0", integrity: "sha512-x" },
    status: "complete",
    complete: true,
    roots: [{ entrypoint: ".", conditions: [] }],
    nodes: [],
    edges: [],
    cycles: [],
    leaves: [],
    graphDigest: "sha256:x"
  });
  const certifications = [];
  hooks.attemptCertification = async args => {
    certifications.push(args);
    return { status: 1, stdout: "", stderr: "refused", timedOut: false };
  };
  try {
    // Same row that routes above, with the option absent: the emitted proposal
    // is handed over and no lane is requested. The default matters because the
    // routing loses receipts on the measured corpus.
    const [result] = await runBenchmark({
      manifest,
      hooks,
      options: { concurrency: 1, certificationConcurrency: 1, attemptCertification: true }
    });
    assert.equal(result.class, "partial-success");
    assert.equal(certifications.length, 1);
    assert.equal(certifications[0].dependencyGraphLane, false);
    assert.equal(certifications[0].proposal, emittedProposal);
    assert.equal(result.certificationAttempt.laneRequested, "reused-proposal");
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
});

test("a routed partial row withholds its proposal and asks for the graph lane", async () => {
  const manifest = { ...fourProbeManifest(), rows: [fourProbeManifest().rows[0]] };
  const temporary = mkdtempSync(join(tmpdir(), "solid-checker-lane-routing-"));
  const hooks = successHooks();
  hooks.mkProject = async () => {
    const projectDir = join(temporary, "project");
    const outputDir = join(temporary, "output");
    mkdirSync(projectDir, { recursive: true });
    mkdirSync(outputDir, { recursive: true });
    return { projectDir, outputDir };
  };
  // A partial generation whose only refusal is a dependency binding, with the
  // proposal sidecars the reuse path would otherwise pick up.
  hooks.generateContract = async ({ outputPath }) => {
    writeFileSync(`${outputPath}.refusals.json`, JSON.stringify({
      format: "solid-checker-contract-proposal-refusals",
      refusalVersion: 1,
      package: { name: "@solid-primitives/alpha", version: "1.0.0" },
      refusals: [DEPENDENCY_BINDING_REFUSAL],
      inapplicable: []
    }));
    writeFileSync(`${outputPath}.certification-inputs.json`, "{}");
    return {
      status: 0,
      stdout:
        `generated unaccepted stable contract proposal for @solid-primitives/alpha@1.0.0 at ${outputPath}` +
        "; 1 artifact case(s) refused and omitted; proof verification must issue its receipt",
      stderr: "",
      timedOut: false
    };
  };
  hooks.planDependencies = () => ({
    schemaVersion: 1,
    rootIdentity: { package: "@solid-primitives/alpha", version: "1.0.0", integrity: "sha512-x" },
    status: "complete",
    complete: true,
    roots: [{ entrypoint: ".", conditions: [] }],
    nodes: [],
    edges: [],
    cycles: [],
    leaves: [],
    graphDigest: "sha256:x"
  });
  const certifications = [];
  hooks.attemptCertification = async args => {
    certifications.push(args);
    return { status: 1, stdout: "", stderr: "refused", timedOut: false };
  };
  try {
    const [result] = await runBenchmark({
      manifest,
      hooks,
      options: {
        concurrency: 1,
        certificationConcurrency: 1,
        attemptCertification: true,
        dependencyGraphLane: true
      }
    });
    assert.equal(result.class, "partial-success");
    assert.equal(certifications.length, 1);
    assert.equal(certifications[0].dependencyGraphLane, true);
    assert.equal(certifications[0].proposal, "");
    assert.equal(result.certificationAttempt.laneRequested, "published-graph");
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
});

// The reuse branch of the routing rule is unreachable end-to-end today, and
// this pins *why* rather than leaving it untested: a partial row is queued for
// certification only when it has a complete dependency plan, and a dependency
// plan is only built from refusals `isDependencyCompositionRefusalText`
// matches. So a partial row whose refusals are all publisher defects is never
// certified in the first place. If that queue condition ever widens, this test
// fails and the reuse branch becomes reachable -- which is the moment to give
// it an end-to-end case of its own. The branch itself is pinned above, at
// `certificationLaneRequest`.
test("a partial row refused only on publisher defects is not queued for certification", async () => {
  const manifest = { ...fourProbeManifest(), rows: [fourProbeManifest().rows[0]] };
  const temporary = mkdtempSync(join(tmpdir(), "solid-checker-lane-reuse-"));
  const hooks = successHooks();
  hooks.mkProject = async () => {
    const projectDir = join(temporary, "project");
    const outputDir = join(temporary, "output");
    mkdirSync(projectDir, { recursive: true });
    mkdirSync(outputDir, { recursive: true });
    return { projectDir, outputDir };
  };
  let emittedProposal = "";
  hooks.generateContract = async ({ outputPath }) => {
    emittedProposal = outputPath;
    writeFileSync(`${outputPath}.refusals.json`, JSON.stringify({
      format: "solid-checker-contract-proposal-refusals",
      refusalVersion: 1,
      package: { name: "@solid-primitives/alpha", version: "1.0.0" },
      // Both classes present in the census: a dependency-composition *reason*
      // is what routes, and this row has none.
      refusals: [PUBLISHER_DEFECT_REFUSAL],
      inapplicable: []
    }));
    writeFileSync(`${outputPath}.certification-inputs.json`, "{}");
    return {
      status: 0,
      stdout:
        `generated unaccepted stable contract proposal for @solid-primitives/alpha@1.0.0 at ${outputPath}` +
        "; 1 artifact case(s) refused and omitted; proof verification must issue its receipt",
      stderr: "",
      timedOut: false
    };
  };
  hooks.planDependencies = () => ({
    schemaVersion: 1,
    rootIdentity: { package: "@solid-primitives/alpha", version: "1.0.0", integrity: "sha512-x" },
    status: "complete",
    complete: true,
    roots: [{ entrypoint: ".", conditions: [] }],
    nodes: [],
    edges: [],
    cycles: [],
    leaves: [],
    graphDigest: "sha256:x"
  });
  const certifications = [];
  hooks.attemptCertification = async args => {
    certifications.push(args);
    return { status: 1, stdout: "", stderr: "refused", timedOut: false };
  };
  try {
    const [result] = await runBenchmark({
      manifest,
      hooks,
      options: { concurrency: 1, certificationConcurrency: 1, attemptCertification: true }
    });
    assert.equal(result.class, "partial-success");
    assert.equal(certifications.length, 0);
    assert.equal(result.dependencyPlan, null);
    assert.equal(result.certificationAttempt, undefined);
    // The proposal was emitted and its sidecars exist; nothing consumed them.
    assert.equal(existsSync(`${emittedProposal}.certification-inputs.json`), true);
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
});

// ---------------------------------------------------------------------------
// Certified coverage: `k of n` entrypoints, read from the published catalog.
// ---------------------------------------------------------------------------

function writeCatalog(root, { relative = ".", contracts }) {
  const directory = join(root, relative);
  mkdirSync(join(directory, "objects"), { recursive: true });
  writeFileSync(join(directory, "accepted-contracts.json"), JSON.stringify({
    format: "solid-checker-accepted-contract-catalog",
    catalogVersion: 2,
    contracts: contracts.map((contract, index) => {
      const name = `objects/${index}.main.json`;
      writeFileSync(join(directory, name), JSON.stringify({
        format: "solid-reactivity-contract",
        schemaVersion: 1,
        package: { name: contract.package, version: contract.version ?? "1.0.0" },
        entrypoints: Object.fromEntries(
          contract.entrypoints.map(entrypoint => [entrypoint, { cases: [] }])
        )
      }));
      return { document: name, documentDigest: `sha256:${index}` };
    })
  }));
}

test("certified coverage counts only the row's own package, in both catalog layouts", () => {
  const temporary = mkdtempSync(join(tmpdir(), "solid-checker-coverage-"));
  const coverage = options =>
    readCertifiedCoverage({ packageVersion: "1.0.0", declaredEntrypoints: 4, ...options });
  try {
    const flat = join(temporary, "flat");
    writeCatalog(flat, {
      contracts: [
        { package: "solid-js", entrypoints: [".", "./refresh"] },
        // The graph lane publishes the dependencies into the same catalog.
        // Counting them would make a row read as more covered the more
        // dependencies it needed.
        { package: "@solidjs/signals", entrypoints: [".", "./map", "./store"] }
      ]
    });
    assert.deepEqual(coverage({ catalogPath: flat, packageName: "solid-js" }), {
      declaredEntrypoints: 4,
      declaredWildcard: false,
      certifiedEntrypoints: 2,
      rootCertified: true
    });

    // The policy-2 case-set lane publishes a pointer at the root, a case-set
    // document under `case-sets/<digest>/`, and one catalog per case named
    // relative to that document.
    const nested = join(temporary, "nested");
    const caseSet = "case-sets/deadbeef";
    mkdirSync(join(nested, caseSet), { recursive: true });
    writeFileSync(join(nested, "accepted-contract-case-set.json"), JSON.stringify({
      format: "solid-checker-accepted-contract-case-set-pointer",
      caseSetVersion: 1,
      document: `${caseSet}/accepted-contract-case-set.json`,
      documentDigest: "sha256:deadbeef"
    }));
    writeFileSync(join(nested, caseSet, "accepted-contract-case-set.json"), JSON.stringify({
      format: "solid-checker-accepted-contract-case-set",
      caseSetVersion: 1,
      cases: [
        { catalog: "cases/aa/accepted-contracts.json" },
        { catalog: "cases/bb/accepted-contracts.json" }
      ]
    }));
    writeCatalog(nested, {
      relative: `${caseSet}/cases/aa`,
      contracts: [{ package: "solid-js", entrypoints: ["./web"] }]
    });
    writeCatalog(nested, {
      relative: `${caseSet}/cases/bb`,
      contracts: [{ package: "solid-js", entrypoints: ["./store"] }]
    });
    // A leftover catalog the case set does not name must not count: the
    // pointer is the published index, not the directory listing.
    writeCatalog(nested, {
      relative: `${caseSet}/cases/stale`,
      contracts: [{ package: "solid-js", entrypoints: [".", "./legacy"] }]
    });
    // Nor may a leftover *root* catalog from an earlier single-case
    // publication to the same root. It carries the root entrypoint, so reading
    // it would both inflate the count and turn `rootCertified` true for a
    // receipt that does not cover the root.
    writeCatalog(nested, {
      contracts: [{ package: "solid-js", entrypoints: [".", "./legacy"] }]
    });
    assert.deepEqual(coverage({ catalogPath: nested, packageName: "solid-js" }), {
      declaredEntrypoints: 4,
      declaredWildcard: false,
      certifiedEntrypoints: 2,
      rootCertified: false
    });

    // No catalog: not measured. Never "covered nothing".
    assert.equal(coverage({ catalogPath: join(temporary, "absent"), packageName: "solid-js" }), null);
    assert.equal(coverage({ catalogPath: "", packageName: "solid-js" }), null);
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
});

test("certified coverage is identity-filtered, and an unfound identity is unmeasured", () => {
  const temporary = mkdtempSync(join(tmpdir(), "solid-checker-coverage-identity-"));
  const coverage = options => readCertifiedCoverage({ declaredEntrypoints: 4, ...options });
  try {
    // The graph lane can publish *another version of the row's own package*
    // into the same catalog -- a `@solidjs/signals` row whose graph pulls a
    // different prerelease is exactly the corpus shape. Name alone would
    // credit that node's entrypoints, and its `.`, to this row.
    const root = join(temporary, "two-versions");
    writeCatalog(root, {
      contracts: [
        { package: "@solidjs/signals", version: "2.0.0-rc.3", entrypoints: ["./map"] },
        { package: "@solidjs/signals", version: "2.0.0-rc.0", entrypoints: [".", "./store"] }
      ]
    });
    assert.deepEqual(
      coverage({
        catalogPath: root,
        packageName: "@solidjs/signals",
        packageVersion: "2.0.0-rc.3"
      }),
      {
        declaredEntrypoints: 4,
        declaredWildcard: false,
        certifiedEntrypoints: 1,
        rootCertified: false
      }
    );

    // A catalog that parses but holds no document for this exact identity is
    // not a measurement of zero coverage: a certified row published a receipt
    // by construction, so this reader failed to find what it covered.
    // `{certifiedEntrypoints: 0}` would have been counted as a partial row.
    assert.equal(
      coverage({
        catalogPath: root,
        packageName: "@solidjs/signals",
        packageVersion: "9.9.9"
      }),
      null
    );
    assert.equal(
      coverage({ catalogPath: root, packageName: "absent-package", packageVersion: "1.0.0" }),
      null
    );
    // A caller that cannot name the version cannot be told which documents are
    // the row's, so the answer is "not measured" rather than a name match.
    assert.equal(coverage({ catalogPath: root, packageName: "@solidjs/signals" }), null);
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
});

test("a wildcard manifest has no denominator even when the counts coincide", () => {
  const temporary = mkdtempSync(join(tmpdir(), "solid-checker-coverage-wildcard-"));
  try {
    const root = join(temporary, "wildcard");
    writeCatalog(root, {
      contracts: [{ package: "wildcard-package", entrypoints: [".", "./dist/a"] }]
    });
    // Two declared entries, one of them a pattern, and two certified. The
    // numbers coincide and the ratio is still meaningless: `./*` stands for as
    // many real entrypoints as the package ships.
    const coincident = readCertifiedCoverage({
      catalogPath: root,
      packageName: "wildcard-package",
      packageVersion: "1.0.0",
      declaredEntrypoints: 2,
      declaredWildcard: true
    });
    assert.deepEqual(coincident, {
      declaredEntrypoints: 2,
      declaredWildcard: true,
      certifiedEntrypoints: 2,
      rootCertified: true
    });
    assert.equal(hasUsableDenominator(coincident), false);
    assert.equal(isCompleteCoverage(coincident), false);
    // The same counts without a wildcard are a real, complete ratio -- so it
    // is the flag doing the work here, not the arithmetic.
    assert.equal(
      isCompleteCoverage({ ...coincident, declaredWildcard: false }),
      true
    );
    // `countDeclaredEntrypoints` is the only place the wildcard can be seen.
    assert.deepEqual(countDeclaredEntrypoints({ ".": "./index.js", "./*": "./dist/*.js" }), {
      count: 2,
      wildcard: true
    });
    assert.deepEqual(countDeclaredEntrypoints({ ".": "./index.js", "./util": "./util.js" }), {
      count: 2,
      wildcard: false
    });
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
});

test("an unreadable manifest leaves no denominator, which is unmeasured and never partial", () => {
  const temporary = mkdtempSync(join(tmpdir(), "solid-checker-coverage-null-"));
  try {
    const root = join(temporary, "no-manifest");
    writeCatalog(root, { contracts: [{ package: "opaque", entrypoints: [".", "./sub"] }] });
    const unmeasured = readCertifiedCoverage({
      catalogPath: root,
      packageName: "opaque",
      packageVersion: "1.0.0",
      declaredEntrypoints: null
    });
    assert.deepEqual(unmeasured, {
      declaredEntrypoints: null,
      declaredWildcard: false,
      certifiedEntrypoints: 2,
      rootCertified: true
    });
    // Neither half of the split, and no fabricated denominator in the report.
    assert.equal(isMeasuredCoverage(unmeasured), false);
    assert.equal(isCompleteCoverage(unmeasured), false);
    assert.equal(hasUsableDenominator(unmeasured), false);
    assert.equal(
      formatCoverage({ attempted: true, status: "certified", coverage: unmeasured }),
      "unmeasured 2 of ? (root)"
    );
    // The wildcard rendering must not claim a wildcard it never measured:
    // "null declared via wildcard" was the bug.
    assert.equal(
      formatCoverage({
        attempted: true,
        status: "certified",
        coverage: {
          declaredEntrypoints: 2,
          declaredWildcard: true,
          certifiedEntrypoints: 20,
          rootCertified: true
        }
      }),
      "partial 20 certified, 2 declared via wildcard (root)"
    );
    // The corpus-wide classification of the same row -- neither half of the
    // split -- is pinned in report.test.mjs, where the summary lives.
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
});

test("complete coverage needs a known denominator, a covered root, and every declared entrypoint", () => {
  assert.equal(
    isCompleteCoverage({ declaredEntrypoints: 4, certifiedEntrypoints: 4, rootCertified: true }),
    true
  );
  // A legacy-`main` package declares no exports map, so its whole surface is
  // the root. That is a manifest shape, not a missing measurement.
  assert.equal(
    isCompleteCoverage({ declaredEntrypoints: 0, certifiedEntrypoints: 1, rootCertified: true }),
    true
  );
  assert.equal(
    isCompleteCoverage({ declaredEntrypoints: 4, certifiedEntrypoints: 1, rootCertified: true }),
    false
  );
  // A wildcard subpath declares one entry and expands to many, so the declared
  // count is not a denominator and the row is not complete against it.
  // `@kobalte/utils@0.9.2` declares 2 and certifies 20.
  assert.equal(
    isCompleteCoverage({ declaredEntrypoints: 2, certifiedEntrypoints: 20, rootCertified: true }),
    false
  );
  assert.equal(
    hasUsableDenominator({ declaredEntrypoints: 2, certifiedEntrypoints: 20, rootCertified: true }),
    false
  );
  assert.equal(
    hasUsableDenominator({ declaredEntrypoints: 4, certifiedEntrypoints: 4, rootCertified: true }),
    true
  );
  // The legacy-`main` shape: one certified root against a declared 0 is not a
  // wildcard expansion.
  assert.equal(
    hasUsableDenominator({ declaredEntrypoints: 0, certifiedEntrypoints: 1, rootCertified: true }),
    true
  );
  assert.equal(
    isCompleteCoverage({ declaredEntrypoints: 4, certifiedEntrypoints: 4, rootCertified: false }),
    false
  );
  // An unknown denominator is not zero, and not complete.
  assert.equal(
    isCompleteCoverage({ declaredEntrypoints: null, certifiedEntrypoints: 4, rootCertified: true }),
    false
  );
  assert.equal(isCompleteCoverage(null), false);
});
