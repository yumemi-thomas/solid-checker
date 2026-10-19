import { createSignal, createMemo, createStore, merge, omit, onSettled, untrack } from "solid-js";
function now(f: () => void) {
  f();
}
function later(f: () => void) {
  setTimeout(f, 0);
}
export function C() {
  const [count] = createSignal(0);
  now(() => { const g = () => count(); console.log(g()); });
  return <main />;
}
