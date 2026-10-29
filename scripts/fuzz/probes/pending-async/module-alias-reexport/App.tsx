// ADR 0511: the alias's import is a re-export of the creating module's `const`; the read is still that memo, pending.
import { createRenderEffect, createSignal } from "solid-js";
import { a1 } from "./helpers";

export function C1() {
  const useIt = a1;
  createRenderEffect(() => {
    return 1;
  }, (v) => {
    useIt();
  });
  return (
    <div>
    </div>
  );
}
