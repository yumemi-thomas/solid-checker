// `plainArithmetic`'s `reads: []`, and the attempt to contradict it.
//
// The observation is exact **for this package**, which is the whole argument
// for a hand recipe: its author knows that every reactive-shaped source this
// module owns is `ownProxy`, and that `observedReads()` counts its trap. A
// synthesized veto cannot know that, which is why none is registered for the
// domain (see the reads veto observation design, 2026-09-10).
import { observedReads, plainArithmetic } from "implementation-census-reads-package";

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  const before = observedReads();
  for (const [a, b, expected] of [[1, 2, 3], [0, 0, 0], [-4, 4, 0]]) {
    if (plainArithmetic(a, b) !== expected) {
      throw new Error("plainArithmetic sample disagrees");
    }
  }
  // No emit is the point: the closure stands because nothing contradicted it.
  if (observedReads() !== before) {
    harness.emit({ marker: "read-operation", kind: "call", phase: "enter" });
  }
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
