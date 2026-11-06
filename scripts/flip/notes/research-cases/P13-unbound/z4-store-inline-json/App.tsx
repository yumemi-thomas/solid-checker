import { createSignal, createMemo, createStore, createEffect, createRoot, untrack, Show, For } from "solid-js";
const [c] = createSignal(1);
const [st] = createStore({ a: 1 });
export function App() {
  const v = JSON.stringify(createStore({ a: 1 })[0]);
  return <p>{v}</p>;
}
