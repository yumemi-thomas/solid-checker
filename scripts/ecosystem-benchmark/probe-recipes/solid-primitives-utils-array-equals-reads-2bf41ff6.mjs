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
// `arrayEquals` is
// `(a, b) => a === b || a.length === b.length && a.every((e, i) => e === b[i])`.
// It iterates two **caller-supplied** arrays, so every element read it performs
// is a read of a value the caller owns, which ADR 0034 assigns to the caller
// and not to this closure. The samples cover identity, equal contents,
// differing length, differing element, and the empty pair.
import { arrayEquals } from "@solid-primitives/utils";

export async function runProbeSession(_session, harness) {
  const shared = [1, 2, 3];
  for (const [a, b, expected] of [
    [shared, shared, true],
    [[1, 2, 3], [1, 2, 3], true],
    [[1, 2], [1, 2, 3], false],
    [[1, 2, 3], [1, 9, 3], false],
    [[], [], true]
  ]) {
    harness.emit({ marker: "call", kind: "call", phase: "enter" });
    const answered = arrayEquals(a, b);
    if (answered !== expected) {
      throw new Error(`arrayEquals(${JSON.stringify(a)}, ${JSON.stringify(b)}) answered ${String(answered)}`);
    }
    harness.emit({ marker: "call", kind: "call", phase: "exit" });
  }
}
