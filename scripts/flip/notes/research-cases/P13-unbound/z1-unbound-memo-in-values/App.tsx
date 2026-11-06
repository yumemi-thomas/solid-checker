import { createSignal, createMemo, createStore, createEffect, createRoot, untrack, Show, For } from "solid-js";
const [c] = createSignal(1);
const [st] = createStore({ a: 1 });
export function App() {
  const o = { a: createMemo(() => 1) };
  const v = Object.values(o)[0]();
  return <p>{v}</p>;
}
