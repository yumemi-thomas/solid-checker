import { createMemo, createSignal, createTrackedEffect, flush, isPending, latest, onSettled, resolve, until, untrack } from "solid-js";
const [n] = createSignal(1);
const p = () => n();
function wrap(f: () => unknown) { return isPending(f); }
export function App() {
  createTrackedEffect(() => {
    const cb = () => { void resolve(p); };
    wrap(cb);
  });
  return <i>{n()}</i>;
}
