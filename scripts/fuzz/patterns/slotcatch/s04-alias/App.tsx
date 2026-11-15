import { createMemo, createSignal, createTrackedEffect, flush, isPending, latest, onSettled, resolve, until, untrack } from "solid-js";
const [n] = createSignal(1);
const p = () => n();
function inner() { void resolve(p); }
export function App() {
  createTrackedEffect(() => {
    const run = inner;
    isPending(run);
  });
  return <i>{n()}</i>;
}
