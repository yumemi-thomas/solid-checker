import { action, createEffect, createMemo, createSignal, onCleanup, onSettled, resolve } from "solid-js";
const [n, setN] = createSignal(1);
const p = () => n();
export function App() {
  createEffect(() => n(), () => { void resolve(p); });
  return <i>app</i>;
}
