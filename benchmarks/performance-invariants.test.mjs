import assert from "node:assert/strict";
import { test } from "vitest";

import {
  DEFAULT_CEILINGS,
  assessPerformance,
  renderPerformanceFindings
} from "./lib/performance-invariants.mjs";

function report({ sources, exportNs, responseBytes, cachedIrNs, firstIrNs, incrementalNs }) {
  return {
    sourceCount: sources,
    medianNs: incrementalNs,
    firstAnalysisBreakdown: { responseBytes: { medianNs: responseBytes * sources } },
    firstRustPipelineBreakdown: {
      reactiveIrTotal: { medianNs: firstIrNs * sources },
      reactiveIr: { interproceduralExportSummaries: { medianNs: exportNs } }
    },
    rustPipelineBreakdown: { reactiveIrTotal: { medianNs: cachedIrNs } }
  };
}

const healthy = {
  small: report({ sources: 500, exportNs: 1_000_000, responseBytes: 892, cachedIrNs: 3000, firstIrNs: 60_000, incrementalNs: 26_000_000 }),
  largeSamples: [
    report({ sources: 1000, exportNs: 1_900_000, responseBytes: 892, cachedIrNs: 3000, firstIrNs: 60_000, incrementalNs: 26_000_000 }),
    report({ sources: 1000, exportNs: 1_900_000, responseBytes: 892, cachedIrNs: 3000, firstIrNs: 61_000, incrementalNs: 26_000_000 }),
    report({ sources: 1000, exportNs: 1_900_000, responseBytes: 892, cachedIrNs: 3000, firstIrNs: 59_000, incrementalNs: 26_000_000 })
  ],
  incremental: report({ sources: 1000, exportNs: 1_900_000, responseBytes: 892, cachedIrNs: 3000, firstIrNs: 60_000, incrementalNs: 26_000_000 })
};

test("a healthy run passes every invariant under both gates", () => {
  for (const wallTimeGate of ["enforce", "report"]) {
    const assessment = assessPerformance(healthy, { wallTimeGate });
    assert.equal(assessment.ok, true);
    assert.deepEqual(assessment.findings.map(finding => finding.status), ["ok", "ok", "ok", "ok", "ok"]);
    assert.match(assessment.summary, /export scaling 1\.90x/);
    assert.match(assessment.summary, /best first IR 59000 ns\/source/);
  }
});

test("structural invariants are enforced regardless of the wall-time gate", () => {
  const slowScaling = {
    ...healthy,
    largeSamples: healthy.largeSamples.map(sample => ({
      ...sample,
      firstRustPipelineBreakdown: {
        ...sample.firstRustPipelineBreakdown,
        reactiveIr: { interproceduralExportSummaries: { medianNs: 3_000_000 } }
      }
    }))
  };
  const assessment = assessPerformance(slowScaling, { wallTimeGate: "report" });
  assert.equal(assessment.ok, false);
  assert.deepEqual(assessment.violations.map(finding => finding.id), ["export-scaling"]);
  assert.match(assessment.violations[0].message, /scales 3\.00x/);
});

test("a shared-runner slowdown is reported, not failed, when wall time is not enforced", () => {
  const slowRunner = {
    ...healthy,
    largeSamples: healthy.largeSamples.map(sample => ({
      ...sample,
      firstRustPipelineBreakdown: { ...sample.firstRustPipelineBreakdown, reactiveIrTotal: { medianNs: 240_000 * 1000 } }
    })),
    incremental: { ...healthy.incremental, medianNs: 450_000_007 }
  };
  const reported = assessPerformance(slowRunner, { wallTimeGate: "report" });
  assert.equal(reported.ok, true, "the run passes");
  assert.deepEqual(reported.reported.map(finding => finding.id), ["first-ir", "incremental"]);
  const enforced = assessPerformance(slowRunner, { wallTimeGate: "enforce" });
  assert.equal(enforced.ok, false);
  assert.deepEqual(enforced.violations.map(finding => finding.id), ["first-ir", "incremental"]);
  assert.match(enforced.violations[1].message, /450000007 ns \(limit 400000000 ns\)/);
});

test("first IR uses the best of the independent cold samples", () => {
  const oneSlowSample = {
    ...healthy,
    largeSamples: [
      { ...healthy.largeSamples[0], firstRustPipelineBreakdown: { ...healthy.largeSamples[0].firstRustPipelineBreakdown, reactiveIrTotal: { medianNs: 300_000 * 1000 } } },
      healthy.largeSamples[1],
      healthy.largeSamples[2]
    ]
  };
  const assessment = assessPerformance(oneSlowSample, { wallTimeGate: "enforce" });
  assert.equal(assessment.ok, true);
  assert.deepEqual(assessment.findings[3].samples.map(Math.round), [300000, 61000, 59000]);
});

test("export scaling uses the best cold sample on both sides", () => {
  const exportAt = (report, ns) => ({
    ...report,
    firstRustPipelineBreakdown: {
      ...report.firstRustPipelineBreakdown,
      reactiveIr: { ...report.firstRustPipelineBreakdown.reactiveIr, interproceduralExportSummaries: { medianNs: ns } }
    }
  });
  // Scheduling jitter only slows samples: one slow small and one slow large
  // sample. The best of each side is 120 us vs 260 us, a 2.17x ratio.
  const jitter = {
    ...healthy,
    smallSamples: [exportAt(healthy.small, 180_000), exportAt(healthy.small, 120_000), exportAt(healthy.small, 125_000)],
    largeSamples: [exportAt(healthy.largeSamples[0], 400_000), exportAt(healthy.largeSamples[1], 260_000), exportAt(healthy.largeSamples[2], 270_000)]
  };
  assert.equal(assessPerformance(jitter, { wallTimeGate: "enforce" }).ok, true);
  // Every large sample slow is a real super-linear cost.
  const superLinear = { ...jitter, largeSamples: jitter.largeSamples.map(sample => exportAt(sample, 400_000)) };
  const assessment = assessPerformance(superLinear, { wallTimeGate: "enforce" });
  assert.equal(assessment.ok, false);
  assert.match(assessment.violations[0].message, /export aggregation scales 3\.\d+x/);
});

test("ceilings can be overridden and unknown gates are refused", () => {
  const strict = assessPerformance(healthy, { ceilings: { ...DEFAULT_CEILINGS, incrementalNs: 1 } });
  assert.deepEqual(strict.violations.map(finding => finding.id), ["incremental"]);
  assert.throws(() => assessPerformance(healthy, { wallTimeGate: "maybe" }), /wall-time gate/);
});

test("the Markdown rendering lists every invariant and explains the reporting mode", () => {
  const markdown = renderPerformanceFindings(assessPerformance(healthy, { wallTimeGate: "report" }), { wallTimeGate: "report" });
  assert.match(markdown, /\| export-scaling \| structural \| 1\.90 x \| 2\.8 x \| ok \|/);
  assert.match(markdown, /\| incremental \| wall-time \| 26000000 ns \| 400000000 ns \| ok \|/);
  assert.match(markdown, /reported, not enforced/);
});
