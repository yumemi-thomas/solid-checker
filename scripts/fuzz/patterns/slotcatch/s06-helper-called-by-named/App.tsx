import { createMemo, createSignal, createTrackedEffect, flush, isPending, latest, onSettled, resolve, until, untrack } from "solid-js";
const [n] = createSignal(1);
const p = () => n();
function inner() { void until(p); }
export function App() {
  createTrackedEffect(() => {
    const cb = () => { inner(); };
    isPending(cb);
  });
  return <i>{n()}</i>;
}
