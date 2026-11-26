import { createEffect, createSignal } from "solid-js";
import type { Setter } from "solid-js";

// ADR 1229: the prop's setter is called in a closure the effect's apply calls in place: a write in an effect apply is
// legal on rc.21. Clean.
function C1(props: { setR1: Setter<number> }) {
  createEffect(
    () => 1,
    () => {
      const f1 = () => {
        props.setR1(4);
      };
      f1();
    },
  );
  return <div />;
}

export function P1() {
  const [, setR1] = createSignal(3);
  return (
    <div>
      <C1 setR1={setR1} />
    </div>
  );
}
