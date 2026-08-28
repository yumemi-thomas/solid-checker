import { test } from "vitest";
import assert from "node:assert/strict";

import { buildReport, renderMarkdown, evaluateThresholds } from "./lib/report.mjs";
import { classifyResult } from "./lib/classify.mjs";
import { FAMILIES } from "./lib/families.mjs";

// Every fixture below is modeled on scratchpad/spec/PILOT-EVIDENCE.md's real
// captured outcomes (2026-08-21, fresh debug binary + bin/solid-typefacts,
// solid-js@1.9.14) so the report is exercised on the actual shapes this
// checker produces, not an invented one. Where the pilot evidence gives an
// exact first-line stderr we run it through the real `classifyResult` so the
// resulting `class`/`signature`/`detail` come from classify.mjs itself, the
// same way run.mjs would produce them.

const ROUTER_STDERR =
  'solid-checker: solid-checker-rust: emit package contract: unresolved obligation at ' +
  '/tmp/probe-1/node_modules/@solidjs/router/dist/data/action.js:1364: ReactiveDispatchUnresolved ' +
  '{ callee: "action", member: Some("toString") }';

// Same documented shape as ROUTER_STDERR (see INTERFACES.md's "Verified real
// stderr shapes": `... ReactiveDispatchUnresolved { callee: "...", member: Some("...") }`)
// but with a different callee/member pair, standing in for motion-solidjs's
// real (uncaptured in the pilot) reactive-dispatch-unresolved failure. Used
// to prove two different callees still normalize into the same failure
// group.
const MOTION_STDERR =
  'solid-checker: solid-checker-rust: emit package contract: unresolved obligation at ' +
  '/tmp/probe-2/node_modules/motion-solidjs/dist/index.js:842: ReactiveDispatchUnresolved ' +
  '{ callee: "animate", member: Some("start") }';

const META_STDERR =
  'solid-checker: @solidjs/meta .:Stylesheet has different semantics across overlapping ' +
  'conditional-export branches [] and ["solid"]; schema v1 cannot represent export-map fallback ' +
  "ordering, so split the entrypoint or review an explicit contract";

const TANSTACK_QUERY_STDERR =
  'solid-checker: solid-checker-rust: emit package contract: cannot statically expand external ' +
  'export-all "@tanstack/query-core" from /tmp/probe-3/node_modules/@tanstack/solid-query/src/index.ts; ' +
  "generate and pass its dependency contract with --contract";

const CORVU_STDERR =
  'solid-checker: solid-checker-rust: emit package contract: cannot statically expand external ' +
  'export-all "@corvu/accordion" from /tmp/probe-4/node_modules/corvu/dist/accordion.jsx; generate and ' +
  "pass its dependency contract with --contract";

const MAP_STDERR =
  'solid-checker: solid-checker-rust: emit package contract: unresolved parameter behavior in createMap ' +
  "parameter 0 (any) at /tmp/probe-5/node_modules/@solid-primitives/map/dist/index.js:5783: parameter 0 " +
  "(any) is passed to resolved ReactiveMap.constructor from " +
  "/tmp/probe-5/node_modules/@solid-primitives/map/dist/index.js, but no package contract proves when it " +
  "executes; required behavior: invoked synchronously; edit this schema-v1 stub and review its evidence: " +
  "/tmp/probe-5/stub.json";

const KOBALTE_CORE_STDERR =
  "solid-checker: solid-checker-rust: native Solid compiler facts error: " +
  "/tmp/probe-6/node_modules/@kobalte/core/dist/chunk/DOJAEHTL.jsx: semantic trace has unresolved " +
  "execution sites: NativeAttribute@3275..3298";

function classifyFrom(stderr, status = 2) {
  return classifyResult({ status, stdout: "", stderr, timedOut: false, phase: "generate" });
}

// Builds one probe result in the exact shape run.mjs hands to buildReport
// (INTERFACES.md's "Probe result shape"), filling in the fields a given test
// doesn't care about with values that always keep the object internally
// consistent (e.g. a success always carries a non-null generatedEntrypoints).
function makeResult(overrides) {
  const outcome =
    overrides.outcome ??
    (overrides.class === "success"
      ? "success"
      : overrides.class === "partial-success"
        ? "partial-success"
        : "failure");
  // A partial contract exists on disk exactly like a complete one, so it
  // carries the same non-null generated/checklist counts plus the refusal
  // count that says what it left out.
  const producedContract = outcome === "success" || outcome === "partial-success";
  const probeKind = overrides.probeKind ?? "only";
  const solidTarget = overrides.solidTarget ?? "solid1";
  const probeId = overrides.probeId ?? `${overrides.package}@${overrides.version}|${solidTarget}|${probeKind}`;
  return {
    probeId,
    family: overrides.family,
    status: overrides.status ?? "official",
    package: overrides.package,
    version: overrides.version,
    solidTarget,
    probeKind,
    channel: overrides.channel ?? "stable",
    solid: overrides.solid ?? { "solid-js": "1.9.14" },
    installedVersions: overrides.installedVersions ?? { [overrides.package]: overrides.version },
    integrityVerified: overrides.integrityVerified ?? true,
    declaredEntrypoints: overrides.declaredEntrypoints ?? 1,
    generatedEntrypoints:
      overrides.generatedEntrypoints !== undefined ? overrides.generatedEntrypoints : producedContract ? 1 : null,
    refusedEntrypoints:
      overrides.refusedEntrypoints !== undefined
        ? overrides.refusedEntrypoints
        : outcome === "partial-success"
          ? 1
          : null,
    refusedArtifactCases: overrides.refusedArtifactCases ?? null,
    checklistItems: overrides.checklistItems !== undefined ? overrides.checklistItems : producedContract ? 10 : null,
    outcome,
    class: overrides.class,
    signature: overrides.signature ?? overrides.class,
    detail: overrides.detail ?? {},
    exitStatus: overrides.exitStatus ?? (producedContract ? 0 : 2),
    timedOut: overrides.timedOut ?? false,
    durationMs: overrides.durationMs ?? 1000,
    installDurationMs: overrides.installDurationMs ?? 500,
    generationDurationMs: overrides.generationDurationMs ?? 400,
    stdout: overrides.stdout ?? "",
    stderr: overrides.stderr ?? ""
  };
}

