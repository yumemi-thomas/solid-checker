import { createEffect, createMemo, createSignal, createTrackedEffect, onSettled, untrack } from "solid-js";

const [count] = createSignal(0);

function readCount() {
  return count();
}

export function App() {
  createMemo(() => count());
  createMemo(() => readCount());
  createEffect(() => count(), () => count());
  createTrackedEffect(() => count());
  onSettled(() => count());
  untrack(() => count());
  return <div>{count()}</div>;
}
