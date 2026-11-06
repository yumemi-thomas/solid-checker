import { createSignal, createMemo, createStore, createEffect, createRoot, untrack, Show, For } from "solid-js";
const [c] = createSignal(1);
const [st] = createStore({ a: 1 });
const useH = () => ({ a: createMemo(() => 1) });
export function App() {
  const h = useH();
  const v = Object.values(h)[0]();
  return <p>{v}</p>;
}
