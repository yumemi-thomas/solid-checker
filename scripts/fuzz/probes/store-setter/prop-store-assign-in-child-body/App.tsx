// Fuzzer source axis (false clean): a child assigns to a store it received as a prop, outside the setter; Solid reports STORE_WRITE_OUTSIDE_SETTER at the assignment.
import { createStore } from "solid-js";
import type { StoreSetter } from "solid-js";

type C1Props = { s1: { v: number }; setS1: StoreSetter<{ v: number }> };

function C1(props: C1Props) {
  props.s1.v = 7;
  return <div>{props.s1.v}</div>;
}

export function P1() {
  const [s1, setS1] = createStore({ v: 0 });
  return <C1 s1={s1} setS1={setS1} />;
}
