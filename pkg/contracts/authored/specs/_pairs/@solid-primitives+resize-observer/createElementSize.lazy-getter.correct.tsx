import { createMemo } from "solid-js";
import { createElementSize } from "@solid-primitives/resize-observer";
export default function App() {
  const size = createElementSize(document.createElement("div"));
  const primed = createMemo(() => size.width);
  return <p>{String(size.width)} {String(primed())}</p>;
}
