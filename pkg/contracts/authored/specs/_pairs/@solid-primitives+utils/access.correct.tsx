import { createSignal, untrack } from "solid-js";
import { access } from "@solid-primitives/utils";
export default function App() {
  const [source] = createSignal(3);
  // Keep the actual signal invocation in the case file, not inside package access.
  const readInCase = () => source();
  const seed = untrack(() => access(readInCase));
  return <p>{seed}</p>;
}
