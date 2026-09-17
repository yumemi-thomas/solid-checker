// Hand-authored `reads: []` veto for `@solid-primitives/utils@6.4.1`, on the published `.`
// runtime case `artifact-case:9887e1372316b2956afed7699a2491cf0143fe1b71ee64dc8da00509890939c5`.
//
// Demand-scoped: consumer projects import `compare` from this exact version
// (`docs/package-contract-v2/phase21/2026-09-12-consumer-demand-measurement.md`).
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
