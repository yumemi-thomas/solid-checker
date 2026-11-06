import { createSignal, createMemo, createEffect, action, createStore } from "solid-js";
function keep(_g: (v: number) => void) { return 1; }
export function App() {
  const [, setS] = createSignal(0);
  createMemo(() => { keep(setS); return 0; });
  return <i>x</i>;
}
