import { createSignal, untrack } from "solid-js";
import { clamp } from "@solid-primitives/utils";
export default function App() {
  const [source] = createSignal(1);
  const input: any = { valueOf() { source(); return 10; } };
  const result = untrack(() => clamp(12, 0, input));
  return <p>{result}</p>;
}
