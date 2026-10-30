// Fuzzer source axis (false clean): a child calls a prop setter through a `merge` view of its props in its body; Solid reports REACTIVE_WRITE_IN_OWNED_SCOPE at the call.
import { createSignal, merge } from "solid-js";
import type { Accessor, Setter } from "solid-js";

type C1Props = { w1: Accessor<number>; setW1: Setter<number> };

function C1(props: C1Props) {
  const merged = merge({ extra: 1 }, props);
  merged.setW1(7);
  return <div>{merged.w1()}</div>;
}

export function P1() {
  const [w1, setW1] = createSignal(0);
  return <C1 w1={w1} setW1={setW1} />;
}