test("report exposes aggregate worker phase timings", () => {
  const results = [
    makeResult({ package: "alpha", class: "success", durationMs: 1000, installDurationMs: 400, generationDurationMs: 500 }),
    makeResult({ package: "bravo", class: "success", durationMs: 800, installDurationMs: 300, generationDurationMs: 450 })
  ];
  const report = buildReport({
    manifest: makeManifest({ results }),
    results,
    startedAt: "2026-08-21T09:00:00.000Z",
    finishedAt: "2026-08-21T09:00:01.000Z"
  });

  assert.deepEqual(report.combined.workerTimings, {
    totalDurationMs: 1800,
    installDurationMs: 700,
    generationDurationMs: 950,
    harnessDurationMs: 150
  });
  const markdown = renderMarkdown(report);
  assert.match(markdown, /Worker time: 1800 ms/);
  assert.match(markdown, /install 700 ms, generation 950 ms, harness 150 ms/);
});

// A minimal manifest good enough for `manifestStats` (rowCount/probeCount)
// and for the `limitations` verbatim-copy requirement — buildReport never
// calls `validateManifest`, so this does not need to satisfy every
// completeness rule that a real discovered manifest.json would.
function makeManifest({ results = [], limitations = [], generatedAt = "2026-08-21T09:00:00.000Z" } = {}) {
  const rowsByKey = new Map();
  for (const result of results) {
    const key = `${result.package}|${result.solidTarget}`;
    const row = rowsByKey.get(key);
    if (row) row.probes.push({});
    else rowsByKey.set(key, { family: result.family, probes: [{}] });
  }
  return { generatedAt, limitations, rows: [...rowsByKey.values()] };
}

function pilotSolid1Results() {
  return [
    // Successes (family, package, version straight from PILOT-EVIDENCE.md).
    makeResult({
      family: "kobalte",
      package: "@kobalte/utils",
      version: "0.9.2",
      class: "success",
      generatedEntrypoints: 1,
      checklistItems: 58,
      stdout: "generated @kobalte/utils@0.9.2 contract with 1 entrypoints at /out/1.json; review plan /out/1-plan.json (58 checklist items)"
    }),
    makeResult({
      family: "solid-primitives",
      package: "@solid-primitives/refs",
      version: "1.1.4",
      class: "success",
      generatedEntrypoints: 1,
      checklistItems: 11
    }),
    makeResult({
      family: "solid-primitives",
      package: "@solid-primitives/scheduled",
      version: "1.5.0",
      class: "success",
      generatedEntrypoints: 1,
      checklistItems: 12
    }),
    // Failures.
    (() => {
      const classified = classifyFrom(META_STDERR);
      return makeResult({
        family: "official-solid",
        package: "@solidjs/meta",
        version: "0.29.4",
        class: classified.class,
        signature: classified.signature,
        detail: classified.detail,
        stderr: META_STDERR
      });
    })(),
    (() => {
      const classified = classifyFrom(ROUTER_STDERR);
      return makeResult({
        family: "official-solid",
        package: "@solidjs/router",
        version: "0.15.3",
        class: classified.class,
        signature: classified.signature,
        detail: classified.detail,
        stderr: ROUTER_STDERR
      });
    })(),
    (() => {
      const classified = classifyFrom(MOTION_STDERR);
      return makeResult({
        family: "motion-solidjs",
        package: "motion-solidjs",
        version: "0.6.0",
        class: classified.class,
        signature: classified.signature,
        detail: classified.detail,
        stderr: MOTION_STDERR
      });
    })(),
    (() => {
      const classified = classifyFrom(TANSTACK_QUERY_STDERR);
      return makeResult({
        family: "tanstack",
        package: "@tanstack/solid-query",
        version: "5.101.4",
        class: classified.class,
        signature: classified.signature,
        detail: classified.detail,
        stderr: TANSTACK_QUERY_STDERR
      });
    })(),
    (() => {
      const classified = classifyFrom(CORVU_STDERR);
      return makeResult({
        family: "corvu",
        package: "corvu",
        version: "0.7.2",
        class: classified.class,
        signature: classified.signature,
        detail: classified.detail,
        stderr: CORVU_STDERR
      });
    })(),
    (() => {
      const classified = classifyFrom(MAP_STDERR);
      return makeResult({
        family: "solid-primitives",
        package: "@solid-primitives/map",
        version: "0.7.4",
        class: classified.class,
        signature: classified.signature,
        detail: classified.detail,
        stderr: MAP_STDERR
      });
    })(),
    (() => {
      const classified = classifyFrom(KOBALTE_CORE_STDERR);
      return makeResult({
        family: "kobalte",
        package: "@kobalte/core",
        version: "0.13.9",
        class: classified.class,
        signature: classified.signature,
        detail: classified.detail,
        stderr: KOBALTE_CORE_STDERR
      });
    })()
  ];
}

test("buildReport is byte-deterministic across repeated calls and shuffled input", () => {
  const results = pilotSolid1Results();
  const manifest = makeManifest({ results });
  const base = { manifest, results, startedAt: "2026-08-21T09:00:00.000Z", finishedAt: "2026-08-21T09:30:00.000Z" };

  const first = JSON.stringify(buildReport(base));
  const second = JSON.stringify(buildReport(base));
  assert.equal(first, second, "two calls with identical input must serialize identically");

  // A different arrival order for the same probes must not change the
  // report at all - grouping and sorting inside buildReport must fully
  // erase input order.
  const shuffled = [...results.slice(5), ...results.slice(0, 5)].reverse();
  const shuffledReport = JSON.stringify(buildReport({ ...base, results: shuffled }));
  assert.equal(first, shuffledReport, "shuffling the input results array must not change the serialized report");
});

