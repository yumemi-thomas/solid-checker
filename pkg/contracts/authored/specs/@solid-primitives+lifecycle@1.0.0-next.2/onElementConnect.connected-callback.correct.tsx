import { createSignal, untrack } from "solid-js";
import { onElementConnect } from "@solid-primitives/lifecycle";
export default function App() {
  const [value] = createSignal(1);
  onElementConnect(document.body, () => { untrack(value); });
  return <p>ready</p>;
}
