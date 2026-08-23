import test from "node:test";
import assert from "node:assert/strict";
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";

import {
  BEHAVIORAL_ROW_KINDS,
  CLAIM_DOMAINS,
  isUnknownClaim,
  readContractContent,
  reviewPlanPathFor,
  summarizeContract,
  summarizeContractDocument,
  summarizeReviewPlan
} from "./lib/contract-content.mjs";
import { buildReport, renderMarkdown } from "./lib/report.mjs";
import { runBenchmark } from "./run.mjs";

// Fixtures below are shaped after the real artifacts this measurement was
// built against: @tanstack/solid-query@5.101.4 on solid-js@1.9.14 emitted 17
// summaries covering 57 export names, with `callbacks` unknown on `useQuery`,
// `useInfiniteQuery` (both at the default level AND inside their
// `@tanstack/custom-condition` variant) and on the re-exported
// `replaceEqualDeep`. Nothing here is invented shape: every field is one the
// checked-in schema/solid-reactivity.schema.json defines.

function contractWith(summaries, exportsMap, extraEntrypoints = {}) {
  return {
    schemaVersion: 1,
    package: { name: "fixture", version: "1.0.0" },
    summaries,
    entrypoints: { ".": { exports: exportsMap }, ...extraEntrypoints }
  };
}

const UNKNOWN = { status: "unknown" };

// ---------------------------------------------------------------------------
// The sentinel itself
// ---------------------------------------------------------------------------

test("isUnknownClaim accepts only the exact one-key sentinel", () => {
  assert.equal(isUnknownClaim({ status: "unknown" }), true);
  // A real claim array, an empty one, and a null are all claims (or absences),
  // never the sentinel.
  assert.equal(isUnknownClaim([]), false);
  assert.equal(isUnknownClaim([{ parameter: 0, execution: "inline" }]), false);
  assert.equal(isUnknownClaim(null), false);
  assert.equal(isUnknownClaim(undefined), false);
  assert.equal(isUnknownClaim("unknown"), false);
  // Extra keys mean it is not the schema's `unknownClaim`, which is
  // additionalProperties:false with `status` its only member.
  assert.equal(isUnknownClaim({ status: "unknown", parameter: 0 }), false);
  assert.equal(isUnknownClaim({ status: "known" }), false);
});

test("the five claim domains are exactly the schema's unknown-capable ones", () => {
  assert.deepEqual(CLAIM_DOMAINS, [
    "callbacks",
    "reactiveReads",
    "returns",
    "ownerRequirements",
    "asyncBehavior"
  ]);
});

// ---------------------------------------------------------------------------
// Counting a contract document
// ---------------------------------------------------------------------------

test("counting is per export NAME, not per summary id", () => {
  // One summary shared by three exported names, exactly how a contract records
  // a barrel of identically-shaped re-exports. A consumer imports a name, so
  // one unknown summary is three unknown exports.
  const document = summarizeContractDocument(
    contractWith({ "function-1": { kind: "function", callbacks: UNKNOWN } }, { "function-1": ["a", "b", "c"] })
  );

  assert.equal(document.exportsTotal, 3);
  assert.equal(document.exportsWithUnknown, 3);
  assert.equal(document.exportsProven, 0);
  assert.equal(document.unknownByDomain.callbacks, 3);
  assert.equal(document.unknownTotal, 3);
});

test("an absent claim domain is a positive claim, never an unknown", () => {
  // `kind: "function"` with no `callbacks` key asserts the export never
  // invokes a caller-supplied callback. Counting that as uncertainty would
  // report the checker as unsure exactly where it was most confident.
  const document = summarizeContractDocument(
    contractWith({ "function-1": { kind: "function" }, value: { kind: "value" } }, { "function-1": ["run"], value: ["DEV"] })
  );

  assert.equal(document.exportsTotal, 2);
  assert.equal(document.exportsProven, 2);
  assert.equal(document.exportsWithUnknown, 0);
  assert.equal(document.unknownTotal, 0);
});

test("an unknown inside a variant is the export's unknown, counted once", () => {
  // The real @tanstack/solid-query useQuery shape: unknown at the default
  // level AND in one variant, with another variant resolving cleanly. That is
  // ONE unknown callbacks claim for this export, not two or three.
  const document = summarizeContractDocument(
    contractWith(
      {
        "function-1": {
          kind: "function",
          callbacks: UNKNOWN,
          variants: [
            { conditions: ["@tanstack/custom-condition"], summary: { kind: "function", callbacks: UNKNOWN } },
            {
              conditions: ["import"],
              summary: { kind: "function", callbacks: [{ parameter: 0, execution: "tracked" }] }
            }
          ]
        }
      },
      { "function-1": ["useQuery"] }
    )
  );

  assert.equal(document.exportsWithUnknown, 1);
  assert.equal(document.unknownByDomain.callbacks, 1);
  assert.equal(document.unknownTotal, 1);
});

