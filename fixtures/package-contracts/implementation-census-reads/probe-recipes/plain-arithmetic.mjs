// `plainArithmetic`'s `reads: []`, and the attempt to contradict it.
//
// The recipe got simpler when the fixture split, and the reason is the whole
// argument for the closure hazard. It used to import `observedReads` and
// compare the proxy trap's counter before and after, because that counter was
// the only way its author could claim the sample had observed *every*
// reactive-shaped source this module owns.
//
// It no longer needs to. `./index.js` carries no run-time accessor
// installation, so the closure states — syntactically, in a fact the census
// can read — that there is no trap here to count. The proxy lives in
// `./owned`, whose `reads` is withdrawn before any candidate is planned.
//
// So this recipe does what a veto is actually for: sample the export, and
// emit only if something contradicts the closure. Nothing can, which is the
// point of a mandatory gate over a domain the census already proved.
import { plainArithmetic } from "implementation-census-reads-package";

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  for (const [a, b, expected] of [[1, 2, 3], [0, 0, 0], [-4, 4, 0]]) {
    if (plainArithmetic(a, b) !== expected) {
      throw new Error("plainArithmetic sample disagrees");
    }
  }
  // No emit is the point: the closure stands because nothing contradicted it.
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
