import { createSignal, untrack } from "solid-js";
import { defer } from "@solid-primitives/utils";
export default function App() {
  const [source] = createSignal(1);
  const compute = defer(source, value => value);
  const value = untrack(() => compute(undefined));
  return <p>{String(value)}</p>;
}
