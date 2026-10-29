// ADR 0511: the parent's async memo handed as an exact prop is read in an IIFE inside the child's render-effect apply; the
// first run is on the call and the IIFE runs in place, so rc.17 throws PENDING_ASYNC_UNTRACKED_READ.
import { createEffect, createMemo, createRenderEffect } from "solid-js";
import type { Accessor } from "solid-js";

function C1(props: { a1: Accessor<number> }) {
  createRenderEffect(() => {
    return 1;
  }, (v) => {
    (() => {
      props.a1();
    })();
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
