// ADR 0511 negative: the helper keeps the closure and never calls it, so the read never runs and nothing throws.
import { createEffect, createMemo, createRenderEffect } from "solid-js";
import type { Accessor } from "solid-js";

const kept: Array<() => unknown> = [];
function h1(cb: () => unknown) {
  kept.push(cb);
  return 1;
}

function C1(props: { a1: Accessor<number> }) {
  createRenderEffect(() => {
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
