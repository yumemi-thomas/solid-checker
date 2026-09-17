// Hand-authored `reads: []` veto for `motion-utils@12.39.0`, on the published
// `.` runtime case
// `artifact-case:9a3a41a5f9148fb1f1e9175f5bea89d05e24dbd149c9629a9f4cf615fc6cfcc1`
// -- the dependency node the motion-solidjs rows certify through.
//
// Why this recipe is allowed to be this short: the artifact case carries no
// `runtime-accessor-installation` closure hazard, so the census states
// syntactically that `dist/es/index.mjs` installs no accessor and there is no
// trap here for the recipe to count (§ 12 of
// `docs/package-contract-v2/phase21/2026-09-10-reads-veto-observation-design.md`).
// The hazard does the work an observation counter used to. What is left is
// what a mandatory veto is for: sample the export and emit only on
// contradiction.
//
// It hands the package no `session` and no `harness`.
//
// `mirrorEasing(easing)` returns a function that calls the caller's easing on
// a reflected argument. The easing is the caller's code (ADR 0034).
import { mirrorEasing } from "motion-utils";

const same = (left, right) => JSON.stringify(left) === JSON.stringify(right);
const near = (left, right) => Math.abs(left - right) < 1e-9;
const expect = (label, ok) => {
  if (!ok) throw new Error(`${label} answered unexpectedly`);
};

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  const mirrored = mirrorEasing(p => p);
  expect("mirroring the identity is the identity", mirrored(0.25) === 0.25 && mirrored(0.75) === 0.75);
  const squared = mirrorEasing(p => p * p);
  expect("mirrorEasing halves the squared curve at the midpoint", near(squared(0.5), 0.5));
  // No emit is the point: nothing contradicted the closure.
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
