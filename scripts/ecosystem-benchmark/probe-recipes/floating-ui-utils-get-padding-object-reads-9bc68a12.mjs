// Hand-authored `reads: []` veto for `@floating-ui/utils@0.2.12`, on the published
// runtime case `artifact-case:9bc68a12911a31424edd543d041c3f76b21f2b2fdef6813016a8f0bf02379f43`
// (the ESM `.` case; the ecosystem corpus reaches it from three rows, and the
// package's second `.` case carries none of these candidates).
//
// Not demand-scoped: written because the second scaffold pass (2026-09-13) showed
// the census can decide this candidate, and one recipe on this dependency node
// closes the entry in every row that depends on it.
//
// `getPaddingObject(padding)` is `typeof padding !== "number" ?
// expandPaddingObject(padding) : { top: padding, … }`. The number branch reads
// nothing; the object branch reads the caller's four sides once each, counted
// here on owned getters and deliberately not emitted (ADR 0034).
// What it cannot do: establish the closure. Finite samples only falsify, and
// the authenticated implementation census remains the proof. It hands the
// package no `session` and no `harness`.
import { getPaddingObject } from "@floating-ui/utils";

export async function runProbeSession(_session, harness) {
  let reads = 0;
  const ownedPadding = {
    get top() { reads += 1; return 1; },
    get right() { reads += 1; return undefined; },
    get bottom() { reads += 1; return 3; },
    get left() { reads += 1; return null; }
  };
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  const uniform = getPaddingObject(5);
  if (uniform.top !== 5 || uniform.right !== 5 || uniform.bottom !== 5 || uniform.left !== 5) {
    throw new Error(`getPaddingObject(5) answered ${JSON.stringify(uniform)}`);
  }
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  const expanded = getPaddingObject(ownedPadding);
  if (expanded.top !== 1 || expanded.right !== 0 || expanded.bottom !== 3 || expanded.left !== 0) {
    throw new Error(`getPaddingObject(object) answered ${JSON.stringify(expanded)}`);
  }
  if (reads !== 4) {
    throw new Error(`expected four reads of the caller's padding, observed ${reads}`);
  }
  // No emit: the reads are the caller's under ADR 0034.
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
