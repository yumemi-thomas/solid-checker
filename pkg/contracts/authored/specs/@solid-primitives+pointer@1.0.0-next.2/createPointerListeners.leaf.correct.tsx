import { onSettled } from "solid-js";
import { createPointerListeners } from "@solid-primitives/pointer";
export default function App() {
  createPointerListeners({ target: document.body, onDown: () => {} });
  return <p>ready</p>;
}
