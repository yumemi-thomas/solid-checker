// Fuzzer source axis (false clean): a child calls a setter it received as a prop in its body; Solid reports REACTIVE_WRITE_IN_OWNED_SCOPE at the call (the checker is silent and claims the file).
import { createSignal } from "solid-js";
import type { Accessor, Setter } from "solid-js";

type C1Props = { w1: Accessor<number>; setW1: Setter<number> };

function C1(props: C1Props) {
  props.setW1(7);
  return <div>{props.w1()}</div>;
}

export function P1() {
  const [w1, setW1] = createSignal(0);
  return <C1 w1={w1} setW1={setW1} />;
}
