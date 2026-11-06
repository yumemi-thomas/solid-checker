import { createSignal, createMemo, createEffect, action, createStore } from "solid-js";
function later(g: (v: number) => void) { return () => g(1); }
export function App() {
  const [, setS] = createSignal(0);
  createMemo(() => { later(setS); return 0; });
  return <i>x</i>;
}
