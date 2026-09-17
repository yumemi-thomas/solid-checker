// Hand-authored probe recipe for the `creates: []` claim domain of
// `memberParameterRooted` (ADR 0034).
//
// The claim is *proved* by the implementation census: the read of `.read` off
// the parameter is `parameter-rooted-accessor` and the call is
// `parameter-rooted`, both the caller's code. This recipe cannot establish that
// and never tries to; it proves it ran, with an object whose `read` is a plain
// function so that a passing veto is a clean one.

import { memberParameterRooted } from "implementation-census-creates-package";

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  memberParameterRooted({ read: () => 1 });
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
