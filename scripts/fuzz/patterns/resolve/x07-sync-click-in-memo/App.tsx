import { action, createEffect, createMemo, createSignal, onCleanup, onSettled, resolve } from "solid-js";
const [n, setN] = createSignal(1);
const p = () => n();
export function App() {
  let el!: HTMLButtonElement;
  const m = createMemo(() => { el?.click(); return n(); });
  return <button ref={el} onClick={() => { void resolve(p); }}>{String(m())}</button>;
}
export const after = () => setN(2);
