// Combination programs (wrong rule): a pending async read in an effect apply, in a component whose JSX holds any expression (`{1}`); Solid reports PENDING_ASYNC_UNTRACKED_READ at the read, the checker names it STRICT_READ_UNTRACKED (with `<div>x</div>` it names it pending-async-unsuspendable-read).
import { createEffect, createMemo } from "solid-js";

export function C1() {
  const a1 = createMemo(async () => 1);
  createEffect(() => {
    return 1;
  }, (v) => {
    a1();
  });
  return <p>{1}</p>;
}
