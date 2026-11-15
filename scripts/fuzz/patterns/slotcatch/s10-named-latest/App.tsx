import { createMemo, createSignal, createTrackedEffect, flush, isPending, latest, onSettled, resolve, until, untrack } from "solid-js";
const [n] = createSignal(1);
const p = () => n();
export function App() {
  createTrackedEffect(() => {
    const cb = () => { void resolve(p); };
    latest(cb);
  });
  return <i>{n()}</i>;
}
