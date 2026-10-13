/** @jsxImportSource @solidjs/web */
import { createSignal } from "solid-js";
import { createMs } from "@solid-primitives/raf";
export default function App() {
  const [fps] = createSignal(60);
  createMs(() => fps());
  return <p>ready</p>;
}
