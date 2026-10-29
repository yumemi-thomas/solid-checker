// ADR 0507 (from the mount axis): a render effect's first apply, in a child an inert parent renders under a module-scope mount, writes a signal (REACTIVE_WRITE_IN_OWNED_SCOPE).
import { createRenderEffect, createSignal } from "solid-js";

function C1() {
  const [w1, setW1] = createSignal(0);
  createRenderEffect(() => {
    return 1;
  }, (v) => {
    setW1(7);
  });
  return (
    <div>{w1()}</div>
  );
}

export function P1() {
  return (
    <div>
      <C1 />
    </div>
  );
}
