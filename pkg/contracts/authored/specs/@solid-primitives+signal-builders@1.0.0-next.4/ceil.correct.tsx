import { createSignal } from "solid-js";
import { ceil } from "@solid-primitives/signal-builders";
export default function App() {
  const [input] = createSignal(1.2);
  const result = ceil(input);
  return <p>{String(result())}</p>;
}
