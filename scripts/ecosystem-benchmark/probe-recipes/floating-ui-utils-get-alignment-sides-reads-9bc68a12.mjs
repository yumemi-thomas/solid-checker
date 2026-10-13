// Hand-authored `reads: []` veto for `@floating-ui/utils@0.2.12`, on the published
// runtime case `artifact-case:9bc68a12911a31424edd543d041c3f76b21f2b2fdef6813016a8f0bf02379f43`
// (the ESM `.` case; the ecosystem corpus reaches it from three rows, and the
// package's second `.` case carries none of these candidates).
//
// Not demand-scoped: written because the second scaffold pass (2026-09-13) showed
// the census can decide this candidate, and one recipe on this dependency node
// closes the entry in every row that depends on it.
//
// `getAlignmentSides(placement, rects, rtl)` derives the alignment axis from
// the caller's placement string and reads `rects.reference[length]` and
// `rects.floating[length]` once each -- reads of the caller's object, counted
// here on owned getters and deliberately not emitted (ADR 0034). The samples
// cover both orderings of the two lengths and the `rtl` flip.
// What it cannot do: establish the closure. Finite samples only falsify, and
// the authenticated implementation census remains the proof. It hands the
// package no `session` and no `harness`.
import { getAlignmentSides } from "@floating-ui/utils";

export async function runProbeSession(_session, harness) {
  let reads = 0;
  const rects = (referenceWidth, floatingWidth) => ({
    reference: {
      get width() { reads += 1; return referenceWidth; },
      get height() { reads += 1; return referenceWidth; }
    },
    floating: {
      get width() { reads += 1; return floatingWidth; },
      get height() { reads += 1; return floatingWidth; }
    }
  });
  for (const [placement, referenceWidth, floatingWidth, rtl, expected] of [
    ["top-start", 100, 50, false, "left,right"],
    ["top-start", 50, 100, false, "right,left"],
    ["top-start", 50, 100, true, "left,right"],
    ["left-end", 50, 100, false, "top,bottom"]
  ]) {
    const before = reads;
    harness.emit({ marker: "call", kind: "call", phase: "enter" });
    const answered = getAlignmentSides(placement, rects(referenceWidth, floatingWidth), rtl);
    if (answered.join() !== expected) {
      throw new Error(`getAlignmentSides(${placement}) answered ${answered.join()}`);
    }
    if (reads - before !== 2) {
      throw new Error(`expected two reads of the caller's rects, observed ${reads - before}`);
    }
    // No emit: the reads are the caller's under ADR 0034.
    harness.emit({ marker: "call", kind: "call", phase: "exit" });
  }
}
