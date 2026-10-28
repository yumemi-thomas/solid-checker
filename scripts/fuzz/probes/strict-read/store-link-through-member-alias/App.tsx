// ADR 0498 (from the matrix's secondary codes): a store link read through a member alias (thing.useIt) for a write in a body is an untracked strict read (STRICT_READ_UNTRACKED).
import { createSignal, createStore } from "solid-js";

export function C1() {
  const [s1, setS1] = createStore({ v: 0, a: { v: 0 }, items: [0] });
  const useThing = () => ({ useIt: s1 });
  const thing = useThing();
  thing.useIt.a.v = 7;
  return (
    <div>
    </div>
  );
}
