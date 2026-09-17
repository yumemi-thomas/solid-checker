// Hand-authored `reads: []` veto for `@floating-ui/utils@0.2.12`, on the published
// runtime case `artifact-case:9bc68a12911a31424edd543d041c3f76b21f2b2fdef6813016a8f0bf02379f43`
// (the ESM `.` case; the ecosystem corpus reaches it from three rows, and the
// package's second `.` case carries none of these candidates).
//
// Not demand-scoped: written because the second scaffold pass (2026-09-13) showed
// the census can decide this candidate, and one recipe on this dependency node
// closes the entry in every row that depends on it.
//
// `expandPaddingObject(padding)` reads `padding.top`, `.right`, `.bottom`
// and `.left` once each and defaults a nullish one to `0`. Those four reads are
// of the caller's object, so the samples count them on owned getters, assert
// exactly four, and deliberately do not emit: they are the caller's under
// ADR 0034.
// What it cannot do: establish the closure. Finite samples only falsify, and
// the authenticated implementation census remains the proof. It hands the
// package no `session` and no `harness`.
import { expandPaddingObject } from "@floating-ui/utils";

export async function runProbeSession(_session, harness) {
  let reads = 0;
  const ownedPadding = {
    get top() { reads += 1; return 1; },
    get right() { reads += 1; return undefined; },
    get bottom() { reads += 1; return 3; },
    get left() { reads += 1; return null; }
  };
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  const answered = expandPaddingObject(ownedPadding);
  if (answered.top !== 1 || answered.right !== 0 || answered.bottom !== 3 || answered.left !== 0) {
    throw new Error(`expandPaddingObject answered ${JSON.stringify(answered)}`);
  }
  if (reads !== 4) {
    throw new Error(`expected four reads of the caller's padding, observed ${reads}`);
  }
  // No emit: the reads are the caller's under ADR 0034.
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  const empty = expandPaddingObject({});
  if (empty.top !== 0 || empty.right !== 0 || empty.bottom !== 0 || empty.left !== 0) {
    throw new Error(`expandPaddingObject({}) answered ${JSON.stringify(empty)}`);
  }
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
