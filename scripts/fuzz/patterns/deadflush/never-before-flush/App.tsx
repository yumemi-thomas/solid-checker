import { createEffect, createSignal, flush } from "solid-js";
function fail(): never {
  throw new Error("x");
}
export function App() {
  const [value] = createSignal(1);
  createEffect(value, () => {
    try {
      fail();
    } catch {}
    return;
    flush();
  });
  return <i>app</i>;
}
