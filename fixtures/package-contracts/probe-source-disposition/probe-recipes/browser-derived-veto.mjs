import { scrollRoot } from "probe-browser-source-only";

// A deliberate contradiction observed in the derived bytes the browser runs:
// the strip-only erasure blanks the return annotation, so reflected source no
// longer spells `): Element`. This is a veto control, not a real-package
// defect; it keeps the browser profile's veto path sensitive.
export function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  scrollRoot();
  if (!scrollRoot.toString().includes("): Element")) {
    harness.emit({ marker: "undeclared-alternative", kind: "callback", ordinal: 0 });
  }
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
