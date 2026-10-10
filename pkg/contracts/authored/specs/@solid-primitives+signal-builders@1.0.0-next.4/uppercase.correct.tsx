import { createSignal } from "solid-js";
import { uppercase } from "@solid-primitives/signal-builders";
export default function App() {
  const [input] = createSignal("hello");
  const result = uppercase(input);
  return <p>{String(result())}</p>;
}
