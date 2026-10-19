import { createSignal, createMemo, createStore } from "solid-js";
function now(f: () => void) {
  f();
}
function useX() {
  const [s] = createStore({ a: 1 });
  now(() => console.log(s.a));
  return s;
}
export function C() {
  useX();
  return <main />;
}
