/** @jsxImportSource @solidjs/web */
import { createSignal, untrack } from "solid-js";
import { createFullscreen } from "@solid-primitives/fullscreen";
export default function App() {
  const [element] = createSignal(document.body);
  const [, setSink] = createSignal(0);
  let calls = 0;
  createFullscreen(() => {
    const node = calls === 0 ? untrack(element) : element();
    if (++calls > 1) { try { setSink(1); } catch {} }
    return node;
  });
  return <p>ready</p>;
}
