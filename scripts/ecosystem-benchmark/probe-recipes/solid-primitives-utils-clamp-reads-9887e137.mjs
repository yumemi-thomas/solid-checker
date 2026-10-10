// Hand-authored `reads: []` veto for `@solid-primitives/utils@6.4.1`, on the published `.`
// runtime case `artifact-case:9887e1372316b2956afed7699a2491cf0143fe1b71ee64dc8da00509890939c5`.
//
// Demand-scoped: consumer projects import `clamp` from this exact version
// (`docs/package-contract-v2/phase21/2026-09-12-consumer-demand-measurement.md`).
//
// `clamp` is `(n, min, max) => Math.min(Math.max(n, min), max)`. Three
// parameters, two `Math` calls, no member of anything this module built. The
// samples cross the three boundaries the arithmetic admits -- below `min`,
// inside the interval, above `max` -- plus a degenerate interval. Objects are
// kept out on purpose: `Math.max` would coerce a caller's object through its
// own `valueOf`, and that read is the caller's under ADR 0034.
// What it cannot do: establish the closure. Finite samples only falsify, and
// the authenticated implementation census remains the proof. It hands the
// package no `session` and no `harness`.
import { clamp } from "@solid-primitives/utils";

export async function runProbeSession(_session, harness) {
  for (const [n, min, max, expected] of [
    [5, 0, 10, 5],
    [-1, 0, 10, 0],
    [11, 0, 10, 10],
    [3, 3, 3, 3],
    [-7, -10, -5, -7]
  ]) {
    harness.emit({ marker: "call", kind: "call", phase: "enter" });
    const answered = clamp(n, min, max);
    if (!Object.is(answered, expected)) {
      throw new Error(`clamp(${n}, ${min}, ${max}) answered ${String(answered)}`);
    }
    // No emit is the point: nothing contradicted the closure.
    harness.emit({ marker: "call", kind: "call", phase: "exit" });
  }
}
