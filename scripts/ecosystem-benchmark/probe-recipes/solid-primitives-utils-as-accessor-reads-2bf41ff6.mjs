// Hand-authored `reads: []` veto for `@solid-primitives/utils@7.0.0-next.4`, on the
// published `.` runtime case `artifact-case:2bf41ff69a22f6022ae4b8485df1bd52096d111e906fc32792e33004f7956ee0`
// (the case the ecosystem corpus certifies under solid-js@2.0.0-rc.0; the same
// dist/index.js bytes appear under two more `.` cases, each with its own copy of
// this module, because the private workspace writes one file per recipe entry).
//
// Not demand-scoped: written because the second scaffold pass (2026-09-13) showed
// the census can decide this candidate, and one recipe on this dependency node
// closes the entry in every row that depends on it.
//
// `asAccessor(v)` is `typeof v === "function" ? v : () => v`. It returns
// either the caller's own function by identity or a fresh closure over the
// caller's value; neither path reads anything, and the closure it builds
// returns the captured value without consulting a source. The last sample
// calls the returned accessor over an owned getter object and asserts the
// getter was not read, so a version whose wrapper started dereferencing the
// value would fail the recipe.
// What it cannot do: establish the closure. Finite samples only falsify, and
// the authenticated implementation census remains the proof. It hands the
// package no `session` and no `harness`.
import { asAccessor } from "@solid-primitives/utils";

export async function runProbeSession(_session, harness) {
  const fn = () => "called";
  let observed = 0;
  const ownedByTheRecipe = {
    get current() {
      observed += 1;
      return "read";
    }
  };
  for (const [input, check] of [
    [fn, answered => answered === fn],
    [1, answered => typeof answered === "function" && answered() === 1],
    [undefined, answered => typeof answered === "function" && answered() === undefined],
    [ownedByTheRecipe, answered => typeof answered === "function" && answered() === ownedByTheRecipe]
  ]) {
    harness.emit({ marker: "call", kind: "call", phase: "enter" });
    const answered = asAccessor(input);
    if (!check(answered)) {
      throw new Error(`asAccessor(${String(input)}) answered ${String(answered)}`);
    }
    harness.emit({ marker: "call", kind: "call", phase: "exit" });
  }
  if (observed !== 0) {
    throw new Error(`asAccessor read a caller-owned source ${observed} time(s)`);
  }
}
