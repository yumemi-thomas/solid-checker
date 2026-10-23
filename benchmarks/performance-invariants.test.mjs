import assert from "node:assert/strict";
import { test } from "vitest";

import {
  COLD_SAMPLES,
  DEFAULT_CEILINGS,
  assessPerformance,
  renderPerformanceFindings
} from "./lib/performance-invariants.mjs";

function report({ sources, exportNs, exportWork = sources * 23, responseBytes, cachedIrNs, firstIrNs, incrementalNs }) {
  return {
    sourceCount: sources,
    medianNs: incrementalNs,
    firstAnalysisBreakdown: { responseBytes: { medianNs: responseBytes * sources } },
    firstRustPipelineBreakdown: {
      reactiveIrTotal: { medianNs: firstIrNs * sources },
      interproceduralExportWork: { medianNs: exportWork },
      reactiveIr: { interproceduralExportSummaries: { medianNs: exportNs } }
    },
    rustPipelineBreakdown: { reactiveIrTotal: { medianNs: cachedIrNs } }
  };
}

const healthy = {
  small: report({ sources: 500, responseBytes: 892, cachedIrNs: 3000, firstIrNs: 60_000, incrementalNs: 26_000_000 }),
  exportSmallSamples: [report({ sources: 500, exportNs: 1_000_000, responseBytes: 892, cachedIrNs: 3000, firstIrNs: 60_000, incrementalNs: 26_000_000 })],
  exportLargeSamples: [report({ sources: 1000, exportNs: 1_900_000, responseBytes: 892, cachedIrNs: 3000, firstIrNs: 60_000, incrementalNs: 26_000_000 })],
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
    assert.deepEqual(assessment.findings.map(finding => finding.status), ["ok", "ok", "ok", "ok", "ok", "ok"]);
    assert.match(assessment.summary, /export work 2\.00x, export time 1\.90x/);
    assert.match(assessment.summary, /best first IR 59000 ns\/source/);
  }
});

const withExport = (samples, change) => samples.map(sample => ({
  ...sample,
  firstRustPipelineBreakdown: { ...sample.firstRustPipelineBreakdown, ...change(sample.firstRustPipelineBreakdown) }
}));
const exportAt = (samples, ns) =>
  withExport(samples, breakdown => ({ reactiveIr: { ...breakdown.reactiveIr, interproceduralExportSummaries: { medianNs: ns } } }));
const exportWorkAt = (samples, units) => withExport(samples, () => ({ interproceduralExportWork: { medianNs: units } }));

test("structural invariants are enforced regardless of the wall-time gate", () => {
  // More than twice the work for twice the files: algorithmic super-linearity.
  const superLinearWork = { ...healthy, exportLargeSamples: exportWorkAt(healthy.exportLargeSamples, 500 * 23 * 3) };
  const assessment = assessPerformance(superLinearWork, { wallTimeGate: "report" });
  assert.equal(assessment.ok, false);
  assert.deepEqual(assessment.violations.map(finding => finding.id), ["export-scaling"]);
  assert.match(assessment.violations[0].message, /does 3\.00x the work/);
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
  assert.deepEqual(assessment.findings[4].samples.map(Math.round), [300000, 61000, 59000]);
});

test("the work counter ignores timing noise entirely", () => {
  // The same count with wall times 10x apart in both directions: the gate on
  // work passes; only the looser wall-time bound can react to time.
  const noisy = {
    ...healthy,
    exportSmallSamples: exportAt(healthy.exportSmallSamples, 100_000),
    exportLargeSamples: exportAt(healthy.exportLargeSamples, 150_000)
  };
  assert.equal(assessPerformance(noisy, { wallTimeGate: "enforce" }).ok, true);
});

test("export wall time uses the best cold sample on both sides", () => {
  const sampleAt = (samples, ns) => exportAt(samples, ns)[0];
  // Scheduling jitter only slows samples: one slow small and one slow large
  // sample. The best of each side is 1.0 ms vs 2.0 ms, a 2.00x ratio.
  const jitter = {
    ...healthy,
    exportSmallSamples: [sampleAt(healthy.exportSmallSamples, 1_800_000), sampleAt(healthy.exportSmallSamples, 1_000_000), sampleAt(healthy.exportSmallSamples, 1_050_000)],
    exportLargeSamples: [sampleAt(healthy.exportLargeSamples, 4_000_000), sampleAt(healthy.exportLargeSamples, 2_000_000), sampleAt(healthy.exportLargeSamples, 2_100_000)]
  };
  assert.equal(assessPerformance(jitter, { wallTimeGate: "enforce" }).ok, true);
  // Every large sample slow is a real super-linear cost the counter did not see.
  const superLinear = { ...jitter, exportLargeSamples: jitter.exportLargeSamples.map(sample => sampleAt([sample], 4_000_000)) };
  const assessment = assessPerformance(superLinear, { wallTimeGate: "report" });
  assert.equal(assessment.ok, false);
  assert.deepEqual(assessment.violations.map(finding => finding.id), ["export-time-scaling"]);
  assert.match(assessment.violations[0].message, /scales 4\.00x in time/);
});

test("a gate that counted no work, or counted inconsistently, fails instead of passing", () => {
  const none = { ...healthy, exportSmallSamples: exportWorkAt(healthy.exportSmallSamples, 0) };
  assert.deepEqual(assessPerformance(none).violations.map(finding => finding.id), ["export-scaling"]);
  assert.match(assessPerformance(none).violations[0].message, /counted no work/);
  const [one] = healthy.exportLargeSamples;
  const disagree = { ...healthy, exportLargeSamples: [...exportWorkAt([one], 23_000), ...exportWorkAt([one], 23_001)] };
  assert.deepEqual(assessPerformance(disagree).violations.map(finding => finding.id), ["export-scaling"]);
});

test("the wall-time ratios are taken over enough cold samples to ride out scheduling noise", () => {
  assert.ok(COLD_SAMPLES >= 9);
});

test("ceilings can be overridden and unknown gates are refused", () => {
  const strict = assessPerformance(healthy, { ceilings: { ...DEFAULT_CEILINGS, incrementalNs: 1 } });
  assert.deepEqual(strict.violations.map(finding => finding.id), ["incremental"]);
  assert.throws(() => assessPerformance(healthy, { wallTimeGate: "maybe" }), /wall-time gate/);
});

test("the Markdown rendering lists every invariant and explains the reporting mode", () => {
  const markdown = renderPerformanceFindings(assessPerformance(healthy, { wallTimeGate: "report" }), { wallTimeGate: "report" });
  assert.match(markdown, /\| export-scaling \| structural \| 2 x \| 2\.2 x \| ok \|/);
  assert.match(markdown, /\| incremental \| wall-time \| 26000000 ns \| 400000000 ns \| ok \|/);
  assert.match(markdown, /reported, not enforced/);
});
