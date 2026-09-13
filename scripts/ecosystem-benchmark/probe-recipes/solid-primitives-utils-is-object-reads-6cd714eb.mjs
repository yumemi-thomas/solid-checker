// Hand-authored `reads: []` veto for `@solid-primitives/utils@7.0.0-next.4`, on the
// published `.` runtime case `artifact-case:6cd714eb04fb05bfa0d7b88061837c8b505d6d91829178ccd8730a40be329b6d`
// (the case the ecosystem corpus certifies under solid-js@2.0.0-rc.3; the same
// dist/index.js bytes appear under two more `.` cases, each with its own copy of
// this module, because the private workspace writes one file per recipe entry).
//
// Not demand-scoped: written because the second scaffold pass (2026-09-13) showed
// the census can decide this candidate, and one recipe on this dependency node
// closes the entry in every row that depends on it.
//
// `isObject` is `value !== null && (typeof value === "object" || typeof
// value === "function")`. A `typeof` reads no property. The apparatus passes an
// object with an owned getter and asserts it never ran.
// What it cannot do: establish the closure. Finite samples only falsify, and
// the authenticated implementation census remains the proof. It hands the
// package no `session` and no `harness`.
import { isObject } from "@solid-primitives/utils";

export async function runProbeSession(_session, harness) {
  let observed = 0;
  const ownedByTheRecipe = {
    get current() {
      observed += 1;
      return "read";
    }
  };
  for (const [input, expected] of [
    [ownedByTheRecipe, true],
    [() => 1, true],
    [null, false],
    [undefined, false],
    [1, false],
    ["object", false]
  ]) {
    harness.emit({ marker: "call", kind: "call", phase: "enter" });
    const answered = isObject(input);
    if (answered !== expected) {
      throw new Error(`isObject(${String(input)}) answered ${String(answered)}`);
    }
    harness.emit({ marker: "call", kind: "call", phase: "exit" });
  }
  if (observed !== 0) {
    throw new Error(`isObject read a caller-owned source ${observed} time(s)`);
  }
}
