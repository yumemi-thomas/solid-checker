import { createSignal } from "solid-js";
import { createHealth } from "./health";

// ADR 1056, the escape twin: `createHealth` hands its parameter whole to `keep`, so `schedule` may be called anywhere with anything: `run`
// is not proven, the slots stay withdrawn, the write is uncertifiable.
createHealth({
  schedule: (delayMs, run) => void setTimeout(run, delayMs),
});

// REACTIVE_WRITE_IN_OWNED_SCOPE: a setter called in a component body, a violation where the signal's slots are exact.
export function BodyWrite() {
  const [s, set] = createSignal(1);
  set(2);
  return <time>{s()}</time>;
}
