// ADR 0510 (false positive of ADR 0503): the IIFE throws a SyntaxError before the write, so rc.17 never reports
// REACTIVE_WRITE_IN_OWNED_SCOPE; the write is an obligation.
import { createRenderEffect, createSignal } from "solid-js";

export function C1() {
  const [w1, setW1] = createSignal(0);
  createRenderEffect(() => {
    return 1;
  }, (v) => {
    (() => {
      JSON.parse("{");
      setW1(7);
    })();
  });
  return (
    <div>
    </div>
  );
}
