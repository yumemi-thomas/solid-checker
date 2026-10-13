import { createSignal } from "solid-js";
import { useLabel } from "dep-hooks";

// Outside the project's inherited `include`: never analyzed, in either mode.
export function Harness() {
  const [name] = createSignal("a");
  const label = useLabel(name);
  return <p>{label}</p>;
}
