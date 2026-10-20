import { createSignal } from "solid-js";
import { useLabel, useLabelMemo } from "dep-dotdot";

// Positive: the selected package's source reads `name` during the call,
// through the module it imports as `../lib/read.js`.
export function LabelNow() {
  const [name] = createSignal("a");
  const label = useLabel(name);
  return <p>{label}</p>;
}

// Negative: the same read, inside a memo the package creates.
export function LabelLater() {
  const [name] = createSignal("b");
  const label = useLabelMemo(name);
  return <p>{label()}</p>;
}
