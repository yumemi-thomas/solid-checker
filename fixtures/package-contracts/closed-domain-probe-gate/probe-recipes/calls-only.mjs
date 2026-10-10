// A recipe for the `creates: []` claim domain of `run` (and, by the same claim
// shape, `runCreatingOwner`, whose claim the census refuses before this recipe
// is ever reached — see README.md).
//
// The claim is *proved* by the implementation census: the only call in either
// body is `callback()`, a callee rooted at a parameter, whose body is the
// caller's behavior. This recipe cannot establish that and never tries to; it
// calls the export with a plain closure and emits only the call enter/exit
// pair, so a complete run with no contradiction marker is a clean pass of the
// veto and nothing more. Before the census existed this recipe was never
// reached — the demand refused as `UnsupportedDemand` at witness acquisition —
// and that was the trap it documented: a passing veto must never be what closes
// a domain.

import { runCreatingOwner } from "closed-domain-probe-gate-package";

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  runCreatingOwner(() => {});
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
