import { createEffect, createSignal, flush } from "solid-js";
export function App() {
  const [value] = createSignal(1);
  createEffect(value, () => {
    return;
    flush();
  });
  return <i>app</i>;
}
