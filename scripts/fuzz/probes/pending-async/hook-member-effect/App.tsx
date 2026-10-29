// ADR 0511: the hook's returned object is kept and its member read in a user effect's apply; the first render's tail flush
// runs it while the memo is pending, so rc.17 throws PENDING_ASYNC_UNTRACKED_READ.
import { createEffect } from "solid-js";
import { useVals } from "./helpers";

export function C1() {
  const vals = useVals();
  createEffect(() => {
    return 1;
  }, (v) => {
    vals.a1();
  });
  return (
    <div>
    </div>
  );
}
