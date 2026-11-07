import { action, createEffect, onSettled } from "solid-js";
export function App() {
  const a1 = action(function* () {
    createEffect(() => 1, () => {});
  });
  onSettled(() => {
    a1();
  });
  return <i>app</i>;
}
