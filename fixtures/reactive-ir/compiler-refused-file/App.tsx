import { createSignal } from "solid-js";

// Positive: a file the compiler accepts keeps its findings beside a refused
// file it does not reach.
export function Counter() {
  const [count] = createSignal(0);
  const seen = count();
  return <p>{seen}</p>;
}
