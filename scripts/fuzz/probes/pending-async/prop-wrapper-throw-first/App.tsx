// ADR 0511 negative: the wrapper throws a SyntaxError before the prop read, so the apply's first run never reads the memo.
import { createEffect, createMemo } from "solid-js";
import type { Accessor } from "solid-js";

function C1(props: { a1: Accessor<number> }) {
  const wrapUse = () => {
    JSON.parse("{");
    props.a1();
  };
  createEffect(() => {
    return 1;
  }, (v) => {
    wrapUse();
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
