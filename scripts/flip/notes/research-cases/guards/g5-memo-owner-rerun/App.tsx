import { createSignal, createMemo, onCleanup } from "solid-js";
const [c] = createSignal(1);
const [on, setOn] = createSignal(1);
export function App() {
  const m = createMemo(() => { onCleanup(() => { c(); }); return on(); });
  return <p>{m()}</p>;
}
export async function after() { setOn(2); }
