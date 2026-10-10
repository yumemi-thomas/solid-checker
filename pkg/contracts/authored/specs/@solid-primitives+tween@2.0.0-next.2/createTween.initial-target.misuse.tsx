/** @jsxImportSource @solidjs/web */
import { createSignal } from "solid-js";
import { createTween } from "@solid-primitives/tween";
export default function App() {
  const [source] = createSignal(1);
  // Keep the read in application code, inside the callback invoked by createTween.
  createTween(() => source(), { duration: 100 });
  return <p>ready</p>;
}
