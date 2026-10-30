// ADR 0511 distinction: the IIFE throws a SyntaxError before the prop read, so the first run never reaches the pending read.
import { createEffect, createMemo, createRenderEffect } from "solid-js";
import type { Accessor } from "solid-js";

function C1(props: { a1: Accessor<number> }) {
  createRenderEffect(() => {
    return 1;
  }, (v) => {
    (() => {
      JSON.parse("{");
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
