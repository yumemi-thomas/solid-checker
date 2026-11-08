import { createSignal, createMemo, createEffect, action, createStore } from "solid-js";
function run(g: () => unknown) { g(); return 1; }
export function App() {
  const act = action(function* () { yield 1; });
  createMemo(() => { run(act); return 0; });
  return <i>x</i>;
}
