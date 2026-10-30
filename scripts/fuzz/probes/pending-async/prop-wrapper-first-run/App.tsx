// ADR 0511: a wrapper the body defines reads the handed prop; the user effect's apply calls it once, in the first render's tail
// flush, while the memo is pending: rc.17 throws PENDING_ASYNC_UNTRACKED_READ at the read.
import { createEffect, createMemo } from "solid-js";
import type { Accessor } from "solid-js";

function C1(props: { a1: Accessor<number> }) {
  const wrapUse = () => {
    props.a1();
  };
  createEffect(() => {
    return 1;
  }, (v) => {
    wrapUse();
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
