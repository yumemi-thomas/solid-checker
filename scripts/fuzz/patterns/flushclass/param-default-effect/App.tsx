import { createEffect, createSignal, flush } from "solid-js";
const [n, setN] = createSignal(0);
let late: undefined | ((x?: void) => void);
export function App() {
  // The default is written in a fresh-stack callback (a timer's) and runs wherever `late` is called: here an effect's apply.
  setTimeout(() => {
    late = (x = flush()) => x;
    setN(1);
  });
  createEffect(
    () => n(),
    () => {
      late?.();
    },
  );
  return <p>x</p>;
}
