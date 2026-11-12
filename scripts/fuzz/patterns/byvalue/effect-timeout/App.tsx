import { createEffect, createSignal, flush, untrack } from "solid-js";
const [n] = createSignal(0);
export function App() {
  createEffect(() => n(), () => {
    setTimeout(flush);
  });
  return <p>{n()}</p>;
}
