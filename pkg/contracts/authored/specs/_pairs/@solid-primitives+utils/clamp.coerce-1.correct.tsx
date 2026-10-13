import { createSignal, untrack } from "solid-js";
import { clamp } from "@solid-primitives/utils";
export default function App() {
  const [source] = createSignal(1);
  const input: any = { valueOf() { source(); return 0; } };
  const result = untrack(() => clamp(12, input, 10));
  return <p>{result}</p>;
}
