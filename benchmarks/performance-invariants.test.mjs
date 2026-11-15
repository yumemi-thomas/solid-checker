import assert from "node:assert/strict";
import { test } from "vitest";

import {
  COLD_SAMPLES,
  DEFAULT_CEILINGS,
  FIRST_IR_MAX_SAMPLES,
  CORPUS_PATH_CHARS,
  FIRST_IR_CORPUS,
  assessPerformance,
  corpusRootPrefix,
  quietLoadBound,
  renderPerformanceFindings,
  sampleFirstIr,
  spreadOf
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
  const noneLarge = { ...healthy, exportLargeSamples: exportWorkAt(healthy.exportLargeSamples, 0) };
  assert.deepEqual(assessPerformance(noneLarge).violations.map(finding => finding.id), ["export-scaling"]);
  assert.match(assessPerformance(noneLarge).violations[0].message, /counted no work/);
  const fractional = { ...healthy, exportLargeSamples: exportWorkAt(healthy.exportLargeSamples, 23_000.5) };
  assert.deepEqual(assessPerformance(fractional).violations.map(finding => finding.id), ["export-scaling"]);
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

// The first-IR sampler (ADR 0986): `values` are ns/source, one per cold process.
const firstIrReport = value => report({ sources: 1000, exportNs: 1_900_000, responseBytes: 892, cachedIrNs: 3000, firstIrNs: value, incrementalNs: 26_000_000 });
const sampler = (values, { load = () => 1, waitMs = 30_000, clock = { at: 0 } } = {}) => {
  const queue = [...values];
  const slept = [];
  const result = sampleFirstIr({
    measureOne: () => firstIrReport(queue.shift()),
    limit: DEFAULT_CEILINGS.firstIrNsPerSource,
    maxSamples: FIRST_IR_MAX_SAMPLES,
    bound: 3.5,
    waitMs,
    load,
    now: () => clock.at,
    sleep: milliseconds => { slept.push(milliseconds); clock.at += milliseconds; }
  });
  return { ...result, slept, taken: values.length - queue.length };
};
const nsOf = reports => reports.map(r => r.firstRustPipelineBreakdown.reactiveIrTotal.medianNs / r.sourceCount);

test("the first-IR sampler stops at the first sample under the ceiling, and its decision is the minimum of all", () => {
  const slow = Array(FIRST_IR_MAX_SAMPLES).fill(180_000);
  const lucky = sampler([181_000, 176_000, 174_900, ...slow]);
  assert.equal(lucky.taken, 3);
  assert.equal(lucky.run.stoppedEarly, true);
  const assessment = assessPerformance({ ...healthy, largeSamples: lucky.reports, firstIrRun: lucky.run });
  assert.equal(assessment.ok, true);
  // The same samples drawn in full give the same minimum, so stopping early cannot change a decision.
  assert.equal(Math.min(...nsOf(lucky.reports)), 174_900);
  assert.equal(Math.min(...nsOf([...lucky.reports, ...slow.map(firstIrReport)])), 174_900);
});

test("a regression raises every sample, so it fails after all of them, whatever the load", () => {
  for (const load of [0.5, 40]) {
    const regressed = sampler(Array(FIRST_IR_MAX_SAMPLES).fill(175_100), { load: () => load });
    assert.equal(regressed.taken, FIRST_IR_MAX_SAMPLES);
    const assessment = assessPerformance({ ...healthy, largeSamples: regressed.reports, firstIrRun: regressed.run });
    assert.equal(assessment.ok, false);
    assert.deepEqual(assessment.violations.map(finding => finding.id), ["first-ir"]);
    assert.match(assessment.violations[0].message, /all 40 samples were over the ceiling/);
  }
  const busy = sampler(Array(FIRST_IR_MAX_SAMPLES).fill(190_000), { load: () => 20 });
  const message = assessPerformance({ ...healthy, largeSamples: busy.reports, firstIrRun: busy.run }).violations[0].message;
  assert.match(message, /1-minute load 20\.0 at the first sample/);
  assert.match(message, /not reached/);
  assert.match(message, /rerun it when it is/);
});

test("the sampler waits for a quiet machine up to the wait, then measures anyway", () => {
  const readings = [9, 8, 7, 3, 3, 3, 3, 3];
  const falling = sampler([170_000], { load: () => readings.length > 1 ? readings.shift() : readings[0] });
  assert.deepEqual(falling.slept, [1000, 1000]);
  assert.equal(falling.run.quiet, true);
  assert.equal(falling.taken, 1);
  const stuck = sampler([170_000], { load: () => 12 });
  assert.equal(stuck.slept.length, 30);
  assert.equal(stuck.run.waitedMs, 30_000);
  assert.equal(stuck.run.quiet, false);
  assert.equal(stuck.taken, 1, "a machine that never quiets is still measured, never skipped or passed");
  const quiet = sampler([170_000], { load: () => 0.5 });
  assert.deepEqual(quiet.slept, []);
});

test("the quiet bound is a quarter of the cores, at least 2", () => {
  assert.equal(quietLoadBound(14), 3.5);
  assert.equal(quietLoadBound(4), 2);
});

test("spreadOf gives the order statistics the failure message reports", () => {
  assert.deepEqual(spreadOf([5, 1, 3, 2, 4]), { n: 5, min: 1, p25: 2, median: 3, p75: 4, max: 5 });
});

test("the first-IR result names the corpus path length", () => {
  const sampled = sampler([170_000]);
  sampled.run.corpusPathChars = 93;
  assert.match(assessPerformance({ ...healthy, largeSamples: sampled.reports, firstIrRun: sampled.run }).summary, /corpus path 93 characters/);
});

test("the first-IR corpus path is 93 characters whatever the temporary directory is (ADR 0995)", () => {
  // The laundering: the corpus lived under $TMPDIR, so the same binary read 93 characters on this Mac and 51 on a Linux runner, and a
  // number measured at one was compared with the limit set at the other (ADR 0986: ~115-190 ns/source per character).
  for (const tmp of ["/tmp", "/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T", "/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/", "/Users/x/dev/sc-scratch.noindex/mac18/tmp"]) {
    const root = `${corpusRootPrefix(tmp)}ABCDEF`; // mkdtemp adds six characters
    assert.equal(`${root}/${FIRST_IR_CORPUS}`.length, CORPUS_PATH_CHARS, tmp);
    assert.ok(root.startsWith(`${tmp.replace(/\/+$/, "")}/solid-checker-performance-`), tmp);
  }
  assert.equal(CORPUS_PATH_CHARS, 93);
  assert.equal(FIRST_IR_CORPUS, "widget-1000");
  // A temporary directory too long for 93 is refused, with the limit, never padded past it or silently measured at another length.
  assert.throws(() => corpusRootPrefix(`/${"d".repeat(70)}`), /point TMPDIR at a shorter directory/);
});
