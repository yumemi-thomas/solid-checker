// ADR 0511: <Loading> builds its children in a computed at creation, so the child's render effect runs its first apply on the
// call while the parent's memo is pending: rc.17 throws PENDING_ASYNC_UNTRACKED_READ.
import { Loading, createEffect, createMemo, createRenderEffect } from "solid-js";
import type { Accessor } from "solid-js";

function C1(props: { a1: Accessor<number> }) {
  createRenderEffect(() => {
    return 1;
  }, (v) => {
    props.a1();
  });
  return (
    <div></div>
  );
}

export function P1() {
  const a1 = createMemo(async () => 1);
  return (
    <div>
      <Loading fallback={0}>
        <C1 a1={a1} />
      </Loading>
    </div>
  );
}
