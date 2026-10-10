import { createSignal, untrack } from "solid-js";
import { onElementConnect } from "@solid-primitives/lifecycle";
export default function App() {
  const [value] = createSignal(1);
  onElementConnect(document.body, () => { value(); }); // Expected STRICT_READ_UNTRACKED inline.
  return <p>ready</p>;
}
