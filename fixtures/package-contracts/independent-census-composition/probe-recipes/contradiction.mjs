import { value } from "root-package";
export async function runProbeSession(_session, harness) {
  value(1);
  // Deliberate veto control, not an observation of a real package defect.
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  harness.emit({ marker: "undeclared-alternative", kind: "callback", ordinal: 0 });
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
