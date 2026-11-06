import { createSignal, createMemo, createEffect, action, createStore } from "solid-js";
function peek(g: () => number) { return g(); }
export function App() {
  const a = createMemo(async () => 1);
  return <p>{peek(a)}</p>;
}
