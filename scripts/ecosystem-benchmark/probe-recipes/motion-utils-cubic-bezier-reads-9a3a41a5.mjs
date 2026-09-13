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
// `cubicBezier(x1, y1, x2, y2)` returns an easing closed over four numbers and
// a binary subdivision; a linear definition returns `noop`. The returned
// function reads only its argument and those numbers.
import { cubicBezier } from "motion-utils";

const same = (left, right) => JSON.stringify(left) === JSON.stringify(right);
const near = (left, right) => Math.abs(left - right) < 1e-9;
const expect = (label, ok) => {
  if (!ok) throw new Error(`${label} answered unexpectedly`);
};

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  const linear = cubicBezier(0, 0, 1, 1);
  expect("cubicBezier(0, 0, 1, 1) is the identity", linear(0.3) === 0.3);
  const easeInCurve = cubicBezier(0.42, 0, 1, 1);
  expect("a curve keeps its endpoints", easeInCurve(0) === 0 && easeInCurve(1) === 1);
  const middle = easeInCurve(0.5);
  expect("an ease-in curve lags the identity in the middle", middle > 0 && middle < 0.5);
  // No emit is the point: nothing contradicted the closure.
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
