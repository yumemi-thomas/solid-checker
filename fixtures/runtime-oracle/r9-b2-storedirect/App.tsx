import { createSignal, createMemo, createStore, merge, omit, onSettled, untrack } from "solid-js";
function now(f: () => void) {
  f();
}
function later(f: () => void) {
  setTimeout(f, 0);
}
export function C() {
  const [s] = createStore({ a: { b: 1 } });
  console.log(s.a.b);
  return <main />;
}
