// ADR 0511 distinction: the wrapper is also called in the body after the effect, where the read meets the memo pending in the
// body's strict-read window; the apply's call is not the wrapper's one call, so no first-run claim is placed at it.
import { createEffect, createMemo } from "solid-js";
import type { Accessor } from "solid-js";

function C1(props: { a1: Accessor<number> }) {
  const wrapUse = () => {
    props.a1();
  };
  createEffect(() => {
    return 1;
  }, (v) => {
    wrapUse();
  });
  wrapUse();
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
