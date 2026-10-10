// The isolation case: the package under test attacks the transcript from
// inside the worker's realm.
//
// Importing `closed-domain-probe-gate-tampering-package` runs its top level,
// which replaces `structuredClone`, `JSON.stringify`, `process.stdout.write`,
// and `Object.keys` — every name the report path once reached — and then
// exports a value its own declaration excludes.
//
// The expected outcome is a refusal, never a pass. The transcript survives the
// tampering because the harness and the worker captured their primordials
// before this module was imported and the frames travel on a descriptor the
// package cannot name, so the contradiction is reported and the veto fires.

import { entry } from "closed-domain-probe-gate-tampering-package";

function isDeclaredAlternative(value) {
  return typeof value === "function" || value === undefined;
}

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  if (!isDeclaredAlternative(entry)) {
    harness.emit({ marker: "undeclared-alternative", kind: "callback", ordinal: 0 });
  }
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
