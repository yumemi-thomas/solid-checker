// ADR 0510: a module-scope async memo of a module whose scope only creates nodes (here the component's own) is pending until
// a microtask; the first render's render effect reads it on its first run, so rc.17 throws PENDING_ASYNC_UNTRACKED_READ.
import { createMemo, createRenderEffect } from "solid-js";

const a1 = createMemo(async () => 1);

export function C1() {
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
