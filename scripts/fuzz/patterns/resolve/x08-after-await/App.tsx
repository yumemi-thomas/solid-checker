import { action, createEffect, createMemo, createSignal, onCleanup, onSettled, resolve } from "solid-js";
const [n, setN] = createSignal(1);
const p = () => n();
export function App() {
  createMemo(async () => { await Promise.resolve(); void resolve(p); return 1; });
  return <i>app</i>;
}
