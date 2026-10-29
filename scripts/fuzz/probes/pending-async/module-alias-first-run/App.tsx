// ADR 0511: a `const` alias of an imported module-scope async memo is that memo; the render effect's first run reads it pending,
// so rc.17 throws PENDING_ASYNC_UNTRACKED_READ.
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
