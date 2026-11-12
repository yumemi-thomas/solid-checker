import { createEffect, createSignal, flush, untrack } from "solid-js";
function run(fn: () => void) {
  fn();
}
const [n] = createSignal(0);
export function App() {
  createEffect(() => n(), () => {
    run(...[flush]);
  });
  return <p>{n()}</p>;
}
