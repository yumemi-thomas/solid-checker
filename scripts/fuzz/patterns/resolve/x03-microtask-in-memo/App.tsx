import { action, createEffect, createMemo, createSignal, onCleanup, onSettled, resolve } from "solid-js";
const [n, setN] = createSignal(1);
const p = () => n();
export function App() {
  const m = createMemo(() => { queueMicrotask(() => { void resolve(p); }); return n(); });
  return <i>{String(m())}</i>;
}
