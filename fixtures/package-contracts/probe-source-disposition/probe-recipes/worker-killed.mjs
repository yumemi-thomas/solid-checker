import { hideOutside } from "probe-effect-reads-document";

// A worker that dies with no chance to report: no failure path inside the
// worker runs, so its report descriptor closes with no run frame on it. Every
// event a passing run carries has already been emitted, so reading the dead
// worker as a clean non-observation would pass the gate.
export function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
  void hideOutside;
  process.kill(process.pid, "SIGKILL");
}
