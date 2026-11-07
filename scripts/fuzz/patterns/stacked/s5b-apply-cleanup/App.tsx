import { createEffect, onCleanup } from "solid-js";
export function App() {
  createEffect(() => 1, () => {
    onCleanup(() => {});
  });
  return <i>app</i>;
}
