// Hand-authored `reads: []` veto for `@solid-primitives/utils@6.4.1`, on the published `.`
// runtime case `artifact-case:9887e1372316b2956afed7699a2491cf0143fe1b71ee64dc8da00509890939c5`.
//
// Demand-scoped: consumer projects import `asAccessor` from this exact version
// (`docs/package-contract-v2/phase21/2026-09-12-consumer-demand-measurement.md`).
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
