// solid-groove StepEditor.tsx:82: a component keeps the tuple createSignal returns
// whole and calls its getter by index (`internal[0]()`).
import { createSignal } from "solid-js";

export function Selection(props: { ids?: () => readonly string[] }) {
  const internal = createSignal<readonly string[]>([]);
  const ids = () => props.ids?.() ?? internal[0]();
  return <p>{ids().length}</p>;
}
