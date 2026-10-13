/** @jsxImportSource @solidjs/web */
import { createSignal } from "solid-js";
import { createFullscreen } from "@solid-primitives/fullscreen";
export default function App() {
  const [element] = createSignal(document.body);
  createFullscreen(() => element());
  return <p>ready</p>;
}
