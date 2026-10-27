import { createSignal, createStore } from "solid-js";

const [other] = createSignal(1);

// Negative: a type-correct rebinding of the accessor and of a store root.
export function Rebinding() {
  let [count] = createSignal(0);
  count = other;
  let [store] = createStore({ n: 0 });
  store = { n: 1 };
  return [count, store];
}
