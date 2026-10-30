// ADR 0511 distinction: the hook is called after the render effect; the effect's first run on the call meets `a1` in its
// temporal dead zone (a ReferenceError), not the pending memo.
import { createRenderEffect } from "solid-js";
import { useVals } from "./helpers";

export function C1() {
  createRenderEffect(() => {
    return 1;
  }, (v) => {
    a1();
  });
  const { a1 } = useVals();
  return (
    <div>
    </div>
  );
}
