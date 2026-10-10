// Hand-authored probe recipe for the `creates: []` claim domains of `loopCall`,
// `switchBreak` and `whileBreak` — the three exports whose bodies carry a
// construct the producer cannot give a reachability *lower bound* for.
//
// One module for all three because the tests supply it per claim id, and each
// invocation below is what makes the veto observe the export whose domain the
// census proved. The claim itself is proved by the census
// (`docs/adr/0008-implementation-census-for-creates.md`): every call inside
// those constructs is on the wire with `reach: unknown`, and `mount` — the one
// callee — is a module-local declaration the census walks.
//
// Each export is called so the loop body actually runs: `loopCall` and
// `whileBreak` loop while their argument is truthy and `mount` returns it
// unchanged, so a truthy argument would not terminate `loopCall`. The falsy
// argument is deliberate: it is the *census* that proves the claim, and a veto
// that hangs proves nothing at all.

import {
  loopCall,
  switchBreak,
  whileBreak
} from "implementation-census-creates-package";

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  loopCall(0);
  switchBreak("mount", { tag: "el" });
  whileBreak(0);
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
