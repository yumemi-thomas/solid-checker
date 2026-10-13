// Hand-authored `reads: []` veto for `@solid-primitives/utils@6.4.1`, on the published `.`
// runtime case `artifact-case:9887e1372316b2956afed7699a2491cf0143fe1b71ee64dc8da00509890939c5`.
//
// Not named by any of the 118 consumer projects in the demand corpus; written
// because the second scaffold pass showed the census can decide this candidate
// and one recipe on this dependency node closes the entry in every row that
// depends on it (2026-09-13).
//
// `isNonNullable` is `(i) => i != null`. Loose inequality against `null`
// coerces nothing -- an object operand is answered by identity -- so the
// samples pass an owned getter object and assert it is never read, beside the
// two nullish values and the falsy primitives that must still answer `true`.
// What it cannot do: establish the closure. Finite samples only falsify, and
// the authenticated implementation census remains the proof. It hands the
// package no `session` and no `harness`.
import { isNonNullable } from "@solid-primitives/utils";

export async function runProbeSession(_session, harness) {
  let observed = 0;
  const ownedByTheRecipe = {
    get current() {
      observed += 1;
      return "read";
    }
  };
  for (const [input, expected] of [
    [null, false],
    [undefined, false],
    [0, true],
    ["", true],
    [false, true],
    [ownedByTheRecipe, true]
  ]) {
    harness.emit({ marker: "call", kind: "call", phase: "enter" });
    const answered = isNonNullable(input);
    if (answered !== expected) {
      throw new Error(`isNonNullable(${String(input)}) answered ${String(answered)}`);
    }
    harness.emit({ marker: "call", kind: "call", phase: "exit" });
  }
  if (observed !== 0) {
    throw new Error(`isNonNullable read a caller-owned source ${observed} time(s)`);
  }
}
