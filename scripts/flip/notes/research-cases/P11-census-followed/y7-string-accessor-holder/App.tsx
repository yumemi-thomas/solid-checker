import { createSignal, createMemo, createStore, createEffect, createRoot, untrack, Show, For } from "solid-js";
const [c] = createSignal(1);
const [st] = createStore({ a: 1 });
export function App() {
  const x = `${{ toString: c }}`;
  return <p>{x}</p>;
}
