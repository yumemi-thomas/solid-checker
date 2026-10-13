import { onSettled } from "solid-js";
import { createElementSize } from "@solid-primitives/resize-observer";
export default function App() {
  onSettled(() => { createElementSize(document.body); });
  return <p>observer</p>;
}

