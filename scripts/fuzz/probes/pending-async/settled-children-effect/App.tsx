// ADR 0510: a tree whose expressions cannot go pending (a literal, a synchronous memo and a signal of literals, string attributes)
// holds nothing in the first render's tail flush; the user effect meets the async memo pending: PENDING_ASYNC_UNTRACKED_READ.
import { createEffect, createMemo, createSignal } from "solid-js";

export function C1() {
  const a1 = createMemo(async () => 1);
  const m1 = createMemo(() => 1);
  const [s1] = createSignal(2);
  createEffect(() => {
    return 1;
  }, (v) => {
    a1();
  });
  return (
    <div>
      <p data-view="v1">{m1()}</p>
      <p data-view="v2">{s1()}</p>
      <p>{1}</p>
    </div>
  );
}
