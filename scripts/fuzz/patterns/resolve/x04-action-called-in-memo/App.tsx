import { action, createEffect, createMemo, createSignal, onCleanup, onSettled, resolve } from "solid-js";
const [n, setN] = createSignal(1);
const p = () => n();
const act = action(function* () { void resolve(p); });
export function App() {
  const m = createMemo(() => { void act(); return n(); });
  return <i>{String(m())}</i>;
}
