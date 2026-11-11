// Test support for ADR 0950 (scripts/lib/assumed.mjs): the reports a what-if build writes and a default build never does, built from the exact shape
// `assumed.mjs` documents, and a fake checker executable that prints one. Every consumer's refusal test feeds its real intake from here, so the shape
// is written once. Not a gate input: nothing but tests imports this.
import { chmodSync, writeFileSync } from "node:fs";
import { join } from "node:path";

export const HAZARD = "OpenMemberWrite(*)";

/** One finding of a default build's report (no `premises`). */
export const finding = (extra = {}) => ({
  id: "SC1001",
  rule: "strict-read-untracked",
  kind: "violation",
  severity: "error",
  message: "a read outside a tracked scope",
  primaryLocation: { path: "/p/App.tsx", startByte: 10, endByte: 20, line: 4, column: 3, endLine: 4 },
  ...extra,
});

/** A report as a default build writes it: no `assumed`, no premise on a finding. */
export const cleanReport = () => ({ status: "violation", findings: [finding()] });

/** What the what-if build writes when an assumed hazard flipped: `assumed` at the top and an `assumed` premise on every finding. */
export const flippedReport = () => ({
  status: "violation",
  findings: [finding({ premises: [{ kind: "assumed", hazards: [HAZARD] }] })],
  assumed: { hazards: [HAZARD], flipped: [HAZARD] },
});

/** Nothing flipped: the report is still tainted (the top-level key is always there), and its findings carry no premise. */
export const unflippedReport = () => ({ status: "violation", findings: [finding()], assumed: { hazards: [HAZARD], flipped: [] } });

/** The second half of the predicate alone: a premise of kind `assumed` on a finding, with no top-level key. */
export const premiseOnlyReport = () => ({
  status: "violation",
  findings: [finding({ premises: [{ kind: "assumed", hazards: [HAZARD] }] })],
});

/** Every tainted shape, by name: a consumer must refuse each. */
export const TAINTED = { flipped: flippedReport, unflipped: unflippedReport, premiseOnly: premiseOnlyReport };

/**
 * An executable that prints `report` (as JSON) on stdout and exits with `status`, in `directory`: what a consumer's intake reads in place of the
 * native checker. It ignores its arguments. Returns its path.
 */
export function fakeChecker(directory, report, { status = 0, name = "fake-checker" } = {}) {
  const data = join(directory, `${name}.json`);
  const path = join(directory, name);
  writeFileSync(data, JSON.stringify(report));
  writeFileSync(path, `#!/bin/sh\ncat "${data}"\nexit ${status}\n`);
  chmodSync(path, 0o755);
  return path;
}
