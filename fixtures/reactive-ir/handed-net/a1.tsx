import { createSignal, createMemo, createEffect, action, createStore } from "solid-js";
function run(g: (v: number) => void) { g(1); return 1; }
export function App() {
  const [, setS] = createSignal(0);
  createMemo(() => { run(setS); return 0; });
  return <i>x</i>;
}