test("solid1 and solid2 results are never merged", () => {
  const solid1 = makeResult({ family: "corvu", package: "corvu", version: "0.7.2", solidTarget: "solid1", class: "success" });
  const solid2 = makeResult({
    family: "corvu",
    package: "corvu",
    version: "0.8.0",
    solidTarget: "solid2",
    probeKind: "only",
    channel: "beta",
    class: "success"
  });
  const results = [solid1, solid2];
  const report = buildReport({
    manifest: makeManifest({ results }),
    results,
    startedAt: "2026-08-21T09:00:00.000Z",
    finishedAt: "2026-08-21T09:01:00.000Z"
  });

  assert.equal(report.solid1.totals.probeCount, 1);
  assert.equal(report.solid2.totals.probeCount, 1);
  const corvuSolid1 = report.solid1.families.find(section => section.family === "corvu");
  const corvuSolid2 = report.solid2.families.find(section => section.family === "corvu");
  assert.deepEqual(corvuSolid1.results.map(result => result.version), ["0.7.2"]);
  assert.deepEqual(corvuSolid2.results.map(result => result.version), ["0.8.0"]);
});

test("family grouping follows FAMILIES order regardless of result arrival order", () => {
  // One result per family, deliberately built in the REVERSE of FAMILIES
  // order so a naive "order by first appearance" implementation would fail
  // this.
  const results = [...FAMILIES]
    .reverse()
    .map(family =>
      makeResult({
        family: family.id,
        package: family.packages[0] ?? `${family.id}-pkg`,
        version: "1.0.0",
        class: "success"
      })
    );
  const report = buildReport({
    manifest: makeManifest({ results }),
    results,
    startedAt: "2026-08-21T09:00:00.000Z",
    finishedAt: "2026-08-21T09:01:00.000Z"
  });

  assert.deepEqual(report.solid1.families.map(section => section.family), FAMILIES.map(family => family.id));
});

test("two reactive-dispatch-unresolved failures with different callees land in one normalized group", () => {
  const router = pilotSolid1Results().find(result => result.package === "@solidjs/router");
  const motion = pilotSolid1Results().find(result => result.package === "motion-solidjs");
  assert.equal(router.class, "reactive-dispatch-unresolved");
  assert.equal(motion.class, "reactive-dispatch-unresolved");
  // The raw detail really does differ per package - this is what makes the
  // grouping below meaningful rather than a tautology.
  assert.notEqual(router.detail.callee, motion.detail.callee);

  const results = [router, motion];
  const report = buildReport({
    manifest: makeManifest({ results }),
    results,
    startedAt: "2026-08-21T09:00:00.000Z",
    finishedAt: "2026-08-21T09:01:00.000Z"
  });

  const dispatchGroups = report.combined.topFailureSignatures.filter(group => group.class === "reactive-dispatch-unresolved");
  assert.equal(dispatchGroups.length, 1, "different callees must not create separate groups");
  assert.equal(dispatchGroups[0].count, 2);
  assert.deepEqual(dispatchGroups[0].packages, ["@solidjs/router", "motion-solidjs"].sort());
});

test("shared blockers count each package once and exclude multi-blocker packages", () => {
  // Package A and B are blocked only by @tanstack/query-core; package C is
  // blocked by BOTH @tanstack/query-core and @corvu/accordion. If C were
  // (over-)counted under query-core, estimatedPackagesUnlocked would read 3
  // instead of 2, overstating what fixing that one contract would achieve.
  const packageA = makeResult({
    family: "tanstack",
    package: "@tanstack/solid-query",
    version: "5.101.4",
    class: "dependency-contract-obligation",
    detail: { module: "@tanstack/query-core" }
  });
  const packageB = makeResult({
    family: "tanstack",
    package: "@tanstack/solid-table",
    version: "8.20.0",
    class: "dependency-contract-obligation",
    detail: { module: "@tanstack/query-core" }
  });
  const packageC = makeResult({
    family: "tanstack",
    package: "@tanstack/solid-form",
    version: "0.1.0",
    solidTarget: "solid1",
    probeKind: "only",
    class: "dependency-contract-obligation",
    detail: { module: "@tanstack/query-core" },
    probeId: "@tanstack/solid-form@0.1.0|solid1|only#a"
  });
  const packageCSecondFailure = makeResult({
    family: "tanstack",
    package: "@tanstack/solid-form",
    version: "0.1.0",
    solidTarget: "solid2",
    probeKind: "only",
    class: "dependency-contract-obligation",
    detail: { module: "@corvu/accordion" }
  });

  const results = [packageA, packageB, packageC, packageCSecondFailure];
  const report = buildReport({
    manifest: makeManifest({ results }),
    results,
    startedAt: "2026-08-21T09:00:00.000Z",
    finishedAt: "2026-08-21T09:01:00.000Z"
  });

  const queryCoreBlocker = report.combined.sharedBlockers.find(blocker => blocker.module === "@tanstack/query-core");
  assert.ok(queryCoreBlocker, "expected a @tanstack/query-core shared blocker entry");
  assert.equal(queryCoreBlocker.estimatedPackagesUnlocked, 2, "C must not be counted as unlocked by query-core alone");
  assert.deepEqual(queryCoreBlocker.packages, ["@tanstack/solid-query", "@tanstack/solid-table"]);

  assert.equal(report.combined.multiBlockerPackages.length, 1);
  assert.equal(report.combined.multiBlockerPackages[0].package, "@tanstack/solid-form");
  assert.deepEqual(report.combined.multiBlockerPackages[0].modules, ["@corvu/accordion", "@tanstack/query-core"]);

  // C must never appear inside any single blocker's package list.
  for (const blocker of report.combined.sharedBlockers) {
    assert.ok(!blocker.packages.includes("@tanstack/solid-form"));
  }
});

