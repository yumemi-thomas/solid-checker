import { createSignal, createTrackedEffect, flush } from "solid-js";
const [n, setN] = createSignal(0);
let late: undefined | ((x?: void) => void);
async function make(p: Promise<void>) {
  await p;
  // After the await, but a default runs where the function is called, not here.
  late = (x = flush()) => x;
  setN(1);
}
export function App() {
  void make(Promise.resolve());
  createTrackedEffect(() => {
    n();
    late?.();
  });
  return <p>x</p>;
}
