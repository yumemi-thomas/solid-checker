import { createSignal } from "solid-js";
import { floor } from "@solid-primitives/signal-builders";
export default function App() {
  const [input] = createSignal(1.2);
  const result = floor(input);
  const current = result(); // Expected STRICT_READ_UNTRACKED.
  return <p>{String(current)}</p>;
}
