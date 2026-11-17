import { createSignal } from "solid-js";
import { createHealth } from "./health";

// ADR 1056, the receiver twin: `createHealth` calls `options.now()`, and the literal's `now` is a method that reads `this`, the literal
// itself, so it may call `this.schedule` with any function: `run` is not proven, the slots stay withdrawn, the write is uncertifiable.
createHealth({
  now() {
    return this.schedule.length;
  },
  schedule: (delayMs, run) => void setTimeout(run, delayMs),
});

// REACTIVE_WRITE_IN_OWNED_SCOPE: a setter called in a component body, a violation where the signal's slots are exact.
export function BodyWrite() {
  const [s, set] = createSignal(1);
  set(2);
  return <time>{s()}</time>;
}
