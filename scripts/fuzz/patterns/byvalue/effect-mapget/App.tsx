import { createEffect, createSignal, flush, untrack } from "solid-js";
const [n] = createSignal(0);
export function App() {
  createEffect(() => n(), () => {
    new Map([["k", flush]]).get("k")!();
  });
  return <p>{n()}</p>;
}
