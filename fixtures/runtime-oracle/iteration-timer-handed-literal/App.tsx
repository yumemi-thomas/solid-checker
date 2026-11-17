import { createSignal } from "solid-js";
import * as Health from "./health";

// ADR 1056: `run` is a parameter of an arrow written as the `schedule` member of an object literal handed whole to the exact project
// function `createHealth` (through a namespace import); `createHealth` only reads members of its parameter, calls `schedule` with an
// arrow, and calls `now`, an arrow, with the literal as `this`. The timer calls an arrow, the window reaches no code.
Health.createHealth({
  now: () => Date.now(),
  schedule: (delayMs, run) => void setTimeout(run, delayMs),
});

// REACTIVE_WRITE_IN_OWNED_SCOPE: a setter called in a component body, a violation where the signal's slots are exact.
export function BodyWrite() {
  const [s, set] = createSignal(1);
  set(2);
  return <time>{s()}</time>;
}
