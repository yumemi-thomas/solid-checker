// The performance invariants, as a pure function over benchmark reports.
//
// Two kinds of invariant live here and they are gated differently:
//
// - Structural invariants — export aggregation scaling, Type Facts payload per
//   source, and cached-result reuse — are properties of the analyzer, not of
//   the machine. They hold the same on a laptop and on a shared runner and are
//   always enforced.
// - Wall-time ceilings — fresh Reactive IR per source and the one-file
//   incremental edit — measure the machine as much as the analyzer. On GitHub's
//   shared ubuntu-24.04 runners the same commit measured 65-106 ms for the
//   incremental edit and 154-246 us/source for first IR across ten consecutive
//   runs (September 2026): a 1.6x swing with no code change. No fixed ceiling
//   can sit inside that band and still catch regressions, so on shared runners
//   they are *reported*, and the interleaved base-versus-head race in
//   compare-performance.mjs (whose ratios stayed within 0.98-1.04 over the same
//   runs) is the regression gate. On a machine you control — `make
//   verify-performance`, `scripts/verify.sh` — they are enforced.

export const DEFAULT_CEILINGS = Object.freeze({
  // Deterministic work units of export aggregation when the corpus doubles.
  // The count is exactly linear on unchanged code (2.00x on the structured
  // corpus), so 2.2x leaves room only for a corpus-shape change, never noise.
  exportWorkScaling: 2.2,
  // Wall time of the same phase, a sanity bound for cost the work counter
  // does not see (an uncounted inner scan). A quadratic doubles to 4x; the
  // best-of-nine ratio on the structured corpus measured 1.81-2.02x over five
  // sets on a loaded machine (the phase takes 1-2 ms there). The widget
  // corpus's 0.1-0.6 ms phase spread 2.2-3.4x, which is why it is not used.
  exportTimeScaling: 3.0,
  responseBytesPerSource: 1000,
  cachedIrNs: 50_000,
  firstIrNsPerSource: 175_000,
  // 400 ms: ADR 0364 turned retention across updates off, so a one-file edit
  // recomputes all 1000 files. Median of 11 runs on the generated corpus:
  // 212 ms (207-223), up from 48 ms (47-52) with retention; the ceiling is
  // about 1.9x the new median. It was 100 ms. Lower it when retention returns.
  incrementalNs: 400_000_000
});

// Cold processes per corpus size. First-IR cost and the export-scaling wall
// time both use the minimum over this many: scheduling slows some samples, a
// real regression slows every one.
export const COLD_SAMPLES = 9;

// The export phase needs a corpus that has something to aggregate. Sizes
// (files) of the "structured" corpus the export-scaling invariant measures.
export const EXPORT_CORPUS_FILES = Object.freeze({ small: 500, large: 1000 });

export const WALL_TIME_GATES = Object.freeze(["enforce", "report"]);

function exportTime(report) {
  return report.firstRustPipelineBreakdown.reactiveIr.interproceduralExportSummaries.medianNs;
}

// Work units the phase counted (node visits per discovery round, leaf
// evaluations, exported properties examined). Identical on every run.
function exportWork(report) {
  return report.firstRustPipelineBreakdown.interproceduralExportWork.medianNs;
}

/**
 * Evaluates every invariant. `wallTimeGate` is "enforce" (a breached ceiling is
 * a violation) or "report" (it is recorded with status "reported" and never
 * fails the run). Returns the findings, whether any violation exists, and a
 * one-line summary.
 */
