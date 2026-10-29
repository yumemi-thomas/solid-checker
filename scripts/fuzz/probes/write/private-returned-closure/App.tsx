// ADR 0506 review (false clean): a component a parent renders by JSX alone calls an arrow an IIFE returns; the arrow writes (REACTIVE_WRITE_IN_OWNED_SCOPE) and the checker certified the program.
import { createSignal } from "solid-js";

function C1() {
  const [c, setC] = createSignal(0);
  const f = (() => () => setC(1))();
  f();
  return <p>{c()}</p>;
}

export function P1() {
  return (
    <div>
      <C1 />
    </div>
  );
}
