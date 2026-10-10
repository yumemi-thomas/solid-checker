// Hand-authored probe recipe for the root `ChoiceAlternatives` claim domain of
// `entry`.
//
// The claim is that the exported value's alternatives are exactly the two the
// declaration enumerates. That claim is *proved* by the Type Facts census —
// the producer enumerated two alternatives and observed both exhaustively, and
// the verifier requires the proposal's enumeration to be that one. This recipe
// cannot establish it and never tries to.
//
// What it can do is catch the package contradicting its own declaration at
// runtime. `entry` is a callable, which is one of the declared alternatives,
// so there is nothing to report and the veto passes. Passing means only that
// nothing contradicted the claim.

import { entry } from "closed-domain-probe-gate-package";

function isDeclaredAlternative(value) {
  return typeof value === "function" || value === undefined;
}

export async function runProbeSession(_session, harness) {
  // The `call` pair is not decoration. Rust refuses an empty event transcript,
  // so a recipe has to prove it ran before its silence about the contradiction
  // can mean anything at all.
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  if (!isDeclaredAlternative(entry)) {
    harness.emit({ marker: "undeclared-alternative", kind: "callback", ordinal: 0 });
  }
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
