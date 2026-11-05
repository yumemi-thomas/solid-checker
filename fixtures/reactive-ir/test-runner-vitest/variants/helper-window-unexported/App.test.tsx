import { it } from "vitest";
import { createRoot, flush } from "solid-js";

// Unexported, called only in a test: its `window` dispatch has a placed caller in
// an open program too (ADR 0640), so no flush below is an obligation.
function press(): void {
  window.dispatchEvent(new Event("keydown"));
}

function withRoot(run: () => void): void {
  createRoot(() => run());
}

it("flushes on the runner's stack", () => {
  press();
  createRoot(() => {
    flush();
  });
  withRoot(() => flush());
});
