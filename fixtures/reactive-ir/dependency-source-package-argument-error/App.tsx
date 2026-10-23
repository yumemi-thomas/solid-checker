import { createSignal } from "solid-js";
import { useLabel } from "dep-label";

export function Label() {
  const [count] = createSignal(1);
  const label = useLabel(count);
  return <p>{label}</p>;
}
