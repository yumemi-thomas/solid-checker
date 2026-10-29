// Fuzzer source axis (false clean): a child destructures a setter from its props and calls it in its body; Solid reports REACTIVE_WRITE_IN_OWNED_SCOPE at the call.
import { createSignal } from "solid-js";
import type { Accessor, Setter } from "solid-js";

type C1Props = { w1: Accessor<number>; setW1: Setter<number> };

function C1({ w1, setW1 }: C1Props) {
  setW1(7);
  return <div>{w1()}</div>;
}

export function P1() {
  const [w1, setW1] = createSignal(0);
  return <C1 w1={w1} setW1={setW1} />;
}
