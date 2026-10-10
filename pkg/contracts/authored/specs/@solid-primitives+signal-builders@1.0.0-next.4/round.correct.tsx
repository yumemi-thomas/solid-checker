import { createSignal } from "solid-js";
import { round } from "@solid-primitives/signal-builders";
export default function App() {
  const [input] = createSignal(1.2);
  const result = round(input);
  return <p>{String(result())}</p>;
}
