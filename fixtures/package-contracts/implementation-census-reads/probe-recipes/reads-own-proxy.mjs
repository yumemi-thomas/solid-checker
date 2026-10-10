// `readsOwnProxy`'s `reads: []`, which is **false**, and this recipe says so.
//
// The export reads a proxy this module built. Under `semantic-model.md`
// § reads [Decision 2026-09-10] a proxy the export *owns* is exactly the case
// the domain is about, so the closure the generator proposed must not survive
// its gate.
//
// This is the observation a synthesized veto structurally cannot make: it
// watches values the *caller* supplied, and this read is on neither an
// argument nor anything reachable from one.
import { observedReads, readsOwnProxy } from "implementation-census-reads-package/owned";

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  const before = observedReads();
  if (readsOwnProxy() !== 1) {
    throw new Error("readsOwnProxy sample disagrees");
  }
  if (observedReads() === before) {
    throw new Error("the fixture's own trap did not run; the observation is broken");
  }
  harness.emit({ marker: "read-operation", kind: "call", phase: "enter" });
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
