// Hand-authored `reads: []` veto for `@floating-ui/utils@0.2.12`, on the published
// runtime case `artifact-case:9bc68a12911a31424edd543d041c3f76b21f2b2fdef6813016a8f0bf02379f43`
// (the ESM `.` case; the ecosystem corpus reaches it from three rows, and the
// package's second `.` case carries none of these candidates).
//
// Not demand-scoped: written because the second scaffold pass (2026-09-13) showed
// the census can decide this candidate, and one recipe on this dependency node
// closes the entry in every row that depends on it.
//
// `getAlignmentAxis(placement)` is `getOppositeAxis(getSideAxis(placement))`:
// one index read of the caller's string and two string comparisons. A
// primitive has no getter for a coercion or a read to reach.
// What it cannot do: establish the closure. Finite samples only falsify, and
// the authenticated implementation census remains the proof. It hands the
// package no `session` and no `harness`.
import { getAlignmentAxis } from "@floating-ui/utils";

export async function runProbeSession(_session, harness) {
  for (const [placement, expected] of [
    ["top", "x"],
    ["bottom-end", "x"],
    ["left", "y"],
    ["right-start", "y"]
  ]) {
    harness.emit({ marker: "call", kind: "call", phase: "enter" });
    const answered = getAlignmentAxis(placement);
    if (answered !== expected) {
      throw new Error(`getAlignmentAxis(${placement}) answered ${String(answered)}`);
    }
    // No emit is the point: nothing contradicted the closure.
    harness.emit({ marker: "call", kind: "call", phase: "exit" });
  }
}
