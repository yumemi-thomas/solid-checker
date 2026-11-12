import { createSignal, createTrackedEffect, flush } from "solid-js";
const [n, setN] = createSignal(0);
let late: undefined | ((x?: void) => void);
export function App() {
  setTimeout(() => {
    late = (x = flush()) => x;
    setN(1);
  });
  createTrackedEffect(() => {
    n();
    late?.();
  });
  return <p>x</p>;
}
