import { createSignal } from "solid-js";
import { startBrowse } from "./session";

// ADR 1056: `resume` is a parameter of `afterNextPaint`, a local function entered only as an operand of the composite callee
// `(options.scheduleAfterPaint ?? afterNextPaint)(…)`, which passes an arrow: the timer calls an arrow, the window reaches no code.
startBrowse({});

// REACTIVE_WRITE_IN_OWNED_SCOPE: a setter called in a component body, a violation where the signal's slots are exact.
export function BodyWrite() {
  const [s, set] = createSignal(1);
  set(2);
  return <time>{s()}</time>;
}
