import { noop } from "probe-typescript-source-only";

export function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  const returned = noop(7);
  // Deliberate veto control, not a real package contradiction. First prove the
  // function is the derived one (annotation erased, source spacing preserved).
  if (returned === 7 && !noop.toString().includes(": number") && noop.toString().includes("value        ")) {
    harness.emit({ marker: "undeclared-alternative", kind: "callback", ordinal: 0 });
  }
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