test("an unknown only in a variant still makes the export unknown", () => {
  const document = summarizeContractDocument(
    contractWith(
      {
        "function-1": {
          kind: "function",
          callbacks: [{ parameter: 0, execution: "inline" }],
          variants: [{ conditions: ["server"], summary: { kind: "function", callbacks: UNKNOWN } }]
        }
      },
      { "function-1": ["render"] }
    )
  );

  assert.equal(document.exportsProven, 0);
  assert.equal(document.exportsWithUnknown, 1);
  assert.equal(document.unknownByDomain.callbacks, 1);
});

test("each domain is counted separately and an export with several is one unknown export", () => {
  const document = summarizeContractDocument(
    contractWith(
      {
        "function-1": {
          kind: "function",
          callbacks: UNKNOWN,
          reactiveReads: UNKNOWN,
          returns: UNKNOWN,
          ownerRequirements: UNKNOWN,
          asyncBehavior: UNKNOWN
        }
      },
      { "function-1": ["everything"] }
    )
  );

  assert.equal(document.exportsTotal, 1);
  assert.equal(document.exportsWithUnknown, 1);
  assert.equal(document.unknownTotal, 5);
  assert.deepEqual(document.unknownByDomain, {
    callbacks: 1,
    reactiveReads: 1,
    returns: 1,
    ownerRequirements: 1,
    asyncBehavior: 1
  });
  // The shape that dominates the real corpus: @kobalte/core@0.13.13 emits ONE
  // such summary and shares it across 452 export names. Counted separately so
  // the five-column table is not read as five independent gaps.
  assert.equal(document.exportsAllDomainsUnknown, 1);
});

test("all-five-domains and variant-only unknowns are tracked separately", () => {
  const document = summarizeContractDocument(
    contractWith(
      {
        // Every domain unknown, no variants: the whole-summary shape.
        "function-1": {
          kind: "function",
          callbacks: UNKNOWN,
          reactiveReads: UNKNOWN,
          returns: UNKNOWN,
          ownerRequirements: UNKNOWN,
          asyncBehavior: UNKNOWN
        },
        // Default fully claimed, uncertainty confined to one condition set.
        "function-2": {
          kind: "function",
          callbacks: [{ parameter: 0, execution: "inline" }],
          variants: [{ conditions: ["server"], summary: { kind: "function", reactiveReads: UNKNOWN } }]
        },
        // Unknown on the default: not variant-only, not all five.
        "function-3": { kind: "function", callbacks: UNKNOWN }
      },
      { "function-1": ["a"], "function-2": ["b"], "function-3": ["c"] }
    )
  );

  assert.equal(document.exportsWithUnknown, 3);
  assert.equal(document.exportsAllDomainsUnknown, 1);
  assert.equal(document.exportsUnknownOnlyInVariants, 1);
});

test("positive behavioral rows are counted across default and variant summaries", () => {
  const document = summarizeContractDocument(
    contractWith(
      {
        "function-1": {
          kind: "function",
          callbacks: [
            { parameter: 0, execution: "tracked" },
            { parameter: 1, execution: "deferred" }
          ],
          reactiveReads: [{ kind: "accessor", label: "memo result" }],
          returns: { kind: "accessor", label: "result" },
          ownerRequirements: [{ operation: "cleanup" }],
          asyncBehavior: "promise",
          variants: [
            {
              conditions: ["server"],
              summary: { kind: "function", callbacks: [{ parameter: 0, execution: "inline" }] }
            }
          ]
        }
      },
      { "function-1": ["createThing"] }
    )
  );

  assert.deepEqual(document.behavioralRows, {
    callbackExecution: 3,
    reactiveRead: 1,
    returnTree: 1,
    ownerRequirement: 1,
    asyncBehavior: 1
  });
  // Every declared row kind is present, so a new kind cannot be added to the
  // list without this measurement counting it.
  assert.deepEqual(Object.keys(document.behavioralRows).sort(), [...BEHAVIORAL_ROW_KINDS].sort());
});

