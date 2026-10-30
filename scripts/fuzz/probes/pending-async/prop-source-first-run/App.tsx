// ADR 0510: the mounting parent creates the async memo and hands it over as an exact prop; the child's render effect reads it on
// its first run, still pending: rc.17 throws PENDING_ASYNC_UNTRACKED_READ.
import { createMemo, createRenderEffect } from "solid-js";
import type { Accessor } from "solid-js";

function C1(props: { a1: Accessor<number> }) {
  createRenderEffect(() => {
    return 1;
  }, (v) => {
    props.a1();
  });
  return (
    <div>
    </div>
  );
}

export function P1() {
  const a1 = createMemo(async () => 1);
  return (
    <div>
      <C1 a1={a1} />
    </div>
  );
}
