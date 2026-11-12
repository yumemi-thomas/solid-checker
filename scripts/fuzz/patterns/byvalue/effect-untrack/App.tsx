import { createEffect, createSignal, flush, untrack } from "solid-js";
const [n] = createSignal(0);
export function App() {
  createEffect(() => n(), () => {
    untrack(flush);
  });
  return <p>{n()}</p>;
}
