import { createMemo, createSignal, from, resolve, until } from "solid-js";
const [n] = createSignal(1);
const p = () => n();
export function App() {
  const m = createMemo(() => from<number>((set) => { void until(p); set(1); return () => {}; }));
  return <i>{String(m())}</i>;
}
