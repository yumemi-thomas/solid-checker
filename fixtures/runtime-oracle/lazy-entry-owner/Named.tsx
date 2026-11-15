import { onCleanup } from "solid-js";

export function Named() {
  onCleanup(() => {});
  return <h2>named</h2>;
}
