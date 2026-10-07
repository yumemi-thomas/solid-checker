import { createSignal, untrack } from "solid-js";
import { access } from "@solid-primitives/utils";
export default function App() {
  const [source] = createSignal(3);
  const seed = untrack(() => access(source));
  return <p>{seed}</p>;
}