test("floor/head differences require both probes to be present", () => {
  const worksOnFloorFailsAtHead = [
    makeResult({
      family: "solid-primitives",
      package: "@solid-primitives/refs",
      version: "2.0.0-beta.1",
      solidTarget: "solid2",
      probeKind: "floor",
      channel: "beta",
      class: "success"
    }),
    makeResult({
      family: "solid-primitives",
      package: "@solid-primitives/refs",
      version: "2.0.0-beta.1",
      solidTarget: "solid2",
      probeKind: "head",
      channel: "rc",
      class: "reactive-dispatch-unresolved",
      detail: { callee: "onCleanup" }
    })
  ];
  const failsOnFloorWorksAtHead = [
    makeResult({
      family: "solid-primitives",
      package: "@solid-primitives/scheduled",
      version: "2.0.0-beta.1",
      solidTarget: "solid2",
      probeKind: "floor",
      channel: "beta",
      class: "type-facts-failure"
    }),
    makeResult({
      family: "solid-primitives",
      package: "@solid-primitives/scheduled",
      version: "2.0.0-beta.1",
      solidTarget: "solid2",
      probeKind: "head",
      channel: "rc",
      class: "success"
    })
  ];
  // A single-probe ("only") package must never be forced into either list.
  const onlyProbePackage = makeResult({
    family: "solid-primitives",
    package: "@solid-primitives/map",
    version: "2.0.0-beta.5",
    solidTarget: "solid2",
    probeKind: "only",
    channel: "beta",
    class: "success"
  });

  const results = [...worksOnFloorFailsAtHead, ...failsOnFloorWorksAtHead, onlyProbePackage];
  const report = buildReport({
    manifest: makeManifest({ results }),
    results,
    startedAt: "2026-08-21T09:00:00.000Z",
    finishedAt: "2026-08-21T09:01:00.000Z"
  });

  assert.deepEqual(
    report.solid2.worksOnFloorFailsAtHead.map(entry => entry.package),
    ["@solid-primitives/refs"]
  );
  assert.deepEqual(
    report.solid2.failsOnFloorWorksAtHead.map(entry => entry.package),
    ["@solid-primitives/scheduled"]
  );
  for (const list of [report.solid2.worksOnFloorFailsAtHead, report.solid2.failsOnFloorWorksAtHead]) {
    assert.ok(!list.some(entry => entry.package === "@solid-primitives/map"));
  }
});

