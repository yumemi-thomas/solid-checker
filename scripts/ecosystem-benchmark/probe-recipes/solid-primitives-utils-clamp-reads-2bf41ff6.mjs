// Hand-authored `reads: []` veto for `@solid-primitives/utils@7.0.0-next.4`,
// on the published `.` runtime case
// `artifact-case:2bf41ff69a22f6022ae4b8485df1bd52096d111e906fc32792e33004f7956ee0`.
//
// Why this recipe is allowed to be this short: the artifact case carries no
// `runtime-accessor-installation` closure hazard, so the census states
// syntactically that `dist/index.js` installs no accessor and there is no trap
// here for the recipe to count (§ 12 of
// `docs/package-contract-v2/phase21/2026-09-10-reads-veto-observation-design.md`).
// The hazard does the work an observation counter used to. What is left is
// what a mandatory veto is for: sample the export and emit only on
// contradiction.
//
// It hands the package no `session` and no `harness`.
//
// `clamp` is `(n, min, max) => Math.min(Math.max(n, min), max)`. Three
// parameters, two `Math` calls, no member of anything this module built. The
// samples cross the three boundaries the arithmetic admits -- below `min`,
// inside the interval, above `max` -- plus a degenerate interval.
import { clamp } from "@solid-primitives/utils";

export async function runProbeSession(_session, harness) {
  for (const [n, min, max, expected] of [
    [5, 0, 10, 5],
    [-1, 0, 10, 0],
    [11, 0, 10, 10],
    [3, 3, 3, 3],
    [-7, -10, -5, -7]
  ]) {
    harness.emit({ marker: "call", kind: "call", phase: "enter" });
    const answered = clamp(n, min, max);
    if (!Object.is(answered, expected)) {
      throw new Error(`clamp(${n}, ${min}, ${max}) answered ${String(answered)}`);
    }
    // No emit is the point: nothing contradicted the closure.
    harness.emit({ marker: "call", kind: "call", phase: "exit" });
  }
}
