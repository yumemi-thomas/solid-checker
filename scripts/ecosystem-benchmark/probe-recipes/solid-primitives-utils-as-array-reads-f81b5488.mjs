// Hand-authored `reads: []` veto for `@solid-primitives/utils@7.0.0-next.4`, on the
// published `.` runtime case `artifact-case:f81b5488e3996a8a5274855cc63234dc16f0708c5e95952296ff999003a4834a`
// (the case the ecosystem corpus certifies under solid-js@2.0.0-rc.3; the same
// dist/index.js bytes appear under two more `.` cases, each with its own copy of
// this module, because the private workspace writes one file per recipe entry).
//
// Not demand-scoped: written because the second scaffold pass (2026-09-13) showed
// the census can decide this candidate, and one recipe on this dependency node
// closes the entry in every row that depends on it.
//
// `asArray` is `Array.isArray(value) ? value : value ? [value] : []`. The
// only value it consults is its argument's truthiness, and `Array.isArray`
// reads no property. The apparatus passes an object with an owned getter and
// asserts the getter never ran, so a version that started reading, say,
// `.length` of a non-array would fail the recipe.
// What it cannot do: establish the closure. Finite samples only falsify, and
// the authenticated implementation census remains the proof. It hands the
// package no `session` and no `harness`.
import { asArray } from "@solid-primitives/utils";

export async function runProbeSession(_session, harness) {
  let observed = 0;
  const ownedByTheRecipe = {
    get length() {
      observed += 1;
      return 1;
    }
  };
  const list = [1, 2];
  for (const [input, check] of [
    [list, answered => answered === list],
    [undefined, answered => Array.isArray(answered) && answered.length === 0],
    [0, answered => Array.isArray(answered) && answered.length === 0],
    ["a", answered => Array.isArray(answered) && answered.length === 1 && answered[0] === "a"],
    [ownedByTheRecipe, answered => Array.isArray(answered) && answered[0] === ownedByTheRecipe]
  ]) {
    harness.emit({ marker: "call", kind: "call", phase: "enter" });
    const answered = asArray(input);
    if (!check(answered)) {
      throw new Error(`asArray(${String(input)}) answered ${String(answered)}`);
    }
    harness.emit({ marker: "call", kind: "call", phase: "exit" });
  }
  if (observed !== 0) {
    throw new Error(`asArray read a caller-owned source ${observed} time(s)`);
  }
}
