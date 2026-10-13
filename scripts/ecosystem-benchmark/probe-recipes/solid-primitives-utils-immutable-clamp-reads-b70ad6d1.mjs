// Hand-authored `reads: []` veto for `@solid-primitives/utils@7.0.0-next.4`,
// on the published `./immutable` runtime case
// `artifact-case:b70ad6d1290a49f1c7da26503bd848d3979fbb44e3e0fca19c60ed92c59bae47`.
//
// Why this recipe is allowed to be this short: the artifact case carries no
// `runtime-accessor-installation` closure hazard, so the census states
// syntactically that `dist/immutable/index.js` installs no accessor and there is no trap
// here for the recipe to count (§ 12 of
// `docs/package-contract-v2/phase21/2026-09-10-reads-veto-observation-design.md`).
// The hazard does the work an observation counter used to. What is left is
// what a mandatory veto is for: sample the export and emit only on
// contradiction.
//
// It hands the package no `session` and no `harness`.
//
// `clamp` is `(n, min, max) => Math.min(max, Math.max(min, n))`: two `Math`
// calls over three numbers, nothing this module built. Samples cross the
// three boundaries the arithmetic admits plus a degenerate interval.
import { clamp } from "@solid-primitives/utils/immutable";

const same = (left, right) => JSON.stringify(left) === JSON.stringify(right);
const expect = (label, ok) => {
  if (!ok) throw new Error(`${label} answered unexpectedly`);
};
export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  for (const [n, min, max, expected] of [[5, 0, 10, 5], [-1, 0, 10, 0], [11, 0, 10, 10], [3, 3, 3, 3]]) {
    expect(`clamp(${n}, ${min}, ${max})`, Object.is(clamp(n, min, max), expected));
  }
  // No emit is the point: nothing contradicted the closure.
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
