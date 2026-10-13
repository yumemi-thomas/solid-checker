import { scrollRoot } from "probe-browser-source-only";

// Runs only under ADR 0033's browser profile: `document` is real, not a shim.
// The drain plan asks for one animation frame, which the Node worker refuses.
export function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  if (scrollRoot() !== document.documentElement) {
    harness.emit({ marker: "undeclared-alternative", kind: "callback", ordinal: 0 });
  }
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
