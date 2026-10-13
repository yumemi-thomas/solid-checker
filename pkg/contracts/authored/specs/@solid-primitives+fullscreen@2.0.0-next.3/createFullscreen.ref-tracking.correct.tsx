/** @jsxImportSource @solidjs/web */
import { untrack } from "solid-js";
import { createSignal } from "solid-js";
import { createFullscreen } from "@solid-primitives/fullscreen";
export default function App() {
  const [element] = createSignal(document.body);
  const [, setSink] = createSignal(0);
  let calls = 0;
  createFullscreen(() => {
    const node = ++calls === 1 ? untrack(element) : element();
    return node;
  });
  return <p>ready</p>;
}
