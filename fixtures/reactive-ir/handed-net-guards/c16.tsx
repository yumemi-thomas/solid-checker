import { createSignal, createMemo, createEffect, createStore, untrack } from "solid-js";
const flag = () => (globalThis as any).__c !== 1;
function useThing() {
  const [n, setN] = createSignal(0);
  return { read: n, go: setN };
}
function usePlain() {
  return { read: () => 0, go: (_v: number) => 0 };
}
export function App() {
  const hook = flag() ? useThing : useThing;
  const t = hook();
  return <button onClick={() => t.go(1)}>x</button>;
  return <i>x</i>;
}