test("an unknown domain contributes no behavioral row for that domain", () => {
  const document = summarizeContractDocument(
    contractWith({ "function-1": { kind: "function", callbacks: UNKNOWN, returns: UNKNOWN } }, { "function-1": ["x"] })
  );
  assert.equal(document.behavioralRows.callbackExecution, 0);
  assert.equal(document.behavioralRows.returnTree, 0);
});

test("a dangling summary id is a hole, counted as neither proven nor unknown", () => {
  const document = summarizeContractDocument(contractWith({}, { "function-missing": ["ghost"] }));
  assert.equal(document.exportsTotal, 1);
  assert.equal(document.exportsProven, 0);
  assert.equal(document.exportsWithUnknown, 0);
  assert.equal(document.exportsWithoutSummary, 1);
});

test("every entrypoint's exports are counted", () => {
  const document = summarizeContractDocument(
    contractWith(
      { "function-1": { kind: "function" }, "function-2": { kind: "function", callbacks: UNKNOWN } },
      { "function-1": ["a"] },
      { "./store": { exports: { "function-2": ["createStore", "produce"] } } }
    )
  );
  assert.equal(document.entrypointsEmitted, 2);
  assert.equal(document.exportsTotal, 3);
  assert.equal(document.exportsWithUnknown, 2);
});

test("a non-object contract is unmeasurable, never an empty measurement", () => {
  assert.equal(summarizeContractDocument(null), null);
  assert.equal(summarizeContractDocument("{}"), null);
});

// ---------------------------------------------------------------------------
// Reading the review plan
// ---------------------------------------------------------------------------

test("refused entrypoints come from the plan's refused-entrypoint items", () => {
  const plan = summarizeReviewPlan({
    schemaVersion: 1,
    items: [
      { id: "a", kind: "refused-entrypoint", target: { entrypoint: "./web" }, text: "./web: reason" },
      { id: "b", kind: "refused-entrypoint", target: { entrypoint: "./dom" }, text: "./dom: reason" },
      { id: "c", kind: "unknown-sentinel", target: { entrypoint: ".", export: "useQuery", field: "callbacks" } }
    ],
    generation: { entrypoints: {} }
  });

  assert.equal(plan.refusedEntrypoints, 2);
  assert.deepEqual(plan.refusedEntrypointNames, ["./dom", "./web"]);
  assert.equal(plan.checklistItems, 3);
  assert.deepEqual(plan.itemsByKind, { "refused-entrypoint": 2, "unknown-sentinel": 1 });
});

test("closure notes are flattened per entrypoint, sorted, counted, and sampled at three", () => {
  const plan = summarizeReviewPlan({
    schemaVersion: 1,
    items: [],
    generation: {
      entrypoints: {
        ".": { targets: ["./index.js"], modules: [], notes: ["b: unresolved", "a: unresolved"] },
        "./store": { targets: ["./store.js"], modules: [], notes: ["c: unresolved", "d: unresolved"] },
        "./web": { targets: ["./web.js"], modules: [] }
      }
    }
  });

  assert.equal(plan.closureNotes, 4);
  assert.deepEqual(plan.closureNoteSamples, [". a: unresolved", ". b: unresolved", "./store c: unresolved"]);
});

test("a plan with no notes anywhere reports zero closure notes", () => {
  const plan = summarizeReviewPlan({
    schemaVersion: 1,
    items: [],
    generation: { entrypoints: { ".": { targets: ["./index.js"], modules: [{ path: "index.js", hash: "abc" }] } } }
  });
  assert.equal(plan.closureNotes, 0);
  assert.deepEqual(plan.closureNoteSamples, []);
});

// ---------------------------------------------------------------------------
// The per-probe block
// ---------------------------------------------------------------------------

function cleanPlan(extra = {}) {
  return { schemaVersion: 1, items: [], generation: { entrypoints: { ".": { targets: [], modules: [] } } }, ...extra };
}

