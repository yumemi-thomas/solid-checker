import { onSettled } from "solid-js";
import { createResizeObserver } from "@solid-primitives/resize-observer";
export default function App() {
  onSettled(() => { createResizeObserver(document.body, () => {}); });
  return <p>observer</p>;
}

