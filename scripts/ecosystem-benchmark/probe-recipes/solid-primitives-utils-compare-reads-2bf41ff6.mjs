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
// `compare` is `(a, b) => a < b ? -1 : a > b ? 1 : 0`. Its two parameters are
// caller-supplied values and the relational operators read nothing this module
// owns. The samples cover all three answers and both operand types the
// comparison is written for.
import { compare } from "@solid-primitives/utils";

export async function runProbeSession(_session, harness) {
  for (const [a, b, expected] of [
    [1, 2, -1],
    [2, 1, 1],
    [2, 2, 0],
    ["a", "b", -1],
    ["b", "a", 1]
  ]) {
    harness.emit({ marker: "call", kind: "call", phase: "enter" });
    const answered = compare(a, b);
    if (answered !== expected) {
      throw new Error(`compare(${JSON.stringify(a)}, ${JSON.stringify(b)}) answered ${String(answered)}`);
    }
    harness.emit({ marker: "call", kind: "call", phase: "exit" });
  }
}
