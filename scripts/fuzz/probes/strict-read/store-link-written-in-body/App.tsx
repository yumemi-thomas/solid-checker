// ADR 0498 (found by replaying a misuse-matrix case): a store link read for a write below it in a component body is an untracked strict read (STRICT_READ_UNTRACKED).
import { createSignal, createStore } from "solid-js";

export function C1() {
  const [s1, setS1] = createStore({ v: 0, a: { v: 0 }, items: [0] });
  s1.a.v = 7;
  return (
    <div>
    </div>
  );
}
