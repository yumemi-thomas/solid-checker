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
  const t = useThing.call(null);
  createMemo(() => { t.go(1); return 0; });
  return <i>x</i>;
}
