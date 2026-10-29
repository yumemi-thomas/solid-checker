// ADR 0503 (from the misuse matrix): a write in a render effect's apply, first run on the call in a component mounted by render, throws REACTIVE_WRITE_IN_OWNED_SCOPE.
import { createRenderEffect, createSignal } from "solid-js";

export function C1() {
  const [w1, setW1] = createSignal(0);
  createRenderEffect(() => {
    return 1;
  }, (v) => {
    setW1(7);
  });
  return (
    <div>
    </div>
  );
}
