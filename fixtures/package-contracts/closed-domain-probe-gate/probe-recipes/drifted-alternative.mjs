// The negative half: the same body pointed at the sibling export.
//
// `driftedEntry` has the byte-identical declaration `(() => void) | undefined`,
// so its Type Facts census — and therefore the closure witness — is identical
// to `entry`'s. Its runtime ships a number. The package contradicts its own
// declaration, this recipe observes it, and the mandatory veto refuses the row.

import { driftedEntry } from "closed-domain-probe-gate-package";

function isDeclaredAlternative(value) {
  return typeof value === "function" || value === undefined;
}

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  if (!isDeclaredAlternative(driftedEntry)) {
    harness.emit({ marker: "undeclared-alternative", kind: "callback", ordinal: 0 });
  }
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
