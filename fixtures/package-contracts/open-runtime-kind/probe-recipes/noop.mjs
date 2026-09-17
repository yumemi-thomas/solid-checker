import { noop } from "probe-open-runtime-kind";

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  noop();
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
