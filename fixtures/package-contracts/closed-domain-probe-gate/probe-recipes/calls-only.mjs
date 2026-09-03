// A recipe for a `creates: []` claim domain, written to show that no recipe
// can rescue one.
//
// `runCreatingOwner` really does create a reactive owner, so `creates: []` is
// false of it — and this recipe, which calls it and emits only the call
// enter/exit pair, observes nothing to the contrary. That is the trap the
// binding must not fall into: a clean pass here would be closure decided by
// finite non-observation.
//
// It never gets that far. The Type Facts census that discharges
// `DomainExhaustiveness` is a census of the declaration, and `index.d.ts` says
// nothing about ownership, so the demand refuses as `UnsupportedDemand` at
// witness acquisition — before any probe is launched.

import { runCreatingOwner } from "closed-domain-probe-gate-package";

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  runCreatingOwner(() => {});
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
