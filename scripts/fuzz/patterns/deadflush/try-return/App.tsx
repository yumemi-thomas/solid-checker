import { createEffect, createSignal, flush } from "solid-js";
export function App() {
  const [value] = createSignal(1);
  createEffect(value, () => {
    try {
      return;
    } finally {
      void 0;
    }
    flush();
  });
  return <i>app</i>;
}
