// ADR 0508 review (false positive): the apply's first run throws PENDING_ASYNC_UNTRACKED_READ at the read, so the write after it never runs (the checker also reported REACTIVE_WRITE_IN_OWNED_SCOPE there).
import { createMemo, createRenderEffect, createSignal } from "solid-js";

export function C1() {
  const [w1, setW1] = createSignal(0);
  const a1 = createMemo(async () => 1);
  createRenderEffect(() => {
    return 1;
  }, (v) => {
    a1();
    setW1(7);
  });
  return (
    <div>{w1()}</div>
  );
}
