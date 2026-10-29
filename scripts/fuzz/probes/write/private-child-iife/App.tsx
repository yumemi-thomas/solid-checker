// ADR 0506 (false clean, from the mount axis): an IIFE in the body of a component a parent renders by JSX alone writes a signal (REACTIVE_WRITE_IN_OWNED_SCOPE); the checker certified the program.
import { createSignal } from "solid-js";

function C1() {
  const [w1, setW1] = createSignal(0);
  (() => {
    setW1(7);
  })();
  return (
    <div>
    </div>
  );
}

export function P1() {
  return (
    <div>
      <C1 />
    </div>
  );
}