test("fullyProven requires no unknown, no refusal, and no closure note", () => {
  const clean = summarizeContract({
    contract: contractWith({ "function-1": { kind: "function" } }, { "function-1": ["a"] }),
    reviewPlan: cleanPlan(),
    refusedEntrypointsFromStdout: 0
  });
  assert.equal(clean.measured, true);
  assert.equal(clean.fullyProven, true);

  const withUnknown = summarizeContract({
    contract: contractWith({ "function-1": { kind: "function", callbacks: UNKNOWN } }, { "function-1": ["a"] }),
    reviewPlan: cleanPlan(),
    refusedEntrypointsFromStdout: 0
  });
  assert.equal(withUnknown.fullyProven, false);

  const withRefusal = summarizeContract({
    contract: contractWith({ "function-1": { kind: "function" } }, { "function-1": ["a"] }),
    reviewPlan: cleanPlan({
      items: [{ id: "a", kind: "refused-entrypoint", target: { entrypoint: "./web" }, text: "./web: reason" }]
    }),
    refusedEntrypointsFromStdout: 1
  });
  assert.equal(withRefusal.fullyProven, false);
  assert.equal(withRefusal.entrypointsRefused, 1);

  const withClosureNote = summarizeContract({
    contract: contractWith({ "function-1": { kind: "function" } }, { "function-1": ["a"] }),
    reviewPlan: {
      schemaVersion: 1,
      items: [],
      generation: { entrypoints: { ".": { targets: [], modules: [], notes: ["./x.js: unresolved specifier"] } } }
    },
    refusedEntrypointsFromStdout: 0
  });
  // Zero unknowns and zero refusals, and still not fully proven: the bytes the
  // contract describes were never fully enumerated.
  assert.equal(withClosureNote.exportsWithUnknown, 0);
  assert.equal(withClosureNote.entrypointsRefused, 0);
  assert.equal(withClosureNote.closureNotes, 1);
  assert.equal(withClosureNote.fullyProven, false);
});

test("a dangling summary blocks fullyProven even with no unknown sentinel", () => {
  const summary = summarizeContract({
    contract: contractWith({}, { "function-missing": ["ghost"] }),
    reviewPlan: cleanPlan(),
    refusedEntrypointsFromStdout: 0
  });
  assert.equal(summary.exportsWithUnknown, 0);
  assert.equal(summary.exportsWithoutSummary, 1);
  assert.equal(summary.fullyProven, false);
});

test("an unparsable contract is measured:false, never a row of zeroes", () => {
  const summary = summarizeContract({ contract: null, reviewPlan: cleanPlan() });
  assert.equal(summary.measured, false);
  assert.equal(summary.fullyProven, null);
  assert.equal(summary.exportsTotal, undefined);
  assert.match(summary.note, /contract document missing or unparsable/);
});

test("a missing review plan is named, falls back to stdout for refusals, and blocks fullyProven", () => {
  const summary = summarizeContract({
    contract: contractWith({ "function-1": { kind: "function" } }, { "function-1": ["a"] }),
    reviewPlan: null,
    refusedEntrypointsFromStdout: 0
  });
  assert.equal(summary.measured, true);
  assert.equal(summary.entrypointsRefused, 0);
  assert.equal(summary.closureNotes, null);
  // Closure notes are unknowable without the plan, so the strictest reading
  // cannot be granted.
  assert.equal(summary.fullyProven, false);
  assert.match(summary.note, /review plan missing or unparsable/);
});

test("stdout and review plan disagreeing about refusals is recorded, not resolved by preference", () => {
  const summary = summarizeContract({
    contract: contractWith({ "function-1": { kind: "function" } }, { "function-1": ["a"] }),
    reviewPlan: cleanPlan({
      items: [{ id: "a", kind: "refused-entrypoint", target: { entrypoint: "./web" }, text: "./web: reason" }]
    }),
    refusedEntrypointsFromStdout: 2
  });
  assert.deepEqual(summary.refusalDisagreement, { stdout: 2, reviewPlan: 1 });
});

// ---------------------------------------------------------------------------
// Reading from disk
// ---------------------------------------------------------------------------

test("reviewPlanPathFor matches the generator's sibling naming", () => {
  assert.equal(reviewPlanPathFor("/tmp/out/solid-reactivity.json"), "/tmp/out/solid-reactivity.review.json");
  assert.equal(reviewPlanPathFor("/tmp/out/contract"), "/tmp/out/contract.review.json");
});

