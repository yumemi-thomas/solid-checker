// ADR 0497 (from the misuse matrix): a store handed through an object-literal arrow's slot, written past the root, is dropped (STORE_WRITE_OUTSIDE_SETTER).
import { createSignal, createStore } from "solid-js";

export function C1() {
  const [s1, setS1] = createStore({ v: 0, a: { v: 0 }, items: [0] });
  const useThing = () => ({ useIt: s1 });
  const { useIt } = useThing();
  useIt.a.v = 7;
  return (
    <div>
    </div>
  );
}
