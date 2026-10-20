import { createSignal } from "solid-js";
import * as Escaped from "dep-ns/widget";

// The namespace itself is a value here: the import keeps its obligation.
export function EscapedNamespace() {
  const [count] = createSignal(0);
  const all = Escaped;
  const now = all.readNow(() => count());
  return <Escaped.Root>{now}</Escaped.Root>;
}
