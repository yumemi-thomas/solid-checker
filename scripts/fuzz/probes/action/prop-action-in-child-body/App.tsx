// Fuzzer source axis (false clean): a child calls an action it received as a prop in its body; Solid reports ACTION_CALLED_IN_OWNED_SCOPE at the call.
import { action, createSignal } from "solid-js";
import type { Accessor } from "solid-js";

type C1Props = { w1: Accessor<number>; a1: () => Promise<void> };

function C1(props: C1Props) {
  void props.a1();
  return <div>{props.w1()}</div>;
}

export function P1() {
  const [w1, setW1] = createSignal(0);
  const a1 = action(function* () { setW1(7); });
  return <C1 w1={w1} a1={a1} />;
}
