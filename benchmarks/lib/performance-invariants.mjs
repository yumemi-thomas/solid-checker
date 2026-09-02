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
  exportScaling: 2.8,
  responseBytesPerSource: 1000,
  cachedIrNs: 50_000,
  firstIrNsPerSource: 175_000,
  incrementalNs: 100_000_000
});

export const WALL_TIME_GATES = Object.freeze(["enforce", "report"]);

function exportTime(report) {
  return report.firstRustPipelineBreakdown.reactiveIr.interproceduralExportSummaries.medianNs;
}

/**
 * Evaluates every invariant. `wallTimeGate` is "enforce" (a breached ceiling is
 * a violation) or "report" (it is recorded with status "reported" and never
 * fails the run). Returns the findings, whether any violation exists, and a
 * one-line summary.
 */
export function assessPerformance(
  { small, largeSamples, incremental },
  { wallTimeGate = "enforce", ceilings = DEFAULT_CEILINGS } = {}
) {
  if (!WALL_TIME_GATES.includes(wallTimeGate)) {
    throw new Error(`wall-time gate must be one of ${WALL_TIME_GATES.join(", ")}, got ${JSON.stringify(wallTimeGate)}`);
  }
  const large = largeSamples[0];
  const scaling = exportTime(large) / Math.max(exportTime(small), 1);
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
    status: value > limit ? "violation" : "ok",
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
    structural("export-scaling", scaling, ceilings.exportScaling, "x", (value, limit) =>
      `package contract export aggregation scales ${value.toFixed(2)}x when the corpus doubles (limit ${limit}x)`
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
      `export scaling ${scaling.toFixed(2)}x, Type Facts ${responseBytes.toFixed(0)} bytes/source, ` +
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