// Floor/head is the same ordered-scale comparison as the baseline diff: a
// package that emitted a complete contract on the floor and only a partial one
// at head lost entrypoints to the newer Solid, and a package that gained a
// partial contract at head improved. Neither move matched the old
// success/failure test, so both were invisible.
test("floor/head partial-success moves are classified by direction, not by success", () => {
  const worseAtHead = [
    makeResult({
      family: "solid-primitives",
      package: "@solid-primitives/refs",
      version: "2.0.0-beta.1",
      solidTarget: "solid2",
      probeKind: "floor",
      channel: "beta",
      class: "success"
    }),
    makeResult({
      family: "solid-primitives",
      package: "@solid-primitives/refs",
      version: "2.0.0-beta.1",
      solidTarget: "solid2",
      probeKind: "head",
      channel: "rc",
      class: "partial-success"
    })
  ];
  const betterAtHead = [
    makeResult({
      family: "solid-primitives",
      package: "@solid-primitives/scheduled",
      version: "2.0.0-beta.1",
      solidTarget: "solid2",
      probeKind: "floor",
      channel: "beta",
      class: "type-facts-failure"
    }),
    makeResult({
      family: "solid-primitives",
      package: "@solid-primitives/scheduled",
      version: "2.0.0-beta.1",
      solidTarget: "solid2",
      probeKind: "head",
      channel: "rc",
      class: "partial-success"
    })
  ];
  const results = [...worseAtHead, ...betterAtHead];
  const report = buildReport({
    manifest: makeManifest({ results }),
    results,
    startedAt: "2026-08-21T09:00:00.000Z",
    finishedAt: "2026-08-21T09:01:00.000Z"
  });

  assert.deepEqual(
    report.solid2.worksOnFloorFailsAtHead.map(entry => [entry.package, entry.floorOutcome, entry.headOutcome]),
    [["@solid-primitives/refs", "success", "partial-success"]]
  );
  assert.deepEqual(
    report.solid2.failsOnFloorWorksAtHead.map(entry => [entry.package, entry.floorOutcome, entry.headOutcome]),
    [["@solid-primitives/scheduled", "failure", "partial-success"]]
  );
  // The rendered rows name the transition, so a partial move is never read as
  // the package having stopped or started working outright.
  const markdown = renderMarkdown(report);
  assert.match(markdown, /### Worse at head than at floor/);
  assert.match(markdown, /- @solid-primitives\/refs \(solid-primitives\): success -> partial-success/);
  assert.match(markdown, /### Better at head than at floor/);
  assert.match(markdown, /- @solid-primitives\/scheduled \(solid-primitives\): failure -> partial-success/);
});

test("beta-only and RC-only classification comes from the probe channel", () => {
  const betaOnly = makeResult({
    family: "corvu",
    package: "corvu",
    version: "0.8.0-beta.3",
    solidTarget: "solid2",
    probeKind: "only",
    channel: "beta",
    class: "success"
  });
  const rcOnly = makeResult({
    family: "solid-devtools",
    package: "solid-devtools",
    version: "0.35.0-rc.1",
    solidTarget: "solid2",
    probeKind: "only",
    channel: "rc",
    class: "success"
  });
  const stableOnly = makeResult({
    family: "solid-recharts",
    package: "solid-recharts",
    version: "2.0.0",
    solidTarget: "solid2",
    probeKind: "only",
    channel: "stable",
    class: "success"
  });

  const results = [betaOnly, rcOnly, stableOnly];
  const report = buildReport({
    manifest: makeManifest({ results }),
    results,
    startedAt: "2026-08-21T09:00:00.000Z",
    finishedAt: "2026-08-21T09:01:00.000Z"
  });

  assert.deepEqual(report.solid2.betaOnlyPackages.map(entry => entry.package), ["corvu"]);
  assert.deepEqual(report.solid2.rcOnlyPackages.map(entry => entry.package), ["solid-devtools"]);
  assert.ok(!report.solid2.betaOnlyPackages.some(entry => entry.package === "solid-recharts"));
  assert.ok(!report.solid2.rcOnlyPackages.some(entry => entry.package === "solid-recharts"));
});

test("success percentage carries its denominator, and a probe-less family reports null not NaN", () => {
  const results = pilotSolid1Results();
  const report = buildReport({
    manifest: makeManifest({ results }),
    results,
    startedAt: "2026-08-21T09:00:00.000Z",
    finishedAt: "2026-08-21T09:01:00.000Z"
  });

  // solid-recharts and solid-devtools have zero solid1 probes in this
  // fixture set (only appear implicitly through motion-solidjs's family,
  // which is separate) - families.mjs still produces a section for them.
  const emptyFamily = report.solid1.families.find(section => section.probeCount === 0);
  assert.ok(emptyFamily, "expected at least one zero-probe family section");
  assert.deepEqual(emptyFamily.successRate, { percentage: null, successes: 0, total: 0 });

  const kobalte = report.solid1.families.find(section => section.family === "kobalte");
  assert.equal(kobalte.successRate.total, 2);
  assert.equal(kobalte.successRate.successes, 1);
  assert.equal(kobalte.successRate.percentage, 50);

  assert.ok(!JSON.stringify(report).includes("NaN"), "report JSON must never contain NaN");
});

test("a partial contract is never counted as a success, and its refusals are reported", () => {
  // The exact shape that made the checked-in report read "Declared 44 /
  // Generated 28 / Success 6/6 (100%)": every probe emitted a contract, but
  // some of those contracts describe only part of their package.
  const results = [
    makeResult({
      package: "@kobalte/core",
      version: "0.13.13",
      family: "kobalte",
      class: "partial-success",
      declaredEntrypoints: 44,
      generatedEntrypoints: 28,
      refusedEntrypoints: 16,
      refusedArtifactCases: null
    }),
    makeResult({
      package: "@kobalte/utils",
      version: "0.9.1",
      family: "kobalte",
      class: "success",
      declaredEntrypoints: 2,
      generatedEntrypoints: 2
    })
  ];
  const report = buildReport({
    manifest: makeManifest({ results }),
    results,
    startedAt: "2026-08-21T09:00:00.000Z",
    finishedAt: "2026-08-21T09:01:00.000Z"
  });

  const kobalte = report.solid1.families.find(section => section.family === "kobalte");
  assert.equal(kobalte.successCount, 1, "only the complete contract is a success");
  assert.equal(kobalte.partialCount, 1);
  assert.equal(kobalte.failureCount, 0, "a partial contract is not a failure either");
  assert.deepEqual(kobalte.successRate, { percentage: 50, successes: 1, total: 2 });
  // Both contracts really exist, so both contribute generated entrypoints;
  // what the refusal count adds is what they do NOT describe.
  assert.equal(kobalte.declaredEntrypoints, 46);
  assert.equal(kobalte.generatedEntrypoints, 30);
  assert.equal(kobalte.refusedEntrypoints, 16);

  assert.equal(report.solid1.totals.successCount, 1);
  assert.equal(report.solid1.totals.partialCount, 1);
  assert.equal(report.solid1.totals.refusedEntrypoints, 16);

  // A partial probe is in neither failureGroups nor successRate's numerator,
  // so it needs its own list or it would be invisible.
  assert.deepEqual(report.combined.partialContracts, [
    {
      probeId: "@kobalte/core@0.13.13|solid1|only",
      package: "@kobalte/core",
      version: "0.13.13",
      family: "kobalte",
      generatedEntrypoints: 28,
      refusedEntrypoints: 16,
      refusedArtifactCases: null
    }
  ]);
  assert.equal(
    report.combined.familyComparison.find(entry => entry.family === "kobalte").solid1.partialCount,
    1
  );

  const markdown = renderMarkdown(report);
  assert.match(markdown, /- Refused entrypoints \(partial contracts\): 16/);
  assert.match(markdown, /- Partial contracts: 1/);
  assert.match(markdown, /- Success \(complete contracts\): 1\/2 \(50%\)/);
  assert.match(markdown, /### Partial contracts/);
  assert.match(
    markdown,
    /- @kobalte\/core@0\.13\.13 \(kobalte\): 28 entrypoint\(s\) generated, 16 entrypoint\(s\) and 0 artifact case\(s\) refused/
  );
});

test("a baseline success that becomes a partial contract is a regression", () => {
  const results = [
    makeResult({ package: "@kobalte/core", version: "0.13.13", family: "kobalte", class: "partial-success" })
  ];
  const baseline = {
    results: [
      {
        probeId: "@kobalte/core@0.13.13|solid1|only",
        package: "@kobalte/core",
        outcome: "success",
        class: "success"
      }
    ]
  };
  const report = buildReport({
    manifest: makeManifest({ results }),
    results,
    baseline,
    startedAt: "2026-08-21T09:00:00.000Z",
    finishedAt: "2026-08-21T09:01:00.000Z"
  });

  assert.equal(report.combined.baseline.regressionCount, 1);
  assert.deepEqual(report.combined.baseline.regressions[0], {
    probeId: "@kobalte/core@0.13.13|solid1|only",
    package: "@kobalte/core",
    previousClass: "success",
    currentClass: "partial-success",
    previousOutcome: "success",
    currentOutcome: "partial-success"
  });
});

// The move the `success` / `not success` comparison matched on neither side:
// the probe had a partial contract and now has none at all, which is the run
// where the contract disappeared entirely. It must count as a regression.
test("a baseline partial contract that becomes a failure is a regression", () => {
  const results = [
    makeResult({ package: "@kobalte/core", version: "0.13.13", family: "kobalte", class: "unclassified" })
  ];
  const baseline = {
    results: [
      {
        probeId: "@kobalte/core@0.13.13|solid1|only",
        package: "@kobalte/core",
        outcome: "partial-success",
        class: "partial-success"
      }
    ]
  };
  const report = buildReport({
    manifest: makeManifest({ results }),
    results,
    baseline,
    startedAt: "2026-08-21T09:00:00.000Z",
    finishedAt: "2026-08-21T09:01:00.000Z"
  });

  assert.equal(report.combined.baseline.regressionCount, 1);
  assert.equal(report.combined.baseline.fixCount, 0);
  assert.deepEqual(report.combined.baseline.regressions[0], {
    probeId: "@kobalte/core@0.13.13|solid1|only",
    package: "@kobalte/core",
    previousClass: "partial-success",
    currentClass: "unclassified",
    previousOutcome: "partial-success",
    currentOutcome: "failure"
  });
});

// The symmetric upward move: a probe that emitted nothing now emits a partial
// contract. That is an improvement, and the rendered line must not overstate it
// as a complete one.
test("a baseline failure that becomes a partial contract is a fix, rendered as partial", () => {
  const results = [
    makeResult({ package: "@kobalte/core", version: "0.13.13", family: "kobalte", class: "partial-success" })
  ];
  const baseline = {
    results: [
      {
        probeId: "@kobalte/core@0.13.13|solid1|only",
        package: "@kobalte/core",
        outcome: "failure",
        class: "unclassified"
      }
    ]
  };
  const report = buildReport({
    manifest: makeManifest({ results }),
    results,
    baseline,
    startedAt: "2026-08-21T09:00:00.000Z",
    finishedAt: "2026-08-21T09:01:00.000Z"
  });

  assert.equal(report.combined.baseline.fixCount, 1);
  assert.equal(report.combined.baseline.regressionCount, 0);
  assert.deepEqual(report.combined.baseline.fixes[0], {
    probeId: "@kobalte/core@0.13.13|solid1|only",
    package: "@kobalte/core",
    previousClass: "unclassified",
    currentClass: "partial-success",
    previousOutcome: "failure",
    currentOutcome: "partial-success"
  });
  assert.match(
    renderMarkdown(report),
    /@kobalte\/core@0\.13\.13\|solid1\|only: unclassified -> partial-success/
  );
});

test("renderMarkdown covers both Solid versions and every family, and flags truncated stderr", () => {
  const longStderr = `solid-checker: solid-checker-rust: ${"x".repeat(1000)}`;
  const longFailure = makeResult({
    family: "solid-primitives",
    package: "@solid-primitives/map",
    version: "0.7.4",
    class: "unclassified",
    stderr: longStderr
  });
  const results = [...pilotSolid1Results(), longFailure];
  const report = buildReport({
    manifest: makeManifest({ results }),
    results,
    startedAt: "2026-08-21T09:00:00.000Z",
    finishedAt: "2026-08-21T09:30:00.000Z",
    checker: { nativeBin: "/repo/rust/target/debug/solid-checker-rust", typeFactsBin: "/repo/bin/solid-typefacts" }
  });

  const markdown = renderMarkdown(report);
  assert.match(markdown, /## Solid 1\.x/);
  assert.match(markdown, /## Solid 2\.x/);
  assert.match(markdown, /## Combined/);
  for (const family of FAMILIES) {
    assert.ok(markdown.includes(`### ${family.label}`), `expected a section for ${family.label}`);
  }
  assert.match(markdown, /truncated/i);
  assert.ok(markdown.includes("/repo/rust/target/debug/solid-checker-rust"));
  assert.ok(markdown.includes("/repo/bin/solid-typefacts"));
});

test("evaluateThresholds fails a regressed family, passes when met, and ignores families with no threshold", () => {
  const results = pilotSolid1Results();
  const report = buildReport({
    manifest: makeManifest({ results }),
    results,
    startedAt: "2026-08-21T09:00:00.000Z",
    finishedAt: "2026-08-21T09:01:00.000Z"
  });

  // kobalte has 1 success out of 2 probes in this fixture set.
  const regressed = evaluateThresholds(report, { families: { kobalte: { minSuccessCount: 2 } } });
  assert.equal(regressed.ok, false);
  assert.equal(regressed.failures.length, 1);
  assert.equal(regressed.failures[0].scope, "family:kobalte");

  const met = evaluateThresholds(report, { families: { kobalte: { minSuccessCount: 1 } } });
  assert.equal(met.ok, true);
  assert.deepEqual(met.failures, []);

  // "corvu" is not named in thresholds at all - it must not surface as a
  // failure even though it has zero successes in this fixture set.
  const absentFamily = evaluateThresholds(report, { families: { "solid-primitives": { minSuccessCount: 2 } } });
  assert.ok(!absentFamily.failures.some(failure => failure.scope.includes("corvu")));

  const globalCheck = evaluateThresholds(report, { global: { minSuccessCount: 100 } });
  assert.equal(globalCheck.ok, false);
  assert.equal(globalCheck.failures[0].scope, "global");

  const percentageCheck = evaluateThresholds(report, {
    global: { minSuccessPercentage: 85 },
    families: { kobalte: { minSuccessPercentage: 75 } }
  });
  assert.equal(percentageCheck.ok, false);
  assert.deepEqual(
    percentageCheck.failures.map(failure => [failure.scope, failure.metric]),
    [
      ["family:kobalte", "successPercentage"],
      ["global", "successPercentage"]
    ]
  );

  const generatableCheck = evaluateThresholds(report, {
    global: { minGeneratablePercentage: 1 },
    families: { kobalte: { minGeneratablePercentage: 90 } }
  });
  assert.equal(generatableCheck.ok, false);
  assert.deepEqual(generatableCheck.failures.map(failure => failure.metric), [
    "generatablePercentage"
  ]);
});

test("discovery limitations are copied into the combined section verbatim", () => {
  const limitations = [
    "the @solid-primitives org listing endpoint is unauthenticated and complete",
    "npm search results are used only for supplemental fork detection, never trusted for org membership"
  ];
  const results = pilotSolid1Results();
  const report = buildReport({
    manifest: makeManifest({ results, limitations }),
    results,
    startedAt: "2026-08-21T09:00:00.000Z",
    finishedAt: "2026-08-21T09:01:00.000Z"
  });

  assert.deepEqual(report.combined.discoveryLimitations, limitations);
});

test("baseline comparison reports regressions and fixes by probe id", () => {
  const priorRouter = makeResult({
    family: "official-solid",
    package: "@solidjs/router",
    version: "0.15.3",
    class: "success"
  });
  const priorMap = makeResult({
    family: "solid-primitives",
    package: "@solid-primitives/map",
    version: "0.7.4",
    class: "unresolved-parameter-behavior"
  });
  const baseline = { results: [priorRouter, priorMap] };

  const currentRouter = classifyFrom(ROUTER_STDERR);
  const currentRouterResult = makeResult({
    family: "official-solid",
    package: "@solidjs/router",
    version: "0.15.3",
    class: currentRouter.class,
    signature: currentRouter.signature,
    detail: currentRouter.detail,
    stderr: ROUTER_STDERR
  });
  const currentMapResult = makeResult({
    family: "solid-primitives",
    package: "@solid-primitives/map",
    version: "0.7.4",
    class: "success"
  });

  const results = [currentRouterResult, currentMapResult];
  const report = buildReport({
    manifest: makeManifest({ results }),
    results,
    startedAt: "2026-08-21T09:00:00.000Z",
    finishedAt: "2026-08-21T09:01:00.000Z",
    baseline
  });

  assert.equal(report.combined.baseline.provided, true);
  assert.equal(report.combined.baseline.regressionCount, 1);
  assert.equal(report.combined.baseline.regressions[0].probeId, currentRouterResult.probeId);
  assert.equal(report.combined.baseline.fixCount, 1);
  assert.equal(report.combined.baseline.fixes[0].probeId, currentMapResult.probeId);

  const noBaselineReport = buildReport({
    manifest: makeManifest({ results }),
    results,
    startedAt: "2026-08-21T09:00:00.000Z",
    finishedAt: "2026-08-21T09:01:00.000Z"
  });
  assert.deepEqual(noBaselineReport.combined.baseline, { provided: false });
});

// Unofficial forks are recorded for review, never counted as the official
// project. Folding a fork's failure into @kobalte/core's success rate would
// attribute a stranger's code to the Kobalte maintainers.
test("supplemental fork results never enter an official family's totals", () => {
  const manifest = { schemaVersion: 1, generatedAt: "2026-08-22T00:00:00.000Z", rows: [], exclusions: [], supplemental: [], limitations: [] };
  const official = {
    probeId: "@kobalte/core@0.13.13|solid1|only",
    family: "kobalte",
    status: "official",
    package: "@kobalte/core",
    version: "0.13.13",
    solidTarget: "solid1",
    probeKind: "only",
    channel: "stable",
    solid: { "solid-js": "1.9.14" },
    installedVersions: {},
    integrityVerified: true,
    declaredEntrypoints: 1,
    generatedEntrypoints: 1,
    checklistItems: 3,
    outcome: "success",
    class: "success",
    signature: "generated",
    detail: {},
    exitStatus: 0,
    timedOut: false,
    durationMs: 10,
    installDurationMs: 5,
    stdout: "",
    stderr: ""
  };
  const fork = {
    ...official,
    probeId: "@trellis-app/kobalte-core@0.13.1|solid1|only",
    status: "supplemental",
    package: "@trellis-app/kobalte-core",
    outcome: "failure",
    class: "type-facts-failure",
    signature: "native Solid compiler facts error",
    generatedEntrypoints: null,
    checklistItems: null,
    exitStatus: 2
  };
  const report = buildReport({
    manifest,
    results: [official, fork],
    startedAt: "2026-08-22T00:00:00.000Z",
    finishedAt: "2026-08-22T00:01:00.000Z"
  });
  const kobalte = report.solid1.families.find(entry => entry.family === "kobalte");
  assert.equal(kobalte.probeCount, 1, "the fork must not be counted as a Kobalte probe");
  assert.equal(kobalte.failureCount, 0, "the fork's failure must not be Kobalte's failure");
  assert.equal(kobalte.successCount, 1);
  assert.equal(report.solid1.totals.probeCount, 1);
  // It is still reported, just on its own.
  assert.equal(report.supplemental.probeCount, 1);
  assert.equal(report.supplemental.results[0].package, "@trellis-app/kobalte-core");
  // And it must not pollute the cross-cutting failure analysis either.
  const signatures = report.combined.topFailureSignatures.map(entry => entry.signature);
  assert.ok(!signatures.includes("native Solid compiler facts error"), "fork failure must not appear in official failure groups");
});

// The "packages unlocked per blocker" figure exists to rank what to fix first.
// Restricting it to one failure class hid the biggest blockers in the real
// corpus: 83 consumer-side contract failures naming @solidjs/web and solid-js
// never reached the analysis, so the report ranked ten one-package blockers and
// omitted the one worth dozens.
test("every class that names a blocking module feeds the shared-blocker analysis", () => {
  const manifest = { schemaVersion: 1, generatedAt: "2026-08-22T00:00:00.000Z", rows: [], exclusions: [], supplemental: [], limitations: [] };
  const base = {
    family: "solid-primitives",
    status: "official",
    solidTarget: "solid2",
    probeKind: "only",
    channel: "beta",
    solid: { "solid-js": "2.0.0-rc.1" },
    installedVersions: {},
    integrityVerified: true,
    declaredEntrypoints: 1,
    generatedEntrypoints: null,
    checklistItems: null,
    outcome: "failure",
    exitStatus: 2,
    timedOut: false,
    durationMs: 1,
    installDurationMs: 1,
    stdout: "",
    stderr: ""
  };
  const results = [
    { ...base, probeId: "a|solid2|only", package: "a", version: "1.0.0", class: "package-contract-environment-dependent", signature: "env", detail: { module: "@solidjs/web" } },
    { ...base, probeId: "b|solid2|only", package: "b", version: "1.0.0", class: "package-contract-environment-dependent", signature: "env", detail: { module: "@solidjs/web" } },
    { ...base, probeId: "c|solid2|only", package: "c", version: "1.0.0", class: "package-contract-export-missing", signature: "missing", detail: { module: "solid-js" } },
    { ...base, probeId: "d|solid2|only", package: "d", version: "1.0.0", class: "dependency-contract-obligation", signature: "dep", detail: { module: "@tanstack/query-core" } }
  ];
  const report = buildReport({ manifest, results, startedAt: "2026-08-22T00:00:00.000Z", finishedAt: "2026-08-22T00:00:01.000Z" });
  const blockers = new Map(report.combined.sharedBlockers.map(entry => [entry.module, entry.estimatedPackagesUnlocked]));
  assert.equal(blockers.get("@solidjs/web"), 2, "environment-dependent failures must be counted as blockers");
  assert.equal(blockers.get("solid-js"), 1, "export-missing failures must be counted as blockers");
  assert.equal(blockers.get("@tanstack/query-core"), 1);
  // Ranked most-unlocked first, so the biggest blocker is what a reader sees.
  assert.equal(report.combined.sharedBlockers[0].module, "@solidjs/web");
});

// The floor/head figure answers "does this package behave differently on an
// early beta than on a newer RC". The package version is identical in both
// probes, so the entry has to name the Solid environments that differ -- and
// the classes, so a reader can see WHAT changed, not merely that something did.
test("a floor/head divergence names the Solid environments and classes, not the package version twice", () => {
  const manifest = { schemaVersion: 1, generatedAt: "2026-08-22T00:00:00.000Z", rows: [], exclusions: [], supplemental: [], limitations: [] };
  const base = {
    family: "solid-primitives",
    status: "official",
    package: "@solid-primitives/example",
    version: "2.0.0-next.1",
    solidTarget: "solid2",
    installedVersions: {},
    integrityVerified: true,
    declaredEntrypoints: 1,
    checklistItems: null,
    timedOut: false,
    durationMs: 1,
    installDurationMs: 1,
    stdout: "",
    stderr: "",
    detail: {}
  };
  const results = [
    { ...base, probeId: "x|solid2|floor", probeKind: "floor", channel: "beta", solid: { "solid-js": "2.0.0-beta.17" },
      outcome: "failure", class: "reactive-dispatch-unresolved", signature: "sig-a", generatedEntrypoints: null, exitStatus: 2 },
    { ...base, probeId: "x|solid2|head", probeKind: "head", channel: "rc", solid: { "solid-js": "2.0.0-rc.1" },
      outcome: "success", class: "success", signature: "ok", generatedEntrypoints: 1, exitStatus: 0 }
  ];
  const report = buildReport({ manifest, results, startedAt: "2026-08-22T00:00:00.000Z", finishedAt: "2026-08-22T00:00:01.000Z" });
  assert.equal(report.solid2.worksOnFloorFailsAtHead.length, 0);
  assert.equal(report.solid2.failsOnFloorWorksAtHead.length, 1);
  const entry = report.solid2.failsOnFloorWorksAtHead[0];
  assert.equal(entry.packageVersion, "2.0.0-next.1");
  assert.deepEqual(entry.floorSolid, { "solid-js": "2.0.0-beta.17" });
  assert.deepEqual(entry.headSolid, { "solid-js": "2.0.0-rc.1" });
  assert.equal(entry.floorClass, "reactive-dispatch-unresolved");
  assert.equal(entry.headClass, "success");
});

test("a filtered report identifies itself as partial rather than reading as a full run", () => {
  // The `Manifest generated at` line describes the corpus the run was selected
  // from -- 417 probes even for a 23-probe sentinel. Without the scope line a
  // reader cannot tell the two artifacts apart at a glance.
  const report = buildReport({
    manifest: { generatedAt: "2026-01-01T00:00:00.000Z", rows: [] },
    results: [],
    startedAt: "2026-01-01T00:00:00.000Z",
    finishedAt: "2026-01-01T00:00:01.000Z",
    scope: { kind: "filtered", sentinel: true, families: [], solidTargets: [], includeSupplemental: false }
  });
  assert.equal(report.scope.kind, "filtered");
  assert.equal(report.scope.sentinel, true);
  assert.match(renderMarkdown(report), /- Scope: PARTIAL -- sentinel subset \(0 probes run\)/);

  const full = buildReport({
    manifest: { generatedAt: "2026-01-01T00:00:00.000Z", rows: [] },
    results: [],
    startedAt: "2026-01-01T00:00:00.000Z",
    finishedAt: "2026-01-01T00:00:01.000Z"
  });
  assert.equal(full.scope.kind, "full");
  assert.match(renderMarkdown(full), /- Scope: full corpus/);
});
