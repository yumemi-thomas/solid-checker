import { createEffect, flush } from "solid-js";
export function App() {
  createEffect(
    (): number => {
      throw new Error("x");
    },
    () => {
      flush();
    },
  );
  return <i>app</i>;
}
