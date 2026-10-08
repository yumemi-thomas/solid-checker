/** @jsxImportSource @solidjs/web */
import { createSignal, untrack } from "solid-js";
import { createFullscreen } from "@solid-primitives/fullscreen";
export default function App() {
  const [element] = createSignal(document.body);
  createFullscreen(() => untrack(element));
  return <p>ready</p>;
}
