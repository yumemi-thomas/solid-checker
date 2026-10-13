// Hand-authored probe recipe for the `returns: []` and `creates: []` claim
// domains of the certifying exports (ADR 0035).
//
// The claims are *proved* by the implementation census; this recipe cannot
// establish them and never tries to. It calls each certifying export with a
// finite sample and emits the corpus's falsification marker if a call hands
// back anything but `undefined` — the observation that would contradict
// `returns: []` — so a passing veto is a clean one.

import {
  bareCompletion,
  earlyBareReturn,
  bareReturnInLoop,
  nestedReturnsValue,
} from "implementation-census-returns-package";

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  const results = [
    bareCompletion(2),
    earlyBareReturn(false),
    earlyBareReturn(true),
    bareReturnInLoop(5),
    bareReturnInLoop(1),
    nestedReturnsValue(4),
  ];
  if (results.some((value) => value !== undefined)) {
    harness.emit({ marker: "undeclared-alternative", kind: "callback", ordinal: 0 });
  }
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
