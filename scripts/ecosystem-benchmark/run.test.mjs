import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "vitest";

import { buildInstallArguments } from "./lib/install.mjs";
import {
  checkRequiredBinaries,
  countDeclaredEntrypoints,
  decideExitCode,
  defaultReportPaths,
  probeOutcome,
  recommendedConcurrency,
  resolveProbeIdFilter,
  runBenchmark,
  runScope,
  startProgressHeartbeat
} from "./run.mjs";

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
        stageDurationsMs: { artifactAcquisition: 1, demandPlanning: 2 }
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
      stage: "witness-acquisition",
      owner: "type-facts",
      demandId: "sha256:missing",
      family: "selected-signature",
      reason: "the automatic type-facts witness adapter is unavailable",
      refusalCount: 1,
      durationMs: 0,
      stageDurationsMs: { artifactAcquisition: 1, demandPlanning: 2 },
      demandCountsByFamily: { "package-identity": 1, "selected-signature": 1 },
      artifactSatisfiedDemandsByFamily: { "package-identity": 1 },
      refusalCountsByFamily: {},
      refusalCountsByOwner: {}
    });
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
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

test("recommendedConcurrency follows available CPUs but caps process fan-out at eight", () => {
  assert.equal(recommendedConcurrency(1), 1);
  assert.equal(recommendedConcurrency(4), 4);
  assert.equal(recommendedConcurrency(12), 8);
  assert.equal(recommendedConcurrency(Number.NaN), 4);
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
  assert.equal(countDeclaredEntrypoints("./index.js"), 1);
  assert.equal(countDeclaredEntrypoints({ import: "./index.mjs", require: "./index.cjs" }), 1);
  assert.equal(countDeclaredEntrypoints({ ".": "./index.js", "./util": "./util.js" }), 2);
  assert.equal(countDeclaredEntrypoints({ ".": "./index.js", "./*": "./dist/*.js" }), 2);
  assert.equal(countDeclaredEntrypoints(null), 0);
  assert.equal(countDeclaredEntrypoints(undefined), 0);
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
});

test("runScope records which filters produced a run", () => {
  const scope = runScope({ sentinel: true, families: ["kobalte"] });
  assert.equal(scope.kind, "filtered");
  assert.equal(scope.sentinel, true);
  assert.deepEqual(scope.families, ["kobalte"]);
  assert.equal(runScope({}).kind, "full");
});
