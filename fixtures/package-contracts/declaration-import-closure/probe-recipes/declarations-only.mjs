import { noop } from "probe-declaration-only";
export function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  noop();
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
