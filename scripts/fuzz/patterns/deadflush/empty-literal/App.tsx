import { createEffect, createSignal, flush } from "solid-js";
export function App() {
  const [value] = createSignal(1);
  createEffect(value, () => {
    ([] as number[]).forEach(() => flush());
  });
  return <i>app</i>;
}
