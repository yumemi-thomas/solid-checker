import { createSignal, createMemo, createStore, createEffect, createRoot, untrack, Show, For } from "solid-js";
const [c] = createSignal(1);
const [st] = createStore({ a: 1 });
export function App() {
  const [x] = [createSignal(1)[0]];
  const v = x();
  return <p>{v}</p>;
}
