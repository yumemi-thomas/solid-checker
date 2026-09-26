import { noop } from "probe-effect-reads-document";

export function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  if (noop() !== undefined) {
    harness.emit({ marker: "undeclared-alternative", kind: "callback", phase: "enter" });
  }
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
