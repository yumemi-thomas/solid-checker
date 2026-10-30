// ADR 0511: a user effect's apply hands a closure reading the prop to a helper that calls it first; the static mount's flush
// runs the apply while the memo is pending, so rc.17 throws PENDING_ASYNC_UNTRACKED_READ.
import { createEffect, createMemo, createRenderEffect } from "solid-js";
import type { Accessor } from "solid-js";

function h1(cb: () => unknown) {
  cb();
  return 1;
}

function C1(props: { a1: Accessor<number> }) {
  createEffect(() => {
    return 1;
  }, (v) => {
    h1(() => {
      props.a1();
    });
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
