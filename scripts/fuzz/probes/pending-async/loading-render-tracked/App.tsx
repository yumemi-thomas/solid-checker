// ADR 0511: a render effect's first run is on the call even when the tree below reads the pending memo; rc.17 throws
// PENDING_ASYNC_UNTRACKED_READ.
import { Loading, createEffect, createMemo, createRenderEffect } from "solid-js";
import type { Accessor } from "solid-js";

function C1(props: { a1: Accessor<number> }) {
  createRenderEffect(() => {
    return 1;
  }, (v) => {
    props.a1();
  });
  return (
    <div>{props.a1()}</div>
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
