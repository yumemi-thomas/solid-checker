import { createSignal, createMemo, createEffect, onSettled } from "solid-js";
const a = createMemo(async () => { await new Promise((r) => setTimeout(r, 40)); return 7; });
export function App() {
  const go = () => { a(); }; setTimeout(go, 0);
  return <i>app</i>;
}
