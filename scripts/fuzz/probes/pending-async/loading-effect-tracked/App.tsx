// ADR 0511 distinction: the child's tree reads the memo, so the boundary shows its fallback and holds the user effect's queued
// run until the reveal; the apply then meets the memo settled (rc.17 warns STRICT_READ_UNTRACKED, no pending throw).
import { Loading, createEffect, createMemo, createRenderEffect } from "solid-js";
import type { Accessor } from "solid-js";

function C1(props: { a1: Accessor<number> }) {
  createEffect(() => {
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
