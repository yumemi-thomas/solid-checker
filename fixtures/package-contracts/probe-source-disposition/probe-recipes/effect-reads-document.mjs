import { hideOutside } from "probe-effect-reads-document";

// Runs `hideOutside` the way a `@solidjs/signals` flush runs an effect: from a
// microtask, outside the recipe's own call chain. Its `ReferenceError` is
// therefore uncaught rather than a rejection of `runProbeSession`, and it is
// raised while the harness drains.
export function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  queueMicrotask(() => {
    hideOutside();
  });
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