export function assessPerformance(
  { small, largeSamples, exportSmallSamples, exportLargeSamples, incremental },
  { wallTimeGate = "enforce", ceilings = DEFAULT_CEILINGS } = {}
) {
  if (!WALL_TIME_GATES.includes(wallTimeGate)) {
    throw new Error(`wall-time gate must be one of ${WALL_TIME_GATES.join(", ")}, got ${JSON.stringify(wallTimeGate)}`);
  }
  // Export aggregation is measured on the structured corpus, where it takes
  // milliseconds, in two ways. The work counter is the gate: it is a count,
  // so noise cannot move it, and it fails on algorithmic super-linearity
  // exactly. The best-of-N wall-time ratio backs it for cost the counter does
  // not see. Every sample of a size must report the same count; a differing
  // count is a nondeterministic analysis, not noise.
  const workOf = samples => {
    const counts = [...new Set(samples.map(exportWork))];
    return counts.length === 1 && Number.isSafeInteger(counts[0]) && counts[0] > 0 ? counts[0] : Number.NaN;
  };
  // A side that counted nothing (or disagreed) is NaN, and NaN fails.
  const smallWork = workOf(exportSmallSamples);
  const largeWork = workOf(exportLargeSamples);
  const workScaling = largeWork / smallWork;
  const timeScaling =
    Math.min(...exportLargeSamples.map(exportTime)) / Math.max(Math.min(...exportSmallSamples.map(exportTime)), 1);
  const responseBytes = small.firstAnalysisBreakdown.responseBytes.medianNs / small.sourceCount;
  const cachedIr = small.rustPipelineBreakdown.reactiveIrTotal.medianNs;
  const firstIrSamples = largeSamples.map(
    report => report.firstRustPipelineBreakdown.reactiveIrTotal.medianNs / report.sourceCount
  );
  // Best of the independent cold processes: shared-runner scheduling can slow
  // one sample, a real regression slows every one.
  const firstIrPerSource = Math.min(...firstIrSamples);
  const incrementalNs = incremental.medianNs;

  const structural = (id, value, limit, unit, describe) => ({
    id,
    kind: "structural",
    value,
    limit,
    unit,
    // NaN (no work counted, or samples that disagree) is a violation: the
    // comparison is written so that it cannot pass.
    status: value <= limit ? "ok" : "violation",
    message: describe(value, limit)
  });
  const wallTime = (id, value, limit, unit, describe, extra = {}) => ({
    id,
    kind: "wall-time",
    value,
    limit,
    unit,
    status: value > limit ? (wallTimeGate === "enforce" ? "violation" : "reported") : "ok",
    message: describe(value, limit),
    ...extra
  });

  const findings = [
    structural("export-scaling", workScaling, ceilings.exportWorkScaling, "x", (value, limit) =>
      Number.isNaN(value)
        ? `package contract export aggregation counted no work or inconsistent work (${smallWork} units at ${EXPORT_CORPUS_FILES.small} files, ${largeWork} at ${EXPORT_CORPUS_FILES.large})`
        : `package contract export aggregation does ${value.toFixed(2)}x the work when the corpus doubles (limit ${limit}x; ${smallWork} to ${largeWork} units)`
    ),
    structural("export-time-scaling", timeScaling, ceilings.exportTimeScaling, "x", (value, limit) =>
      `package contract export aggregation scales ${value.toFixed(2)}x in time when the corpus doubles (limit ${limit}x)`
    ),
    structural("type-facts-response-bytes", responseBytes, ceilings.responseBytesPerSource, "bytes/source", (value, limit) =>
      `first Type Facts response uses ${value.toFixed(0)} bytes/source (limit ${limit})`
    ),
    structural("cached-ir", cachedIr, ceilings.cachedIrNs, "ns", (value, limit) =>
      `cached Reactive IR analysis takes ${value} ns (limit ${limit} ns for shared-result reuse)`
    ),
    wallTime("first-ir", firstIrPerSource, ceilings.firstIrNsPerSource, "ns/source", (value, limit) =>
      `best first Reactive IR analysis uses ${value.toFixed(0)} ns/source (limit ${limit}; samples ${firstIrSamples.map(sample => sample.toFixed(0)).join(", ")})`,
      { samples: firstIrSamples }
    ),
    wallTime("incremental", incrementalNs, ceilings.incrementalNs, "ns", (value, limit) =>
      `one-file incremental analysis takes ${value} ns (limit ${limit} ns)`
    )
  ];
  const violations = findings.filter(finding => finding.status === "violation");
  const reported = findings.filter(finding => finding.status === "reported");
  return {
    findings,
    ok: violations.length === 0,
    violations,
    reported,
    summary:
      `export work ${workScaling.toFixed(2)}x, export time ${timeScaling.toFixed(2)}x, Type Facts ${responseBytes.toFixed(0)} bytes/source, ` +
      `cached IR ${cachedIr} ns, best first IR ${firstIrPerSource.toFixed(0)} ns/source ` +
      `(${firstIrSamples.map(sample => sample.toFixed(0)).join(", ")}), incremental ${incrementalNs} ns`
  };
}

/** A Markdown table of the findings, for a job summary or a report. */
export function renderPerformanceFindings(assessment, { wallTimeGate }) {
  const lines = [
    "| Invariant | Kind | Measured | Limit | Status |",
    "| --- | --- | --- | --- | --- |"
  ];
  for (const finding of assessment.findings) {
    const measured = Number.isInteger(finding.value) ? String(finding.value) : finding.value.toFixed(finding.unit === "x" ? 2 : 0);
    lines.push(`| ${finding.id} | ${finding.kind} | ${measured} ${finding.unit} | ${finding.limit} ${finding.unit} | ${finding.status} |`);
  }
  if (wallTimeGate === "report") {
    lines.push("");
    lines.push(
      "Wall-time ceilings are reported, not enforced, on shared runners; the interleaved base-versus-head comparison is the regression gate."
    );
  }
  return `${lines.join("\n")}\n`;
}
