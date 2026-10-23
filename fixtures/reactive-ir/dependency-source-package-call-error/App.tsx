import { createSignal } from "solid-js";
import { run } from "dep-run";

export function Panel() {
  const [count] = createSignal(1);
  run(count);
  return <p>{count()}</p>;
}