test("readContractContent reads the contract and its sibling plan off disk", () => {
  const directory = mkdtempSync(join(tmpdir(), "solid-checker-content-test-"));
  try {
    const contractPath = join(directory, "solid-reactivity.json");
    writeFileSync(
      contractPath,
      JSON.stringify(
        contractWith({ "function-1": { kind: "function", callbacks: UNKNOWN } }, { "function-1": ["useQuery"] })
      )
    );
    writeFileSync(join(directory, "solid-reactivity.review.json"), JSON.stringify(cleanPlan()));

    const content = readContractContent(contractPath, 0);
    assert.equal(content.measured, true);
    assert.equal(content.exportsTotal, 1);
    assert.equal(content.unknownByDomain.callbacks, 1);
    assert.equal(content.closureNotes, 0);
    assert.equal(content.fullyProven, false);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("readContractContent on a path with no files is measured:false", () => {
  const content = readContractContent("/nonexistent/solid-reactivity.json", 0);
  assert.equal(content.measured, false);
});

// ---------------------------------------------------------------------------
// The wiring in run.mjs
// ---------------------------------------------------------------------------

test("runProbe reads contract content BEFORE cleanup removes the output directory", async () => {
  // The trap this guards: the read lives inside runProbe's `try`, and the
  // directory it reads from is deleted by the `finally` immediately after. Move
  // the read one block later and every probe silently reports `measured: false`
  // while the benchmark still exits 0 with a full report.
  const temporary = mkdtempSync(join(tmpdir(), "solid-checker-content-wiring-"));
  const outputDir = join(temporary, "out");
  const manifest = {
    schemaVersion: 1,
    rows: [
      {
        family: "solid-primitives",
        status: "official",
        package: "@solid-primitives/alpha",
        solidTarget: "solid1",
        version: "1.0.0",
        probes: [
          { id: "@solid-primitives/alpha@1.0.0|solid1|only", kind: "only", channel: "stable", solid: { "solid-js": "1.9.14" } }
        ]
      }
    ],
    supplemental: []
  };

  let cleanupRan = false;
  const hooks = {
    now: () => Date.now(),
    mkProject: async () => ({ projectDir: join(temporary, "project"), outputDir }),
    installPackages: async () => ({
      status: 0,
      stdout: "",
      stderr: "",
      timedOut: false,
      installedVersions: { "@solid-primitives/alpha": "1.0.0", "solid-js": "1.9.14" },
      integrity: {}
    }),
    generateContract: async ({ outputPath }) => {
      mkdirSync(dirname(outputPath), { recursive: true });
      writeFileSync(
        outputPath,
        JSON.stringify(
          contractWith(
            {
              "function-1": { kind: "function", callbacks: UNKNOWN },
              "function-2": { kind: "function", callbacks: [{ parameter: 0, execution: "inline" }] }
            },
            { "function-1": ["createThing"], "function-2": ["run"] }
          )
        )
      );
      writeFileSync(
        `${outputPath.slice(0, -5)}.review.json`,
        JSON.stringify({
          schemaVersion: 1,
          items: [],
          generation: { entrypoints: { ".": { targets: [], modules: [], notes: ["./x.js: unresolved specifier"] } } }
        })
      );
      return {
        status: 0,
        stdout: `generated pkg@1.0.0 contract with 1 entrypoints at ${outputPath}; review plan /tmp/plan.md (3 checklist items)`,
        stderr: "",
        timedOut: false
      };
    },
    cleanup: async ({ projectDir, outputDir: out }) => {
      cleanupRan = true;
      await rm(projectDir, { recursive: true, force: true });
      await rm(out, { recursive: true, force: true });
    }
  };

  try {
    const [result] = await runBenchmark({ manifest, hooks, options: { concurrency: 1 } });

    assert.equal(result.outcome, "success");
    assert.equal(cleanupRan, true);
    assert.equal(existsSync(outputDir), false, "the output directory must actually have been removed");
    // ...and the content was still measured, from bytes that no longer exist.
    assert.equal(result.contractContent.measured, true);
    assert.equal(result.contractContent.exportsTotal, 2);
    assert.equal(result.contractContent.exportsProven, 1);
    assert.equal(result.contractContent.unknownByDomain.callbacks, 1);
    assert.equal(result.contractContent.closureNotes, 1);
    assert.equal(result.contractContent.fullyProven, false);
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
});

test("a probe that never produced a contract carries no content block at all", async () => {
  const manifest = {
    schemaVersion: 1,
    rows: [
      {
        family: "solid-primitives",
        status: "official",
        package: "@solid-primitives/alpha",
        solidTarget: "solid1",
        version: "1.0.0",
        probes: [
          { id: "@solid-primitives/alpha@1.0.0|solid1|only", kind: "only", channel: "stable", solid: { "solid-js": "1.9.14" } }
        ]
      }
    ],
    supplemental: []
  };
  const hooks = {
    now: () => Date.now(),
    mkProject: async () => ({ projectDir: "/tmp/nonexistent-project", outputDir: "/tmp/nonexistent-out" }),
    installPackages: async () => ({
      status: 0,
      stdout: "",
      stderr: "",
      timedOut: false,
      installedVersions: { "@solid-primitives/alpha": "1.0.0", "solid-js": "1.9.14" },
      integrity: {}
    }),
    generateContract: async () => ({
      status: 2,
      stdout: "",
      stderr: "solid-checker: . has only a CJS runtime target; CJS contract generation is unsupported",
      timedOut: false
    }),
    cleanup: async () => {}
  };

  const [result] = await runBenchmark({ manifest, hooks, options: { concurrency: 1 } });
  assert.equal(result.outcome, "failure");
  // Null, not `{ measured: false }`: there was never a contract to measure, and
  // that is a different fact from one that could not be read.
  assert.equal(result.contractContent, null);
});

// ---------------------------------------------------------------------------
// Aggregation into the report
// ---------------------------------------------------------------------------

function contentBlock(overrides = {}) {
  return {
    measured: true,
    fullyProven: true,
    entrypointsEmitted: 1,
    entrypointsRefused: 0,
    refusedEntrypointNames: [],
    exportsTotal: 10,
    exportsProven: 10,
    exportsWithUnknown: 0,
    exportsAllDomainsUnknown: 0,
    exportsUnknownOnlyInVariants: 0,
    exportsWithoutSummary: 0,
    unknownByDomain: { callbacks: 0, reactiveReads: 0, returns: 0, ownerRequirements: 0, asyncBehavior: 0 },
    unknownTotal: 0,
    behavioralRows: { callbackExecution: 4, reactiveRead: 2, returnTree: 1, ownerRequirement: 0, asyncBehavior: 0 },
    closureNotes: 0,
    closureNoteSamples: [],
    reviewPlanItems: 12,
    reviewPlanItemsByKind: {},
    ...overrides
  };
}

function probeResult(overrides) {
  const outcome = overrides.outcome ?? "success";
  const solidTarget = overrides.solidTarget ?? "solid1";
  const probeKind = overrides.probeKind ?? "only";
  return {
    probeId: overrides.probeId ?? `${overrides.package}@1.0.0|${solidTarget}|${probeKind}`,
    family: overrides.family ?? "solid-primitives",
    status: "official",
    package: overrides.package,
    version: overrides.version ?? "1.0.0",
    solidTarget,
    probeKind,
    channel: "stable",
    solid: { "solid-js": "1.9.14" },
    installedVersions: {},
    integrityVerified: true,
    declaredEntrypoints: 1,
    generatedEntrypoints: outcome === "failure" ? null : 1,
    refusedEntrypoints: outcome === "partial-success" ? 1 : null,
    checklistItems: outcome === "failure" ? null : 10,
    contractContent: overrides.contractContent ?? (outcome === "failure" ? null : contentBlock()),
    outcome,
    class: outcome === "failure" ? "install-failure" : outcome,
    signature: "sig",
    detail: {},
    exitStatus: 0,
    timedOut: false,
    durationMs: 10,
    installDurationMs: 5,
    generationDurationMs: 4,
    stdout: "",
    stderr: ""
  };
}

function reportFor(results) {
  return buildReport({
    manifest: { schemaVersion: 1, generatedAt: "2026-08-22T00:00:00.000Z", rows: [], supplemental: [], limitations: [] },
    results,
    startedAt: "2026-08-22T09:00:00.000Z",
    finishedAt: "2026-08-22T09:01:00.000Z"
  });
}

test("aggregation sums contract content over contract-producing probes only", () => {
  const results = [
    probeResult({ package: "alpha" }),
    probeResult({
      package: "bravo",
      contractContent: contentBlock({
        fullyProven: false,
        exportsTotal: 20,
        exportsProven: 17,
        exportsWithUnknown: 3,
        unknownByDomain: { callbacks: 3, reactiveReads: 1, returns: 0, ownerRequirements: 0, asyncBehavior: 0 },
        unknownTotal: 4
      })
    }),
    probeResult({ package: "charlie", outcome: "failure" })
  ];

  const content = reportFor(results).combined.contractContent;
  assert.equal(content.probesMeasured, 2);
  assert.equal(content.probesFullyProven, 1);
  assert.equal(content.probesWithUnknowns, 1);
  assert.equal(content.exportsTotal, 30);
  assert.equal(content.exportsProven, 27);
  assert.equal(content.exportsProvenPercentage, 90);
  assert.equal(content.unknownTotal, 4);
  assert.deepEqual(content.unknownByDomain, {
    callbacks: 3,
    reactiveReads: 1,
    returns: 0,
    ownerRequirements: 0,
    asyncBehavior: 0
  });
  // The failed probe never wrote a contract, so it contributes nothing here —
  // not a zero-export row that would dilute the ratio.
  assert.deepEqual(content.unmeasuredProbes, []);
});

test("a partial contract's content is measured exactly like a complete one's", () => {
  const results = [
    probeResult({
      package: "alpha",
      outcome: "partial-success",
      contractContent: contentBlock({ fullyProven: false, entrypointsRefused: 2 })
    })
  ];
  const content = reportFor(results).combined.contractContent;
  assert.equal(content.probesMeasured, 1);
  assert.equal(content.probesWithRefusals, 1);
  assert.equal(content.entrypointsRefused, 2);
  assert.equal(content.probesFullyProven, 0);
});

test("a package is fully proven only when every one of its probes is", () => {
  const results = [
    probeResult({ package: "alpha", solidTarget: "solid1" }),
    probeResult({
      package: "alpha",
      solidTarget: "solid2",
      probeKind: "head",
      contractContent: contentBlock({ fullyProven: false, exportsWithUnknown: 1, unknownTotal: 1 })
    }),
    probeResult({ package: "bravo" })
  ];
  const content = reportFor(results).combined.contractContent;
  assert.equal(content.packagesMeasured, 2);
  assert.equal(content.packagesFullyProven, 1);
  assert.equal(content.packagesFullyProvenPercentage, 50);
  assert.equal(content.probesMeasured, 3);
  assert.equal(content.probesFullyProven, 2);
});

test("a contract-producing probe with no readable content is named, never counted clean", () => {
  const results = [
    probeResult({ package: "alpha" }),
    probeResult({
      package: "bravo",
      contractContent: { measured: false, fullyProven: null, note: "contract document missing or unparsable" }
    })
  ];
  const content = reportFor(results).combined.contractContent;
  assert.equal(content.probesMeasured, 1);
  assert.equal(content.probesFullyProven, 1);
  assert.deepEqual(content.unmeasuredProbes, [
    { probeId: "bravo@1.0.0|solid1|only", package: "bravo", note: "contract document missing or unparsable" }
  ]);
  assert.match(renderMarkdown(reportFor(results)), /Contracts that could not be read/);
});

test("per-family aggregation keeps every family and attributes each probe once", () => {
  const results = [
    probeResult({ package: "alpha", family: "solid-primitives" }),
    probeResult({
      package: "bravo",
      family: "tanstack",
      contractContent: contentBlock({
        fullyProven: false,
        exportsTotal: 57,
        exportsProven: 54,
        exportsWithUnknown: 3,
        unknownByDomain: { callbacks: 3, reactiveReads: 0, returns: 0, ownerRequirements: 0, asyncBehavior: 0 },
        unknownTotal: 3
      })
    })
  ];
  const content = reportFor(results).combined.contractContent;
  const primitives = content.families.find(family => family.family === "solid-primitives");
  const tanstack = content.families.find(family => family.family === "tanstack");

  assert.equal(content.families.length, 8);
  assert.equal(primitives.probesMeasured, 1);
  assert.equal(primitives.probesFullyProven, 1);
  assert.equal(tanstack.probesMeasured, 1);
  assert.equal(tanstack.unknownTotal, 3);
  assert.equal(tanstack.exportsProvenPercentage, 94.74);
  // A family with no probe reports "nothing measured", not 0% or 100%.
  const kobalte = content.families.find(family => family.family === "kobalte");
  assert.equal(kobalte.probesMeasured, 0);
  assert.equal(kobalte.exportsProvenPercentage, null);
});

test("topUnknownProbes ranks by absolute unknown count and names the dominant domain", () => {
  const results = [
    probeResult({ package: "alpha" }),
    probeResult({
      package: "bravo",
      contractContent: contentBlock({
        fullyProven: false,
        exportsWithUnknown: 2,
        unknownByDomain: { callbacks: 2, reactiveReads: 1, returns: 0, ownerRequirements: 0, asyncBehavior: 0 },
        unknownTotal: 3
      })
    }),
    probeResult({
      package: "charlie",
      contractContent: contentBlock({
        fullyProven: false,
        exportsWithUnknown: 9,
        unknownByDomain: { callbacks: 1, reactiveReads: 8, returns: 0, ownerRequirements: 0, asyncBehavior: 0 },
        unknownTotal: 9
      })
    })
  ];
  const content = reportFor(results).combined.contractContent;

  assert.deepEqual(
    content.topUnknownProbes.map(entry => [entry.package, entry.unknownTotal, entry.dominantDomain]),
    [
      ["charlie", 9, "reactiveReads"],
      ["bravo", 3, "callbacks"]
    ]
  );
  // A fully proven probe never appears in a list of unknowns.
  assert.equal(
    content.topUnknownProbes.some(entry => entry.package === "alpha"),
    false
  );
});

test("dominantDomain reports all-domains rather than picking one of five equal columns", () => {
  // The real @kobalte/core shape: every unknown export is unknown in all five
  // domains, so naming "callbacks" (the first column) as the cause would be an
  // artifact of column order, not a finding.
  const results = [
    probeResult({
      package: "kobalte-like",
      family: "kobalte",
      contractContent: contentBlock({
        fullyProven: false,
        exportsTotal: 610,
        exportsProven: 158,
        exportsWithUnknown: 452,
        exportsAllDomainsUnknown: 452,
        unknownByDomain: {
          callbacks: 452,
          reactiveReads: 452,
          returns: 452,
          ownerRequirements: 452,
          asyncBehavior: 452
        },
        unknownTotal: 2260
      })
    }),
    probeResult({
      package: "web-like",
      family: "official-solid",
      contractContent: contentBlock({
        fullyProven: false,
        exportsTotal: 382,
        exportsProven: 194,
        exportsWithUnknown: 188,
        exportsAllDomainsUnknown: 0,
        unknownByDomain: { callbacks: 0, reactiveReads: 188, returns: 0, ownerRequirements: 0, asyncBehavior: 0 },
        unknownTotal: 188
      })
    })
  ];
  const content = reportFor(results).combined.contractContent;

  assert.deepEqual(
    content.topUnknownProbes.map(entry => [entry.package, entry.dominantDomain]),
    [
      ["kobalte-like", "all-domains"],
      ["web-like", "reactiveReads"]
    ]
  );
  assert.equal(content.exportsAllDomainsUnknown, 452);
});

test("the markdown carries the headline numbers, the domain table, and the demand caveat", () => {
  const results = [
    probeResult({ package: "alpha" }),
    probeResult({
      package: "bravo",
      family: "tanstack",
      contractContent: contentBlock({
        fullyProven: false,
        exportsTotal: 20,
        exportsProven: 18,
        exportsWithUnknown: 2,
        unknownByDomain: { callbacks: 2, reactiveReads: 0, returns: 0, ownerRequirements: 0, asyncBehavior: 0 },
        unknownTotal: 2,
        exportsAllDomainsUnknown: 0,
        exportsUnknownOnlyInVariants: 1,
        closureNotes: 1,
        closureNoteSamples: [". ./x.js: unresolved specifier"]
      })
    })
  ];
  const markdown = renderMarkdown(reportFor(results));

  assert.match(markdown, /## Contract content \(what the emitted contracts claim\)/);
  assert.match(markdown, /Probes fully proven[^\n]*1\/2 \(50%\)/);
  assert.match(markdown, /Exports proven: 28\/30 \(93\.33%\)/);
  assert.match(markdown, /Closure notes \(block byte-attested verification\): 1/);
  assert.match(markdown, /\| callbacks \| 2 \|/);
  assert.match(markdown, /0 unknown in ALL five domains/);
  assert.match(markdown, /1 unknown only inside a conditional variant/);
  assert.match(markdown, /### Positive behavioral rows/);
  assert.match(markdown, /### Contract content by family/);
  // The caveat is not optional prose: without it the ratio reads as a claim
  // about the ecosystem rather than about an unreviewed generated draft.
  assert.match(markdown, /GENERATED DRAFT, not consumer findings/);
  assert.match(markdown, /becomes a finding only when a consumer actually touches that surface/);
});

test("a report whose probes carry no content block renders the section as unmeasured", () => {
  // Guards the additive contract: an older caller (or a test fixture) that
  // never populated contractContent must still build and render a report.
  const results = [{ ...probeResult({ package: "alpha" }), contractContent: undefined }];
  const report = reportFor(results);
  assert.equal(report.combined.contractContent.probesMeasured, 0);
  assert.equal(report.combined.contractContent.packagesFullyProvenPercentage, null);
  assert.match(renderMarkdown(report), /No contract content measured\./);
});
