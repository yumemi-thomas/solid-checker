import { createEffect, createSignal } from "solid-js";
import type { Accessor } from "solid-js";

// ADR 1229: the prop's accessor is called in a closure the apply calls once, in place: the read is the apply's, an
// untracked read of a signal (rc.21 warns STRICT_READ_UNTRACKED). A violation.
function C1(props: { r1: Accessor<number> }) {
  createEffect(
    () => 1,
    () => {
      const f1 = () => {
        props.r1();
      };
      f1();
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
