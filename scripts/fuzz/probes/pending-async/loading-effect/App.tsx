// ADR 0511: under <Loading> with nothing pending read below it, the boundary shows content and the first render's tail flush
// runs the user effect while the memo is pending: rc.17 throws PENDING_ASYNC_UNTRACKED_READ.
import { Loading, createEffect, createMemo, createRenderEffect } from "solid-js";
import type { Accessor } from "solid-js";

function C1(props: { a1: Accessor<number> }) {
  createEffect(() => {
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
