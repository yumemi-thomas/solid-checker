import { onSettled } from "solid-js";
import { makeResizeObserver } from "@solid-primitives/resize-observer";
export default function App() {
  const observer = makeResizeObserver(() => {});
  onSettled(() => {
    observer.observe(document.body);
  });
  return <p>size</p>;
}

