// The static call graph contains cycleA -> cycleB -> cycleA, while the boolean
// argument makes the sampled execution finite. The creates census must close
// the graph through an exact local-recursion back-edge before this mandatory
// veto is allowed to run.

import { cycle } from "implementation-census-creates-package";

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  cycle();
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
