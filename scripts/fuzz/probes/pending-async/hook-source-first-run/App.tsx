// ADR 0511: a hook of another module creates the async memo and returns it; the component calls it before its render effect,
// whose first run is on the call and meets the memo pending, so rc.17 throws PENDING_ASYNC_UNTRACKED_READ.
import { createRenderEffect } from "solid-js";
import { useVals } from "./helpers";

export function C1() {
  const { a1 } = useVals();
  createRenderEffect(() => {
    return 1;
  }, (v) => {
    a1();
  });
  return (
    <div>
    </div>
  );
}
