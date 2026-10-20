import { createSignal } from "solid-js";
import { useLabel } from "dep-hooks";

// Uncertifiable, not a violation: this import resolves to another installed
// copy of the package, `legacy/node_modules/dep-hooks`, whose source is not
// the selected one.
export function LegacyLabel() {
  const [name] = createSignal("a");
  const label = useLabel(name);
  return <p>{label}</p>;
}
