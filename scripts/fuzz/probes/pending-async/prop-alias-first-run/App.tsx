// ADR 0511: the child aliases the parent's handed memo (`const d = props.a1`) before its render effect; the first run on the
// call reads it pending, so rc.17 throws PENDING_ASYNC_UNTRACKED_READ.
import { createMemo, createRenderEffect } from "solid-js";
import type { Accessor } from "solid-js";

function C1(props: { a1: Accessor<number> }) {
  const read = props.a1;
  createRenderEffect(() => {
    return 1;
  }, (v) => {
    read();
  });
  return <div></div>;
}

export function P1() {
  const a1 = createMemo(async () => 1);
  return (
    <div>
      <C1 a1={a1} />
    </div>
  );
}
