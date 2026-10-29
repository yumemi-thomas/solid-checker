// Fuzzer source axis (false clean): a setter reaches the child through a middle component that spreads its props (`<C1 {...props} />`); the child's body call is REACTIVE_WRITE_IN_OWNED_SCOPE in Solid.
import { createSignal } from "solid-js";
import type { Accessor, Setter } from "solid-js";

type C1Props = { w1: Accessor<number>; setW1: Setter<number> };

function C1(props: C1Props) {
  props.setW1(7);
  return <div>{props.w1()}</div>;
}

function M1(props: C1Props) {
  return <C1 {...props} />;
}

export function P1() {
  const [w1, setW1] = createSignal(0);
  return <M1 w1={w1} setW1={setW1} />;
}
