import { noop } from "probe-typescript-source-only";

export function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  if (noop(7) !== 7) {
    harness.emit({ marker: "undeclared-alternative", kind: "callback", phase: "enter" });
  }
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
