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
// `trueFn` is `() => true`. No parameter, no operand, no member access. It is
// the degenerate case of the domain and the cheapest possible falsifier: if a
// future version of this package makes `trueFn` consult anything at all, the
// constant it answers is the first thing that moves.
import { trueFn } from "@solid-primitives/utils";

export async function runProbeSession(_session, harness) {
  for (let sample = 0; sample < 3; sample += 1) {
    harness.emit({ marker: "call", kind: "call", phase: "enter" });
    const answered = trueFn();
    if (answered !== true) {
      throw new Error(`trueFn() answered ${String(answered)}`);
    }
    harness.emit({ marker: "call", kind: "call", phase: "exit" });
  }
}
