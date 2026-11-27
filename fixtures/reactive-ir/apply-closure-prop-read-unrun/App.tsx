import { createEffect, createSignal } from "solid-js";
import type { Accessor } from "solid-js";

// ADR 1229: the closure is handed on, never called in place, so its read is no read of the apply: it keeps its
// obligation (no read runs on rc.21).
function C1(props: { r1: Accessor<number> }) {
  createEffect(
    () => 1,
    () => {
      const f1 = () => {
        props.r1();
      };
      return void f1;
    },
  );
  return <div />;
}

export function P1() {
  const [r1] = createSignal(3);
  return (
    <div>
      <C1 r1={r1} />
    </div>
  );
}
