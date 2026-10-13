import { createSignal } from "solid-js";
import { createDerivedStaticStore } from "@solid-primitives/static-store";
export default function App() {
  const [input] = createSignal(0);
  const state = createDerivedStaticStore(() => ({ value: input() }));
  const current = state.value; // Expected STRICT_READ_UNTRACKED.
  return <p>{String(current)}</p>;
}
