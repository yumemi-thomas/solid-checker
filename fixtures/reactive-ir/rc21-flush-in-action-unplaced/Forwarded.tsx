import { action, flush } from "solid-js";

// ADR 0832: a closure handed down a chain of forwarding helpers inside an
// action's body runs on a step's stack (Chrome, rc.21: FLUSH_IN_ACTION). The
// forward runs in place (ADR 0931): an SC2006 violation.
export const forwarded = action(function* () {
  const hc0 = (g: () => void) => { g(); };
  const hc1 = (g: () => void) => { hc0(g); };
  hc1(() => {
    flush();
  });
});

// Silent: after the body's await no step is on the stack, wherever the
// forwarded closure runs.
export const afterAwait = action(async function* () {
  await Promise.resolve();
  const hc0 = (g: () => void) => { g(); };
  const hc1 = (g: () => void) => { hc0(g); };
  hc1(() => {
    flush();
  });
});
