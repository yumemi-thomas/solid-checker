// ADR 0511: the prop is read in a named closure the render-effect apply calls once; rc.17 throws PENDING_ASYNC_UNTRACKED_READ.
import { createEffect, createMemo, createRenderEffect } from "solid-js";
import type { Accessor } from "solid-js";

function C1(props: { a1: Accessor<number> }) {
  createRenderEffect(() => {
    return 1;
  }, (v) => {
    const f1 = () => {
      props.a1();
    };
    f1();
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
