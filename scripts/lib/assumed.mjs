// ADR 0950: what-if runs. The what-if checker (`make build-what-if`: `solid-checker-what-if --assume <hazard ids>`) analyzes a project with named
// whole-program hazard facts treated as absent, to PREDICT what proving them away would lift. Its JSON report is tainted, in exactly this shape,
// which no other build writes:
//   - a top-level `assumed`: `{ hazards: [...the ids assumed], flipped: [...the ones the program really had] }`, always present;
//   - when `flipped` is not empty, `premises: [{ kind: "assumed", hazards: [...] }]` on every finding.
// A prediction is no fact about the shipped checker, so EVERY consumer of the checker's JSON report refuses a tainted one at its intake
// (`refuseAssumed`): it can never become a snapshot, a verdict, a baseline or a finding row. Only scripts/what-if.mjs reads one, on purpose.
//
// The published packages cannot import from scripts/: packages/cli/eslint.cjs, packages/cli/scripts/development-feedback.mjs and
// packages/wasm/owned-client.mjs carry a copy of the predicate and the refusal, each with a test of its own.

export class AssumedReportError extends Error {
  constructor(message) {
    super(message);
    this.name = "AssumedReportError";
  }
}

/**
 * Whether a parsed checker report is tainted by a what-if assumption: it has a top-level `assumed` key (whatever its value), or one of its
 * findings has a premise of kind `assumed`. Anything that is not a report object is not tainted (it is no report at all; the reader's own
 * checks refuse it).
 */
export function isAssumed(report) {
  if (report === null || typeof report !== "object") return false;
  if (Object.prototype.hasOwnProperty.call(report, "assumed")) return true;
  const findings = report.findings;
  return Array.isArray(findings) && findings.some((finding) => Array.isArray(finding?.premises) && finding.premises.some((premise) => premise?.kind === "assumed"));
}

/**
 * The report itself when it is the checker's; throws an `AssumedReportError` naming `consumer` when it is tainted. `consumer` names the reader
 * (its script path, or what it is), so the error says whose intake refused.
 */
export function refuseAssumed(report, consumer) {
  if (!isAssumed(report)) return report;
  const hazards = Array.isArray(report.assumed?.hazards) ? ` (assumed: ${report.assumed.hazards.join(", ")})` : "";
  throw new AssumedReportError(
    `${consumer} refuses this checker report: it is tainted by a what-if assumption${hazards}, a prediction of what proving a hazard away would lift and no fact about the checker. Only scripts/what-if.mjs reads one.`,
  );
}
