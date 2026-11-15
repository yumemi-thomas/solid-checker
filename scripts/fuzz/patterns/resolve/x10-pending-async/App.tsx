import { action, createEffect, createMemo, createSignal, onCleanup, onSettled, resolve } from "solid-js";
const [n, setN] = createSignal(1);
const p = () => n();
import { isPending } from "solid-js";
export function App() {
  const m = createMemo(() => { void isPending(async () => { await resolve(p); return false; }); return n(); });
  return <i>{String(m())}</i>;
}
