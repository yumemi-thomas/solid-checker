// ADR 0499 (from the matrix's secondary codes): a store link read for a write in a closure the body calls once is an untracked strict read (STRICT_READ_UNTRACKED).
import { createSignal, createStore } from "solid-js";

export function C1() {
  const [s1, setS1] = createStore({ v: 0, a: { v: 0 }, items: [0] });
  const f1 = () => {
    s1.a.v = 7;
  };
  f1();
  return (
    <div>
    </div>
  );
}
