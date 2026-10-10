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
// `number` is `(raw) => Number(raw)`. One coercion of the caller's string.
// The samples pin the documented edges -- `""` to `0`, a non-numeric string
// to `NaN` -- beside an integer, a decimal, and surrounding whitespace.
// Objects are kept out on purpose: `Number` would coerce a caller's object
// through its own `valueOf`, and that read is the caller's under ADR 0034.
// What it cannot do: establish the closure. Finite samples only falsify, and
// the authenticated implementation census remains the proof. It hands the
// package no `session` and no `harness`.
import { number } from "@solid-primitives/utils";

export async function runProbeSession(_session, harness) {
  for (const [raw, expected] of [
    ["42", 42],
    ["1.5", 1.5],
    [" 7 ", 7],
    ["", 0],
    ["x", Number.NaN]
  ]) {
    harness.emit({ marker: "call", kind: "call", phase: "enter" });
    const answered = number(raw);
    if (!Object.is(answered, expected)) {
      throw new Error(`number(${JSON.stringify(raw)}) answered ${String(answered)}`);
    }
    // No emit is the point: nothing contradicted the closure.
    harness.emit({ marker: "call", kind: "call", phase: "exit" });
  }
}
