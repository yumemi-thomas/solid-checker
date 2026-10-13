// Hand-authored `reads: []` veto for `@floating-ui/utils@0.2.12`, on the published
// runtime case `artifact-case:9bc68a12911a31424edd543d041c3f76b21f2b2fdef6813016a8f0bf02379f43`
// (the ESM `.` case; the ecosystem corpus reaches it from three rows, and the
// package's second `.` case carries none of these candidates).
//
// Not demand-scoped: written because the second scaffold pass (2026-09-13) showed
// the census can decide this candidate, and one recipe on this dependency node
// closes the entry in every row that depends on it.
//
// `clamp(start, value, end)` is `max(start, min(value, end))` over the module's
// aliases of `Math.max`/`Math.min`. Note the argument order: the bound comes
// first. Objects are kept out of the samples on purpose: `Math.min` would
// coerce a caller's object through its own `valueOf`, a read that is the
// caller's under ADR 0034.
// What it cannot do: establish the closure. Finite samples only falsify, and
// the authenticated implementation census remains the proof. It hands the
// package no `session` and no `harness`.
import { clamp } from "@floating-ui/utils";

export async function runProbeSession(_session, harness) {
  for (const [start, value, end, expected] of [
    [0, 5, 10, 5],
    [0, -1, 10, 0],
    [0, 11, 10, 10],
    [3, 3, 3, 3],
    [-10, -7, -5, -7]
  ]) {
    harness.emit({ marker: "call", kind: "call", phase: "enter" });
    const answered = clamp(start, value, end);
    if (!Object.is(answered, expected)) {
      throw new Error(`clamp(${start}, ${value}, ${end}) answered ${String(answered)}`);
    }
    // No emit is the point: nothing contradicted the closure.
    harness.emit({ marker: "call", kind: "call", phase: "exit" });
  }
}
