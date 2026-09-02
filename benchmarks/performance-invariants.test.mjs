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
    incremental: { ...healthy.incremental, medianNs: 105_790_007 }
  };
  const reported = assessPerformance(slowRunner, { wallTimeGate: "report" });
  assert.equal(reported.ok, true, "the run passes");
  assert.deepEqual(reported.reported.map(finding => finding.id), ["first-ir", "incremental"]);
  const enforced = assessPerformance(slowRunner, { wallTimeGate: "enforce" });
  assert.equal(enforced.ok, false);
  assert.deepEqual(enforced.violations.map(finding => finding.id), ["first-ir", "incremental"]);
  assert.match(enforced.violations[1].message, /105790007 ns \(limit 100000000 ns\)/);
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

test("ceilings can be overridden and unknown gates are refused", () => {
  const strict = assessPerformance(healthy, { ceilings: { ...DEFAULT_CEILINGS, incrementalNs: 1 } });
  assert.deepEqual(strict.violations.map(finding => finding.id), ["incremental"]);
  assert.throws(() => assessPerformance(healthy, { wallTimeGate: "maybe" }), /wall-time gate/);
});

test("the Markdown rendering lists every invariant and explains the reporting mode", () => {
  const markdown = renderPerformanceFindings(assessPerformance(healthy, { wallTimeGate: "report" }), { wallTimeGate: "report" });
  assert.match(markdown, /\| export-scaling \| structural \| 1\.90 x \| 2\.8 x \| ok \|/);
  assert.match(markdown, /\| incremental \| wall-time \| 26000000 ns \| 100000000 ns \| ok \|/);
  assert.match(markdown, /reported, not enforced/);
});
