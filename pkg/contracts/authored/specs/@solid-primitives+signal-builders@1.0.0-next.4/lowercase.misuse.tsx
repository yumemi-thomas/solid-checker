import { createSignal } from "solid-js";
import { lowercase } from "@solid-primitives/signal-builders";
export default function App() {
  const [input] = createSignal("hello");
  const result = lowercase(input);
  const current = result(); // Expected STRICT_READ_UNTRACKED.
  return <p>{String(current)}</p>;
}
