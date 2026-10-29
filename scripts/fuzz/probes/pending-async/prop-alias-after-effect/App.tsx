// ADR 0511 negative: the alias is declared after the render effect, so its first run meets `read` in its temporal dead zone
// (a ReferenceError), not the pending memo.
import { createMemo, createRenderEffect } from "solid-js";
import type { Accessor } from "solid-js";

function C1(props: { a1: Accessor<number> }) {
  createRenderEffect(() => {
    return 1;
  }, (v) => {
    read();
  });
  const read = props.a1;
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
