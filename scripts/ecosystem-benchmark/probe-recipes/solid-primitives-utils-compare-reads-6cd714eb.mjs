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
// `compare` is `(a, b) => (a < b ? -1 : a > b ? 1 : 0)`. Two relational
// operators over the caller's values and nothing else. The samples cover both
// orders, equality, strings, and a `NaN` operand (both comparisons false, so
// `0`). Objects are kept out on purpose: `<` would coerce a caller's object
// through its own `valueOf`, and that read is the caller's under ADR 0034.
// What it cannot do: establish the closure. Finite samples only falsify, and
// the authenticated implementation census remains the proof. It hands the
// package no `session` and no `harness`.
import { compare } from "@solid-primitives/utils";

export async function runProbeSession(_session, harness) {
  for (const [a, b, expected] of [
    [1, 2, -1],
    [2, 1, 1],
    [2, 2, 0],
    ["a", "b", -1],
    ["b", "a", 1],
    [Number.NaN, 1, 0]
  ]) {
    harness.emit({ marker: "call", kind: "call", phase: "enter" });
    const answered = compare(a, b);
    if (!Object.is(answered, expected)) {
      throw new Error(`compare(${String(a)}, ${String(b)}) answered ${String(answered)}`);
    }
    // No emit is the point: nothing contradicted the closure.
    harness.emit({ marker: "call", kind: "call", phase: "exit" });
  }
}
