import { createSignal } from "solid-js";
import { useLabel } from "dep-hooks";

// Positive: the selected package's source reads `name` during the call.
export function LabelNow() {
  const [name] = createSignal("a");
  const label = useLabel(name);
  return <p>{label}</p>;
}
