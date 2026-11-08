import { createSignal, createMemo, createEffect, onSettled } from "solid-js";
const a = createMemo(async () => { await new Promise((r) => setTimeout(r, 40)); return 7; });
export function App() {
  setTimeout(() => { a(); }, 0);
  return <i>app</i>;
}
