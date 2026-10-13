import { createSignal } from "solid-js";
import { clamp } from "@solid-primitives/utils";
export default function App() {
  const [source] = createSignal(1);
  const input: any = { valueOf() { source(); return 12; } };
  const result = clamp(input, 0, 10);
  return <p>{result}</p>;
}
